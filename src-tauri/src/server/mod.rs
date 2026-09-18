mod ws;

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::extract::{Path, Query, State, WebSocketUpgrade};
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use include_dir::{include_dir, Dir};
use tokio::sync::{broadcast, Mutex as TokioMutex};

use crate::types::{ProcessSortKey, RuntimeKind};
use crate::Backend;

use ws::{handle_ws, LogEntry, WsFrame};

pub static DIST: Dir<'_> =
    include_dir!("$CARGO_MANIFEST_DIR/../dist");

pub struct ServerState {
    pub backend: Backend,
    pub events: broadcast::Sender<WsFrame>,
    pub log_streams: Arc<TokioMutex<HashMap<String, LogEntry>>>,
    pub token: Option<String>,
}

impl ServerState {
    fn token_ok(&self, provided: Option<&str>) -> bool {
        self.token.as_ref().map_or(true, |t| provided == Some(t))
    }
}

type Boxes = Arc<TokioMutex<HashMap<RuntimeKind, Arc<tokio::sync::RwLock<crate::engine::RuntimeState>>>>>;

pub(crate) async fn get_or_connect(
    states: &Boxes,
    kind: RuntimeKind,
) -> Result<bollard::Docker, String> {
    let entry = {
        let map = states.lock().await;
        map.get(&kind).cloned().ok_or("unknown runtime")?
    };
    if let Some(d) = entry.read().await.docker.clone() {
        return Ok(d);
    }
    let (d, _) = crate::engine::docker::connect_for(kind).await?;
    Ok(d)
}

pub async fn run(
    bind: IpAddr,
    port: u16,
    token: Option<String>,
    backend: Backend,
) -> std::io::Result<()> {
    let (events_tx, _rx) = broadcast::channel::<WsFrame>(512);
    let st = Arc::new(ServerState {
        backend,
        events: events_tx,
        log_streams: Arc::new(TokioMutex::new(HashMap::new())),
        token,
    });

    tokio::spawn(ws::reflect_loop(st.clone()));

    let addr = SocketAddr::new(bind, port);
    let app = Router::new()
        .route("/ws", get(ws_route))
        .route("/api/invoke/{cmd}", post(invoke_route))
        .fallback(get(fallback_route))
        .with_state(st);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let local = listener.local_addr()?;
    eprintln!("Monitor server: http://{local}  (ctrl-c to stop)");
    axum::serve(listener, app).await
}

async fn ws_route(
    State(st): State<Arc<ServerState>>,
    Query(q): Query<HashMap<String, String>>,
    ws: WebSocketUpgrade,
) -> Response {
    if !st.token_ok(q.get("token").map(|v| v.as_str())) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let st2 = st.clone();
    ws.on_upgrade(move |sock| handle_ws(sock, st2))
}

async fn invoke_route(
    State(st): State<Arc<ServerState>>,
    Query(q): Query<HashMap<String, String>>,
    Path(cmd): Path<String>,
    Json(args): Json<serde_json::Value>,
) -> Response {
    if !st.token_ok(q.get("token").map(|v| v.as_str())) {
        return (StatusCode::UNAUTHORIZED, "missing or invalid token".to_string()).into_response();
    }
    let res: Result<serde_json::Value, String> = match cmd.as_str() {
        "get_host_metrics" => {
            let host = st.backend.host.read().await.clone();
            Ok(serde_json::to_value(host).unwrap_or(serde_json::Value::Null))
        }
        "get_network_map" => {
            let nm = st.backend.netmap.read().await.clone();
            Ok(serde_json::to_value(nm).unwrap_or(serde_json::Value::Null))
        }
        "get_runtime_snapshot" => {
            let Some(kind_val) = args.get("kind").cloned() else {
                return (StatusCode::BAD_REQUEST, "missing kind".to_string()).into_response();
            };
            match serde_json::from_value::<RuntimeKind>(kind_val) {
                Ok(kind) => {
                    let snap = st.backend.snapshots.read().await.get(&kind).cloned();
                    Ok(serde_json::to_value(snap.unwrap_or_default())
                        .unwrap_or(serde_json::Value::Null))
                }
                Err(e) => Err(format!("bad kind: {e}")),
            }
        }
        "detect_runtimes" => {
            let list = crate::engine::detect::detect_all().await;
            Ok(serde_json::to_value(list).unwrap_or(serde_json::Value::Null))
        }
        "set_process_sort" => {
            let Some(key_val) = args.get("key").cloned() else {
                return (StatusCode::BAD_REQUEST, "missing key".to_string()).into_response();
            };
            match serde_json::from_value::<ProcessSortKey>(key_val) {
                Ok(key) => {
                    *st.backend.sort_key.write().await = key;
                    Ok(serde_json::Value::Null)
                }
                Err(e) => Err(format!("bad key: {e}")),
            }
        }
        "start_log_stream" => {
            let (kind, cid) = match stream_args(&args) {
                Ok(v) => v,
                Err(r) => return r.into_response(),
            };
            match ws::start_log_stream(&st, kind, cid).await {
                Ok(()) => Ok(serde_json::Value::Null),
                Err(e) => Err(e),
            }
        }
        "stop_log_stream" => {
            let (kind, cid) = match stream_args(&args) {
                Ok(v) => v,
                Err(r) => return r.into_response(),
            };
            ws::stop_log_stream(&st, kind, cid).await;
            Ok(serde_json::Value::Null)
        }
        _ => Err(format!("unknown command: {cmd}")),
    };
    match res {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

fn stream_args(args: &serde_json::Value) -> Result<(RuntimeKind, String), (StatusCode, String)> {
    let kind_val = args
        .get("kind")
        .cloned()
        .ok_or((StatusCode::BAD_REQUEST, "missing kind".to_string()))?;
    let cid = args
        .get("containerId")
        .or_else(|| args.get("container_id"))
        .and_then(|v| v.as_str())
        .ok_or((StatusCode::BAD_REQUEST, "missing containerId".to_string()))?
        .to_string();
    let kind: RuntimeKind = serde_json::from_value(kind_val)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("bad kind: {e}")))?;
    Ok((kind, cid))
}

/// Static assets (SPA fallback to /index.html) from dist/ embedded at build
/// time; returns 503 when assets were not embedded (dist missing at build).
async fn fallback_route(uri: Uri) -> Response {
    let raw = uri.path().trim_start_matches('/');
    let path = if raw.is_empty() { "index.html" } else { raw };
    let decoded = percent_decode(path);
    let file = DIST
        .get_file(&decoded)
        .or_else(|| DIST.get_file("index.html"));
    let Some(file) = file else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "frontend assets missing — run `pnpm build` and rebuild",
        )
            .into_response();
    };
    let mime = mime_for(&decoded);
    ([(header::CONTENT_TYPE, mime)], file.contents().to_vec()).into_response()
}

fn percent_decode(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn mime_for(path: &str) -> &'static str {
    let ext = path.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    match ext {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "wasm" => "application/wasm",
        "webp" => "image/webp",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}
