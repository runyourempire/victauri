use std::time::Duration;

struct Config {
    port: u16,
    interval: Duration,
    max_failures: u32,
    on_failure_cmd: Option<String>,
}

impl Config {
    fn from_env() -> Self {
        Self {
            port: std::env::var("VICTAURI_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .or_else(|| std::env::args().nth(1).and_then(|s| s.parse().ok()))
                .unwrap_or(7373),
            interval: Duration::from_secs(
                std::env::var("VICTAURI_INTERVAL")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(5),
            ),
            max_failures: std::env::var("VICTAURI_MAX_FAILURES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3),
            on_failure_cmd: std::env::var("VICTAURI_ON_FAILURE").ok(),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = Config::from_env();
    let url = format!("http://127.0.0.1:{}/health", config.port);

    tracing::info!(
        port = config.port,
        interval_secs = config.interval.as_secs(),
        max_failures = config.max_failures,
        on_failure = config.on_failure_cmd.as_deref().unwrap_or("(none)"),
        "Victauri watchdog started"
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    let mut consecutive_failures: u32 = 0;
    let mut action_fired = false;

    loop {
        tokio::time::sleep(config.interval).await;

        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                if consecutive_failures > 0 {
                    tracing::info!(
                        after_failures = consecutive_failures,
                        "Victauri MCP server recovered"
                    );
                    consecutive_failures = 0;
                    action_fired = false;
                }
            }
            Ok(resp) => {
                consecutive_failures += 1;
                tracing::warn!(
                    status = %resp.status(),
                    failure_count = consecutive_failures,
                    "Health check returned non-success status"
                );
            }
            Err(e) => {
                consecutive_failures += 1;
                tracing::warn!(
                    error = %e,
                    failure_count = consecutive_failures,
                    "Health check failed"
                );
            }
        }

        if consecutive_failures >= config.max_failures && !action_fired {
            tracing::error!(
                failure_count = consecutive_failures,
                "Victauri MCP server unreachable — the Tauri app may have crashed"
            );

            if let Some(ref cmd) = config.on_failure_cmd {
                tracing::info!(command = cmd, "Executing recovery action");
                match run_recovery(cmd).await {
                    Ok(status) => {
                        tracing::info!(exit_code = ?status.code(), "Recovery action completed");
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Recovery action failed to execute");
                    }
                }
            }

            action_fired = true;
        }
    }
}

async fn run_recovery(cmd: &str) -> anyhow::Result<std::process::ExitStatus> {
    let status = if cfg!(windows) {
        tokio::process::Command::new("cmd")
            .args(["/C", cmd])
            .status()
            .await?
    } else {
        tokio::process::Command::new("sh")
            .args(["-c", cmd])
            .status()
            .await?
    };
    Ok(status)
}
