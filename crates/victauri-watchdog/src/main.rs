use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(7373);

    let check_interval = Duration::from_secs(5);
    let url = format!("http://127.0.0.1:{port}/health");

    tracing::info!("Victauri watchdog monitoring port {port}");

    let mut consecutive_failures = 0u32;

    loop {
        tokio::time::sleep(check_interval).await;

        match reqwest::get(&url).await {
            Ok(resp) if resp.status().is_success() => {
                if consecutive_failures > 0 {
                    tracing::info!("Victauri MCP server recovered after {consecutive_failures} failures");
                    consecutive_failures = 0;
                }
            }
            Ok(resp) => {
                consecutive_failures += 1;
                tracing::warn!(
                    "Victauri health check returned {}: failure #{consecutive_failures}",
                    resp.status()
                );
            }
            Err(e) => {
                consecutive_failures += 1;
                tracing::warn!(
                    "Victauri health check failed: {e} — failure #{consecutive_failures}"
                );
            }
        }

        if consecutive_failures >= 3 {
            tracing::error!(
                "Victauri MCP server unreachable after {consecutive_failures} checks. \
                 The Tauri app may have crashed."
            );
            // TODO: Configurable recovery action (restart app, notify agent, etc.)
        }
    }
}
