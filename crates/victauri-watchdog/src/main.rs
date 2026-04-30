//! Watchdog process that monitors and restarts the Victauri MCP server if it becomes unresponsive.

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
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("victauri-watchdog {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("victauri-watchdog {}", env!("CARGO_PKG_VERSION"));
        println!("Crash-recovery sidecar for Victauri MCP server\n");
        println!("USAGE: victauri-watchdog [PORT]\n");
        println!("OPTIONS:");
        println!("  -h, --help       Print help");
        println!("  -V, --version    Print version\n");
        println!("ENVIRONMENT:");
        println!("  VICTAURI_PORT           Server port (default: 7373)");
        println!("  VICTAURI_INTERVAL       Poll interval in seconds (default: 5)");
        println!("  VICTAURI_MAX_FAILURES   Consecutive failures before action (default: 3)");
        println!("  VICTAURI_ON_FAILURE     Shell command to run on failure");
        return Ok(());
    }

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

#[cfg(test)]
#[allow(unsafe_code)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn clear_env() {
        // SAFETY: test-only — ENV_LOCK serializes all env access in this module.
        unsafe {
            std::env::remove_var("VICTAURI_PORT");
            std::env::remove_var("VICTAURI_INTERVAL");
            std::env::remove_var("VICTAURI_MAX_FAILURES");
            std::env::remove_var("VICTAURI_ON_FAILURE");
        }
    }

    #[test]
    fn config_defaults() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        clear_env();
        let config = Config::from_env();
        assert_eq!(config.port, 7373);
        assert_eq!(config.interval, Duration::from_secs(5));
        assert_eq!(config.max_failures, 3);
        assert!(config.on_failure_cmd.is_none());
    }

    #[test]
    fn config_from_env_vars() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        clear_env();
        // SAFETY: test-only — ENV_LOCK serializes all env access in this module.
        unsafe {
            std::env::set_var("VICTAURI_PORT", "9999");
            std::env::set_var("VICTAURI_INTERVAL", "10");
            std::env::set_var("VICTAURI_MAX_FAILURES", "5");
            std::env::set_var("VICTAURI_ON_FAILURE", "echo recovered");
        }
        let config = Config::from_env();
        assert_eq!(config.port, 9999);
        assert_eq!(config.interval, Duration::from_secs(10));
        assert_eq!(config.max_failures, 5);
        assert_eq!(config.on_failure_cmd, Some("echo recovered".to_string()));
        clear_env();
    }

    #[test]
    fn config_invalid_env_uses_defaults() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        clear_env();
        // SAFETY: test-only — ENV_LOCK serializes all env access in this module.
        unsafe {
            std::env::set_var("VICTAURI_PORT", "not_a_number");
            std::env::set_var("VICTAURI_INTERVAL", "abc");
            std::env::set_var("VICTAURI_MAX_FAILURES", "xyz");
        }
        let config = Config::from_env();
        assert_eq!(config.port, 7373);
        assert_eq!(config.interval, Duration::from_secs(5));
        assert_eq!(config.max_failures, 3);
        clear_env();
    }

    #[tokio::test]
    async fn recovery_runs_echo() {
        let cmd = "echo ok";
        let status = run_recovery(cmd).await.unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn recovery_bad_command_fails() {
        let cmd = if cfg!(windows) { "exit /b 1" } else { "exit 1" };
        let status = run_recovery(cmd).await.unwrap();
        assert!(!status.success());
    }
}
