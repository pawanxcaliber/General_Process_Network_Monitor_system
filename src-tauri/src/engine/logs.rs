use bollard::container::{LogOutput, LogsOptions};
use bollard::Docker;
use futures_util::StreamExt;
use tauri::{AppHandle, Emitter};

use crate::types::LogLine;

/// Consumes log lines from a container. GUI mode forwards them to the
/// frontend via the Tauri event `container-logs:{kind}:{id}`.
pub async fn stream_logs(app: AppHandle, docker: Docker, kind: String, container_id: String) {
    let event = format!("container-logs:{}:{}", kind, container_id);
    follow_logs(docker, kind, container_id, move |line| {
        let _ = app.emit(&event, &line);
    })
    .await;
}

/// Runtime-agnostic log follower used by both GUI and CLI mode.
pub async fn follow_logs<F>(docker: Docker, kind: String, container_id: String, mut on_line: F)
where
    F: FnMut(LogLine) + Send,
{
    let _context = kind;
    let opts: LogsOptions<String> = LogsOptions {
        follow: true,
        stdout: true,
        stderr: true,
        timestamps: true,
        tail: "500".to_string(),
        ..Default::default()
    };

    let mut stream = docker.logs(&container_id, Some(opts));

    while let Some(item) = stream.next().await {
        let chunk = match item {
            Ok(c) => c,
            Err(_) => break,
        };
        let (kind, message) = match chunk {
            LogOutput::StdErr { message } => ("stderr", message),
            LogOutput::StdIn { message } => ("stdout", message),
            LogOutput::Console { message } => ("stdout", message),
            LogOutput::StdOut { message } => ("stdout", message),
        };
        let full = String::from_utf8_lossy(&message).to_string();
        let (ts, text) = split_timestamp(&full);
        on_line(LogLine {
            stream: kind.to_string(),
            ts: Some(ts),
            text,
        });
    }

    on_line(LogLine {
        stream: "status".to_string(),
        ts: None,
        text: "__EOF__".to_string(),
    });
}

fn split_timestamp(full: &str) -> (String, String) {
    if let Some(idx) = full.find(' ') {
        let head = &full[..idx];
        if head.len() >= 20 && head.contains('T') && head.ends_with('Z') {
            return (
                head.to_string(),
                full[idx + 1..].trim_end_matches('\n').to_string(),
            );
        }
    }
    (String::new(), full.trim_end_matches('\n').to_string())
}
