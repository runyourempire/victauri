//! Victauri — full-stack introspection for Tauri apps via an embedded MCP server.
//!
//! Add this plugin to your Tauri app for AI-agent-driven testing and debugging:
//! DOM snapshots, IPC tracing, cross-boundary verification, and 55 more tools —
//! all accessible over the Model Context Protocol.
//!
//! # Quick Start
//!
//! ```ignore
//! tauri::Builder::default()
//!     .plugin(victauri_plugin::init())
//!     .run(tauri::generate_context!())
//!     .unwrap();
//! ```
//!
//! In debug builds this starts an MCP server on port 7373. In release builds
//! the plugin is a no-op with zero overhead.
//!
//! # Configuration
//!
//! ```ignore
//! tauri::Builder::default()
//!     .plugin(
//!         victauri_plugin::VictauriBuilder::new()
//!             .port(8080)
//!             .generate_auth_token()
//!             .strict_privacy_mode()
//!             .build(),
//!     )
//!     .run(tauri::generate_context!())
//!     .unwrap();
//! ```

pub mod bridge;
mod js_bridge;
pub mod mcp;
mod memory;
pub mod privacy;
pub mod redaction;
pub(crate) mod screenshot;
mod tools;

pub mod auth;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime};
use tokio::sync::{Mutex, oneshot};
use victauri_core::{CommandRegistry, EventLog, EventRecorder};

pub use victauri_macros::inspectable;

const DEFAULT_PORT: u16 = 7373;
const DEFAULT_EVENT_CAPACITY: usize = 10_000;
const DEFAULT_RECORDER_CAPACITY: usize = 50_000;
const DEFAULT_EVAL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Map of pending JavaScript eval callbacks, keyed by request ID.
/// Each entry holds a oneshot sender that resolves when the webview returns a result.
pub type PendingCallbacks = Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>;

/// Runtime state shared between the MCP server and all tool handlers.
pub struct VictauriState {
    /// Ring-buffer event log for IPC calls, state changes, and DOM mutations.
    pub event_log: EventLog,
    /// Registry of all discovered Tauri commands with metadata.
    pub registry: CommandRegistry,
    /// TCP port the MCP server listens on.
    pub port: u16,
    /// Pending JavaScript eval callbacks awaiting webview responses.
    pub pending_evals: PendingCallbacks,
    /// Session recorder for time-travel debugging.
    pub recorder: EventRecorder,
    /// Privacy configuration (tool disabling, command filtering, output redaction).
    pub privacy: privacy::PrivacyConfig,
    /// Timeout for JavaScript eval operations.
    pub eval_timeout: std::time::Duration,
}

/// Builder for configuring the Victauri plugin before adding it to a Tauri app.
///
/// Supports port selection, authentication, privacy controls, output redaction,
/// and capacity tuning. All settings have sensible defaults and can be overridden
/// via environment variables.
pub struct VictauriBuilder {
    port: Option<u16>,
    event_capacity: usize,
    recorder_capacity: usize,
    eval_timeout: std::time::Duration,
    auth_token: Option<String>,
    disabled_tools: Vec<String>,
    command_allowlist: Option<Vec<String>>,
    command_blocklist: Vec<String>,
    redaction_patterns: Vec<String>,
    redaction_enabled: bool,
    strict_privacy: bool,
}

impl Default for VictauriBuilder {
    fn default() -> Self {
        Self {
            port: None,
            event_capacity: DEFAULT_EVENT_CAPACITY,
            recorder_capacity: DEFAULT_RECORDER_CAPACITY,
            eval_timeout: DEFAULT_EVAL_TIMEOUT,
            auth_token: None,
            disabled_tools: Vec::new(),
            command_allowlist: None,
            command_blocklist: Vec::new(),
            redaction_patterns: Vec::new(),
            redaction_enabled: false,
            strict_privacy: false,
        }
    }
}

