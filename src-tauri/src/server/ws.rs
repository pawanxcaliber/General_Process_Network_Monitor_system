use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::engine::logs;
use crate::types::{HostPayload, LogLine, NetworkMapPayload, RuntimeKind as RKind, RuntimeSnapshot, RuntimeTick, RuntimeStatus};

use super::ServerState;

/// One server → client websocket frame; mirrors Tauri event semantics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsFrame {
    pub event: String,
    pub payload: serde_json::Value,
}

impl WsFrame {
    pub fn ser<T: Serialize>(event: &str, payload: &T) -> Self {
        Self {
            event: event.to_string(),
            payload: serde_json::to_value(payload).unwrap_or(serde_json::Value::Null),
        }
    }

    pub fn text(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Active container log stream per `kind:container` refcounted on viewers.
pub struct LogEntry {
    pub task: tokio::task::JoinHandle<()>,
}

/// Starts (or reuses) a container log stream and broadcasts frames to all
/// websocket clients. Mirrors the GUI `start_log_stream` command.
pub async fn start_log_stream(
    st: &Arc<ServerState>,
    kind: RKind,
    container_id: String,
) -> Result<(), String> {
    let key = format!("{}:{}", kind.as_str(), container_id);

    let docker = super::get_or_connect(&st.backend.states, kind).await?;

    let mut streams = st.log_streams.lock().await;
    if let Some(e) = streams.get(&key) {
        if !e.task.is_finished() {
            return Ok(());
        }
    }

    let (tx, _rx) = broadcast::channel::<LogLine>(256);
    let events = st.events.clone();
    let cid = container_id.clone();
    let kname = kind.as_str().to_string();
    let event_name = format!("container-logs:{}", key);
    let task = tokio::spawn(async move {
        logs::follow_logs(docker, kname, cid, move |line: LogLine| {
            if line.text == "__EOF__" {
                let eof = LogLine {
                    stream: "status".into(),
                    ts: None,
                    text: "__EOF__".into(),
                };
                let _ = tx.send(eof.clone());
                let _ = events.send(WsFrame::ser(&event_name, &eof));
            } else {
                let _ = tx.send(line.clone());
                let _ = events.send(WsFrame::ser(&event_name, &line));
            }
        })
        .await;
    });
    streams.insert(key, LogEntry { task });
    Ok(())
}

/// Stops a container log stream (mirrors the GUI `stop_log_stream`).
pub async fn stop_log_stream(st: &Arc<ServerState>, kind: RKind, container_id: String) {
    let key = format!("{}:{}", kind.as_str(), container_id);
    let event_name = format!("container-logs:{}", key);
    let entry = st.log_streams.lock().await.remove(&key);
    if let Some(entry) = entry {
        entry.task.abort();
    }
    let eof = LogLine {
        stream: "status".into(),
        ts: None,
        text: "__EOF__".into(),
    };
    let _ = st
        .events
        .send(WsFrame::ser(&event_name, &eof));
}

/// Initial frames pushed on a fresh WS connection; same "first fetch" as the
/// GUI's useBackend:
pub async fn initial_frames(st: &Arc<ServerState>) -> Vec<WsFrame> {
    let mut out = Vec::new();
    let host: HostPayload = st.backend.host.read().await.clone();
    out.push(WsFrame::ser("host-metrics-tick", &host));
    let netmap: NetworkMapPayload = st.backend.netmap.read().await.clone();
    out.push(WsFrame::ser("network-map-tick", &netmap));
    let map = st.backend.snapshots.read().await.clone();
    for k in [
        RKind::Docker,
        RKind::Podman,
        RKind::Kubernetes,
        RKind::Vm,
    ] {
        let snap: Option<&RuntimeSnapshot> = map.get(&k);
        let snap = snap.cloned().unwrap_or_default();
        out.push(WsFrame::ser(
            "runtime-tick",
            &RuntimeTick { kind: k, snapshot: snap },
        ));
    }
    let stts: Vec<RuntimeStatus> = engine_detect().await;
    for s in stts {
        out.push(WsFrame::ser("runtime-status", &s));
    }
    out
}

async fn engine_detect() -> Vec<RuntimeStatus> {
    crate::engine::detect::detect_all().await
}

/// Reflects cache changes to the event broadcast, so WS clients receive the
/// same tick stream the GUI gets via Tauri events.
pub async fn reflect_loop(st: Arc<ServerState>) {
    let mut prev_host = Vec::new();
    let mut prev_netmap = Vec::new();
    let mut prev_snaps: HashMap<RKind, Vec<u8>> = HashMap::new();
    loop {
        tokio::time::sleep(Duration::from_millis(200)).await;

        let host = st.backend.host.read().await.clone();
        if ok_changed(&mut prev_host, &host) {
            let _ = st.events.send(WsFrame::ser("host-metrics-tick", &host));
        }

        let netmap = st.backend.netmap.read().await.clone();
        if ok_changed(&mut prev_netmap, &netmap) {
            let _ = st.events.send(WsFrame::ser("network-map-tick", &netmap));
        }

        let snaps = st.backend.snapshots.read().await.clone();
        for k in [
            RKind::Docker,
            RKind::Podman,
            RKind::Kubernetes,
            RKind::Vm,
        ] {
            let snap = snaps
                .get(&k)
                .cloned()
                .unwrap_or_default();
            let prev = prev_snaps.entry(k).or_default();
            let encoded = serde_json::to_vec(&snap).unwrap_or_default();
            if *prev != encoded {
                *prev = encoded;
                let _ = st.events.send(WsFrame::ser(
                    "runtime-tick",
                    &RuntimeTick {
                        kind: k,
                        snapshot: snap,
                    },
                ));
            }
        }
    }
}

fn ok_changed<T: Serialize>(prev: &mut Vec<u8>, cur: &T) -> bool {
    let encoded = serde_json::to_vec(cur).unwrap_or_default();
    if *prev != encoded {
        *prev = encoded;
        true
    } else {
        false
    }
}

/// Handles one WS connection: initial state, then streams broadcast events.
pub async fn handle_ws(socket: axum::extract::ws::WebSocket, st: Arc<ServerState>) {
    let (mut sender, mut receiver) = socket.split();

    for f in initial_frames(&st).await {
        if sender
            .send(Message::Text(f.text().into()))
            .await
            .is_err()
        {
            return;
        }
    }

    let mut fan = st.events.subscribe();
    loop {
        tokio::select! {
            frame = fan.recv() => match frame {
                Ok(f) => {
                    if sender.send(Message::Text(f.text().into())).await.is_err() {
                        return;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            },
            msg = receiver.next() => match msg {
                Some(Ok(_msg)) => continue,
                Some(Err(_)) | None => break,
            },
        }
    }
}
