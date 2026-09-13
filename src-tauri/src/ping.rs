use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

const PING_TARGET: &str = "1.1.1.1";
const TCP_TARGET: &str = "1.1.1.1:443";
const MAX_FAILURES: u32 = 3;

pub async fn start_ping_poll(cache: Arc<RwLock<Option<f64>>>) {
    let mut failures: u32 = 0;
    let mut icmp_blocked = false;
    loop {
        let ms = if icmp_blocked {
            tcp_ping_with_retry().await
        } else {
            match icmp_ping().await {
                Some(ms) => Some(ms),
                None => {
                    // ICMP unavailable here; skip it on subsequent cycles.
                    icmp_blocked = true;
                    tcp_ping_with_retry().await
                }
            }
        };

        match ms {
            Some(ms) => {
                failures = 0;
                *cache.write().await = Some(ms);
            }
            None => {
                failures += 1;
                // Keep the last good value through transient failures.
                if failures >= MAX_FAILURES {
                    *cache.write().await = None;
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

/// ICMP first, then TCP connect latency for networks that block outbound
/// ICMP. Retries TCP once before reporting failure.
async fn tcp_ping_with_retry() -> Option<f64> {
    if let Some(ms) = tcp_ping().await {
        return Some(ms);
    }
    tcp_ping().await
}

async fn icmp_ping() -> Option<f64> {
    let out = tokio::process::Command::new("ping")
        .args(ping_args())
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    parse_ping_ms(&stdout)
}

/// Platform-specific `ping` arguments (count 1, ~2s wait).
fn ping_args() -> Vec<&'static str> {
    #[cfg(target_os = "windows")]
    {
        vec!["-n", "1", "-w", "2000", PING_TARGET]
    }
    #[cfg(target_os = "macos")]
    {
        // macOS `-W` is a millisecond wait time.
        vec!["-c", "1", "-W", "2000", PING_TARGET]
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        // Linux/BSD `-W` is a second timeout.
        vec!["-c", "1", "-W", "2", PING_TARGET]
    }
}

async fn tcp_ping() -> Option<f64> {
    let start = Instant::now();
    let ok = tokio::time::timeout(
        Duration::from_secs(3),
        tokio::net::TcpStream::connect(TCP_TARGET),
    )
    .await
    .ok()?
    .ok()?;
    drop(ok);
    Some(start.elapsed().as_secs_f64() * 1000.0)
}

fn parse_ping_ms(out: &str) -> Option<f64> {
    for line in out.lines() {
        // Windows reports sub-millisecond replies as `time<1ms`.
        if line.contains("time<") || line.contains("time <") {
            return Some(0.5);
        }
        if let Some(idx) = line.find("time=") {
            let rest = &line[idx + 5..];
            let end = rest
                .find(|c: char| !(c.is_ascii_digit() || c == '.'))
                .unwrap_or(rest.len());
            if let Ok(ms) = rest[..end].parse::<f64>() {
                return Some(ms);
            }
        }
    }
    None
}