impl VictauriBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the TCP port for the MCP server (default: 7373, env: `VICTAURI_PORT`).
    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    /// Set the maximum number of events in the ring-buffer log (default: 10,000).
    pub fn event_capacity(mut self, capacity: usize) -> Self {
        self.event_capacity = capacity;
        self
    }

    /// Set the maximum events kept during session recording (default: 50,000).
    pub fn recorder_capacity(mut self, capacity: usize) -> Self {
        self.recorder_capacity = capacity;
        self
    }

    /// Set the timeout for JavaScript eval operations (default: 30s, env: `VICTAURI_EVAL_TIMEOUT`).
    pub fn eval_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.eval_timeout = timeout;
        self
    }

    /// Set an explicit auth token for the MCP server (env: `VICTAURI_AUTH_TOKEN`).
    pub fn auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    /// Generate a random UUID v4 auth token.
    pub fn generate_auth_token(mut self) -> Self {
        self.auth_token = Some(auth::generate_token());
        self
    }

    /// Disable specific MCP tools by name (e.g., `["eval_js", "screenshot"]`).
    pub fn disable_tools(mut self, tools: &[&str]) -> Self {
        self.disabled_tools = tools.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Only allow these Tauri commands to be invoked via MCP (positive allowlist).
    pub fn command_allowlist(mut self, commands: &[&str]) -> Self {
        self.command_allowlist = Some(commands.iter().map(|s| s.to_string()).collect());
        self
    }

    /// Block specific Tauri commands from being invoked via MCP.
    pub fn command_blocklist(mut self, commands: &[&str]) -> Self {
        self.command_blocklist = commands.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Add a regex pattern for output redaction (e.g., `r"SECRET_\w+"`).
    pub fn add_redaction_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.redaction_patterns.push(pattern.into());
        self
    }

    /// Enable output redaction with built-in patterns (API keys, emails, tokens).
    pub fn enable_redaction(mut self) -> Self {
        self.redaction_enabled = true;
        self
    }

    /// Enable strict privacy mode: disables dangerous tools (eval_js, screenshot,
    /// inject_css, set_storage, delete_storage, navigate, set_dialog_response,
    /// fill, type_text), enables output redaction with built-in PII patterns.
    pub fn strict_privacy_mode(mut self) -> Self {
        self.strict_privacy = true;
        self
    }

    fn resolve_port(&self) -> u16 {
        self.port
            .or_else(|| std::env::var("VICTAURI_PORT").ok()?.parse().ok())
            .unwrap_or(DEFAULT_PORT)
    }

    fn resolve_auth_token(&self) -> Option<String> {
        self.auth_token
            .clone()
            .or_else(|| std::env::var("VICTAURI_AUTH_TOKEN").ok())
    }

    fn resolve_eval_timeout(&self) -> std::time::Duration {
        std::env::var("VICTAURI_EVAL_TIMEOUT")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .map(std::time::Duration::from_secs)
            .unwrap_or(self.eval_timeout)
    }

    fn build_privacy_config(&self) -> privacy::PrivacyConfig {
        if self.strict_privacy {
            let mut config = privacy::strict_privacy_config();
            for cmd in &self.command_blocklist {
                config.command_blocklist.insert(cmd.clone());
            }
            if let Some(ref allow) = self.command_allowlist {
                config.command_allowlist = Some(allow.iter().cloned().collect());
            }
            for tool in &self.disabled_tools {
                config.disabled_tools.insert(tool.clone());
            }
            if !self.redaction_patterns.is_empty() {
                config.redactor = redaction::Redactor::new(&self.redaction_patterns);
            }
            config
        } else {
            privacy::PrivacyConfig {
                command_allowlist: self
                    .command_allowlist
                    .as_ref()
                    .map(|v| v.iter().cloned().collect::<HashSet<String>>()),
                command_blocklist: self.command_blocklist.iter().cloned().collect(),
                disabled_tools: self.disabled_tools.iter().cloned().collect(),
                redactor: redaction::Redactor::new(&self.redaction_patterns),
                redaction_enabled: self.redaction_enabled,
            }
        }
    }

    pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
        #[cfg(not(debug_assertions))]
        {
            Builder::new("victauri").build()
        }

        #[cfg(debug_assertions)]
        {
            let port = self.resolve_port();
            let event_capacity = self.event_capacity;
            let recorder_capacity = self.recorder_capacity;
            let eval_timeout = self.resolve_eval_timeout();
            let auth_token = self.resolve_auth_token();
            let privacy_config = self.build_privacy_config();

            Builder::new("victauri")
                .setup(move |app, _api| {
                    let event_log = EventLog::new(event_capacity);
                    let registry = CommandRegistry::new();

                    let state = Arc::new(VictauriState {
                        event_log,
                        registry,
                        port,
                        pending_evals: Arc::new(Mutex::new(HashMap::new())),
                        recorder: EventRecorder::new(recorder_capacity),
                        privacy: privacy_config,
                        eval_timeout,
                    });

                    app.manage(state.clone());

                    if let Some(ref token) = auth_token {
                        tracing::info!(
                            "Victauri MCP server auth token: [REDACTED] (check VICTAURI_AUTH_TOKEN env var)"
                        );
                        tracing::debug!("Auth token value: {token}");
                    }

                    let app_handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) =
                            mcp::start_server_with_options(app_handle, state, port, auth_token)
                                .await
                        {
                            tracing::error!("Victauri MCP server failed to start: {e}");
                        }
                    });

                    tracing::info!("Victauri plugin initialized — MCP server on port {port}");
                    Ok(())
                })
                .js_init_script(js_bridge::INIT_SCRIPT.to_string())
                .invoke_handler(tauri::generate_handler![
                    tools::victauri_eval_js,
                    tools::victauri_eval_callback,
                    tools::victauri_get_window_state,
                    tools::victauri_list_windows,
                    tools::victauri_get_ipc_log,
                    tools::victauri_get_registry,
                    tools::victauri_get_memory_stats,
                    tools::victauri_dom_snapshot,
                    tools::victauri_verify_state,
                    tools::victauri_detect_ghost_commands,
                    tools::victauri_check_ipc_integrity,
                ])
                .build()
        }
    }
}

