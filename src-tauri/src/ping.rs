use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

const PING_TARGET: &str = "1.1.1.1";

pub async fn start_ping_poll(cache: Arc<RwLock<Option<f64>>>) {
    loop {
        let ms = ping_once().await;
        *cache.write().await = ms;
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn ping_once() -> Option<f64> {
    let out = tokio::process::Command::new("ping")
        .args(["-c", "1", "-W", "2", PING_TARGET])
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    parse_ping_ms(&stdout)
}

fn parse_ping_ms(out: &str) -> Option<f64> {
    for line in out.lines() {
        if let Some(idx) = line.find("time=") {
            let rest = &line[idx + 5..];
            let end = rest
                .find(|c: char| !(c.is_ascii_digit() || c == '.'))
                .unwrap_or(rest.len());
            return rest[..end].parse::<f64>().ok();
        }
    }
    None
}
