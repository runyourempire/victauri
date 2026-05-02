use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use crate::VictauriClient;
use crate::error::TestError;

/// Managed Tauri application lifecycle for integration testing.
///
/// Spawns a Tauri app as a child process, waits for the Victauri MCP server
/// to become healthy, and provides connected [`VictauriClient`] instances.
/// The app is killed when the `TestApp` is dropped.
///
/// # Example
///
/// ```rust,ignore
/// use victauri_test::TestApp;
///
/// #[tokio::test]
/// async fn my_app_works() {
///     let app = TestApp::spawn("cargo run -p my-app").await.unwrap();
///     let mut client = app.client().await.unwrap();
///     client.click_by_text("Submit").await.unwrap();
///     client.expect_text("Success").await.unwrap();
/// }
/// ```
pub struct TestApp {
    child: Option<Child>,
    port: u16,
    token: Option<String>,
}

impl TestApp {
    /// Spawn an application from a shell command and wait for it to become ready.
    ///
    /// Polls the Victauri health endpoint until it responds (up to 30 seconds).
    /// Uses port auto-discovery via temp files, falling back to env vars and defaults.
    ///
    /// # Errors
    ///
    /// Returns [`TestError::Connection`] if the app fails to start or the health
    /// endpoint doesn't respond within the timeout.
    pub async fn spawn(cmd: &str) -> Result<Self, TestError> {
        Self::spawn_with_options(cmd, None, Duration::from_secs(30)).await
    }

    /// Spawn with explicit port and timeout configuration.
    ///
    /// # Errors
    ///
    /// Returns [`TestError::Connection`] if the app fails to start or the health
    /// endpoint doesn't respond within the timeout.
    pub async fn spawn_with_options(
        cmd: &str,
        port: Option<u16>,
        timeout: Duration,
    ) -> Result<Self, TestError> {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() {
            return Err(TestError::Connection("empty command".into()));
        }

        let child = Command::new(parts[0])
            .args(&parts[1..])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| TestError::Connection(format!("failed to spawn `{cmd}`: {e}")))?;

        let mut app = Self {
            child: Some(child),
            port: port.unwrap_or(0),
            token: None,
        };

        app.wait_for_ready(timeout).await?;
        Ok(app)
    }

    /// Spawn the bundled demo app from the workspace.
    ///
    /// Equivalent to `TestApp::spawn("cargo run -p demo-app")` but with
    /// appropriate environment variables set.
    ///
    /// # Errors
    ///
    /// Returns [`TestError::Connection`] if the demo app fails to start.
    pub async fn spawn_demo() -> Result<Self, TestError> {
        let port = discover_port();
        let parts = ["cargo", "run", "-p", "demo-app"];

        let child = Command::new(parts[0])
            .args(&parts[1..])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| TestError::Connection(format!("failed to spawn demo-app: {e}")))?;

        let mut app = Self {
            child: Some(child),
            port,
            token: None,
        };

        app.wait_for_ready(Duration::from_secs(60)).await?;
        Ok(app)
    }

    /// Connect to an already-running Victauri app (no process management).
    ///
    /// Useful when the app is started externally (e.g., by CI or a dev script).
    ///
    /// # Errors
    ///
    /// Returns [`TestError::Connection`] if the health endpoint doesn't respond.
    pub async fn attach(port: u16, token: Option<String>) -> Result<Self, TestError> {
        let app = Self {
            child: None,
            port,
            token,
        };

        let http = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{port}/health");
        let resp = http
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| TestError::Connection(format!("health check failed: {e}")))?;

        if !resp.status().is_success() {
            return Err(TestError::Connection(format!(
                "health returned {}",
                resp.status()
            )));
        }

        Ok(app)
    }

    /// Create a new connected [`VictauriClient`] for this app.
    ///
    /// Each call returns a fresh MCP session.
    ///
    /// # Errors
    ///
    /// Returns errors from [`VictauriClient::connect_with_token`].
    pub async fn client(&self) -> Result<VictauriClient, TestError> {
        VictauriClient::connect_with_token(self.port, self.token.as_deref()).await
    }

    /// The port the MCP server is running on.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    async fn wait_for_ready(&mut self, timeout: Duration) -> Result<(), TestError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|e| TestError::Connection(e.to_string()))?;

        let start = std::time::Instant::now();
        let poll_interval = Duration::from_millis(200);

        loop {
            if start.elapsed() > timeout {
                return Err(TestError::Connection(format!(
                    "app did not become ready within {}s — check that the Victauri plugin is \
                     initialized and the MCP server is listening. Try setting VICTAURI_PORT or \
                     checking the app's stderr for errors.",
                    timeout.as_secs()
                )));
            }

            if let Some(ref mut child) = self.child
                && let Some(status) = child.try_wait().ok().flatten()
            {
                return Err(TestError::Connection(format!(
                    "app process exited with {status} before becoming ready"
                )));
            }

            let port = self.discover_actual_port();
            let url = format!("http://127.0.0.1:{port}/health");

            if let Ok(resp) = http.get(&url).send().await
                && resp.status().is_success()
            {
                self.port = port;
                self.token = discover_token();
                return Ok(());
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

    fn discover_actual_port(&self) -> u16 {
        if self.port != 0 {
            return self.port;
        }
        discover_port()
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn discover_port() -> u16 {
    if let Ok(p) = std::env::var("VICTAURI_PORT")
        && let Ok(port) = p.parse::<u16>()
    {
        return port;
    }
    let path = port_file_path();
    if let Ok(contents) = std::fs::read_to_string(&path)
        && let Ok(port) = contents.trim().parse::<u16>()
    {
        return port;
    }
    7373
}

fn discover_token() -> Option<String> {
    if let Ok(token) = std::env::var("VICTAURI_AUTH_TOKEN") {
        return Some(token);
    }
    let path = token_file_path();
    let token = std::fs::read_to_string(&path).ok()?;
    let token = token.trim().to_string();
    if token.is_empty() { None } else { Some(token) }
}

fn port_file_path() -> PathBuf {
    std::env::temp_dir().join("victauri.port")
}

fn token_file_path() -> PathBuf {
    std::env::temp_dir().join("victauri.token")
}