/// Initialize the Victauri plugin with default settings (port 7373 or VICTAURI_PORT env var).
///
/// In debug builds: starts the embedded MCP server, injects the JS bridge, and
/// registers all Tauri command handlers.
///
/// In release builds: returns a no-op plugin. The MCP server, JS bridge, and
/// all introspection tools are completely stripped — zero overhead, zero attack surface.
///
/// For custom configuration, use `VictauriBuilder::new().port(8080).build()`.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    VictauriBuilder::new().build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_default_values() {
        let builder = VictauriBuilder::new();
        assert_eq!(builder.event_capacity, DEFAULT_EVENT_CAPACITY);
        assert_eq!(builder.recorder_capacity, DEFAULT_RECORDER_CAPACITY);
        assert!(builder.auth_token.is_none());
        assert!(builder.disabled_tools.is_empty());
        assert!(builder.command_allowlist.is_none());
        assert!(builder.command_blocklist.is_empty());
        assert!(!builder.redaction_enabled);
        assert!(!builder.strict_privacy);
    }

    #[test]
    fn builder_port_override() {
        let builder = VictauriBuilder::new().port(9090);
        assert_eq!(builder.resolve_port(), 9090);
    }

    #[test]
    fn builder_default_port() {
        let builder = VictauriBuilder::new();
        // Clear env var to test default
        unsafe { std::env::remove_var("VICTAURI_PORT") };
        assert_eq!(builder.resolve_port(), DEFAULT_PORT);
    }

    #[test]
    fn builder_auth_token_explicit() {
        let builder = VictauriBuilder::new().auth_token("my-secret");
        assert_eq!(builder.resolve_auth_token(), Some("my-secret".to_string()));
    }

    #[test]
    fn builder_auth_token_generated() {
        let builder = VictauriBuilder::new().generate_auth_token();
        let token = builder.resolve_auth_token().unwrap();
        assert_eq!(token.len(), 36);
    }

    #[test]
    fn builder_capacities() {
        let builder = VictauriBuilder::new()
            .event_capacity(500)
            .recorder_capacity(2000);
        assert_eq!(builder.event_capacity, 500);
        assert_eq!(builder.recorder_capacity, 2000);
    }

    #[test]
    fn builder_disable_tools() {
        let builder = VictauriBuilder::new().disable_tools(&["eval_js", "screenshot"]);
        assert_eq!(builder.disabled_tools.len(), 2);
        assert!(builder.disabled_tools.contains(&"eval_js".to_string()));
    }

    #[test]
    fn builder_command_allowlist() {
        let builder = VictauriBuilder::new().command_allowlist(&["greet", "increment"]);
        assert!(builder.command_allowlist.is_some());
        assert_eq!(builder.command_allowlist.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn builder_command_blocklist() {
        let builder = VictauriBuilder::new().command_blocklist(&["dangerous_cmd"]);
        assert_eq!(builder.command_blocklist.len(), 1);
    }

    #[test]
    fn builder_redaction() {
        let builder = VictauriBuilder::new()
            .add_redaction_pattern(r"SECRET_\w+")
            .enable_redaction();
        assert!(builder.redaction_enabled);
        assert_eq!(builder.redaction_patterns.len(), 1);
    }

    #[test]
    fn builder_strict_privacy_config() {
        let builder = VictauriBuilder::new().strict_privacy_mode();
        let config = builder.build_privacy_config();
        assert!(config.redaction_enabled);
        assert!(!config.disabled_tools.is_empty());
        assert!(config.disabled_tools.contains("eval_js"));
        assert!(config.disabled_tools.contains("screenshot"));
    }

    #[test]
    fn builder_normal_privacy_config() {
        let builder = VictauriBuilder::new()
            .command_blocklist(&["secret_cmd"])
            .disable_tools(&["eval_js"]);
        let config = builder.build_privacy_config();
        assert!(config.command_blocklist.contains("secret_cmd"));
        assert!(config.disabled_tools.contains("eval_js"));
        assert!(!config.redaction_enabled);
    }

    #[test]
    fn builder_strict_with_extra_blocklist() {
        let builder = VictauriBuilder::new()
            .strict_privacy_mode()
            .command_blocklist(&["extra_dangerous"]);
        let config = builder.build_privacy_config();
        assert!(config.command_blocklist.contains("extra_dangerous"));
        assert!(config.disabled_tools.contains("eval_js"));
    }
}
