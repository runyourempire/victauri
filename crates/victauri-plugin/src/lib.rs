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

pub type PendingCallbacks = Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>;

pub struct VictauriState {
    pub event_log: EventLog,
    pub registry: CommandRegistry,
    pub port: u16,
    pub pending_evals: PendingCallbacks,
    pub recorder: EventRecorder,
    pub privacy: privacy::PrivacyConfig,
}

pub struct VictauriBuilder {
    port: Option<u16>,
    event_capacity: usize,
    recorder_capacity: usize,
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

    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub fn event_capacity(mut self, capacity: usize) -> Self {
        self.event_capacity = capacity;
        self
    }

    pub fn recorder_capacity(mut self, capacity: usize) -> Self {
        self.recorder_capacity = capacity;
        self
    }

    pub fn auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    pub fn generate_auth_token(mut self) -> Self {
        self.auth_token = Some(auth::generate_token());
        self
    }

    pub fn disable_tools(mut self, tools: &[&str]) -> Self {
        self.disabled_tools = tools.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn command_allowlist(mut self, commands: &[&str]) -> Self {
        self.command_allowlist = Some(commands.iter().map(|s| s.to_string()).collect());
        self
    }

    pub fn command_blocklist(mut self, commands: &[&str]) -> Self {
        self.command_blocklist = commands.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn add_redaction_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.redaction_patterns.push(pattern.into());
        self
    }

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
