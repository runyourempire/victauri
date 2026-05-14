use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::tab_state::TabManager;

/// MCP tool handler for the browser extension.
///
/// Dispatches tool calls to the Chrome extension via native messaging,
/// routes to the correct tab, and manages per-tab state.
#[derive(Clone)]
pub struct VictauriBrowserHandler {
    tab_manager: Arc<TabManager>,
    tool_invocations: Arc<AtomicU64>,
}

impl VictauriBrowserHandler {
    #[must_use]
    pub fn new(tab_manager: Arc<TabManager>) -> Self {
        Self {
            tab_manager,
            tool_invocations: Arc::new(AtomicU64::new(0)),
        }
    }

    pub async fn tab_count(&self) -> usize {
        self.tab_manager.tab_count().await
    }

    /// List available tools with their descriptions.
    #[must_use]
    pub fn list_tools(&self) -> Vec<ToolInfo> {
        vec![
            ToolInfo::new("eval_js", "Execute JavaScript in the active page"),
            ToolInfo::new("dom_snapshot", "Get accessible DOM tree with ref handles"),
            ToolInfo::new(
                "find_elements",
                "Search DOM elements by text, role, selector, or attribute",
            ),
            ToolInfo::new("interact", "Click, hover, focus, scroll, or select elements"),
            ToolInfo::new("input", "Fill, type text, or press keys"),
            ToolInfo::new(
                "inspect",
                "CSS inspection, visual debug, accessibility audit, performance",
            ),
            ToolInfo::new("css", "Inject or remove custom CSS"),
            ToolInfo::new(
                "logs",
                "Console, network, navigation, dialog, and event logs",
            ),
            ToolInfo::new("storage", "localStorage, sessionStorage, and cookie access"),
            ToolInfo::new("navigate", "Navigate, go back, manage dialogs"),
            ToolInfo::new("wait_for", "Wait for DOM conditions, text, or URL changes"),
            ToolInfo::new(
                "assert_semantic",
                "Evaluate expression and assert condition",
            ),
            ToolInfo::new("recording", "Record interactions, checkpoint, replay"),
            ToolInfo::new("screenshot", "Take page screenshot (PNG)"),
            ToolInfo::new("tabs", "Manage browser tabs and windows"),
            ToolInfo::new("page_info", "Get page metadata, headers, and resources"),
            ToolInfo::new("cookies", "Cross-origin cookie management"),
            ToolInfo::new("get_diagnostics", "Browser and extension diagnostics"),
            ToolInfo::new("get_plugin_info", "Extension and host version info"),
            ToolInfo::new("get_memory_stats", "JS heap memory statistics"),
        ]
    }

    /// Execute a tool by name with JSON arguments.
    ///
    /// # Errors
    ///
    /// Returns an error string if the tool is unknown or execution fails.
    pub async fn execute_tool(
        &self,
        name: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.tool_invocations.fetch_add(1, Ordering::Relaxed);

        match name {
            "get_plugin_info" => Ok(serde_json::json!({
                "name": "victauri-browser",
                "version": env!("CARGO_PKG_VERSION"),
                "mode": "browser",
                "tool_count": self.list_tools().len(),
                "tab_count": self.tab_manager.tab_count().await,
                "invocations": self.tool_invocations.load(Ordering::Relaxed),
            })),
            "tabs" => {
                let action = args
                    .get("action")
                    .and_then(|v| v.as_str())
                    .unwrap_or("list");
                match action {
                    "list" => {
                        let tabs = self.tab_manager.list_tabs().await;
                        Ok(serde_json::to_value(tabs).unwrap_or_default())
                    }
                    _ => Err(format!("unknown tabs action: {action}")),
                }
            }
            _ => Err(format!(
                "tool '{name}' requires extension connection (not yet dispatching)"
            )),
        }
    }
}

#[derive(Clone, serde::Serialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
}

impl ToolInfo {
    fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_list_has_20_tools() {
        let tab_mgr = Arc::new(TabManager::new());
        let handler = VictauriBrowserHandler::new(tab_mgr);
        assert_eq!(handler.list_tools().len(), 20);
    }

    #[tokio::test]
    async fn plugin_info_returns_metadata() {
        let tab_mgr = Arc::new(TabManager::new());
        let handler = VictauriBrowserHandler::new(tab_mgr);
        let result = handler
            .execute_tool("get_plugin_info", serde_json::json!({}))
            .await
            .unwrap();
        assert_eq!(result["name"], "victauri-browser");
        assert_eq!(result["mode"], "browser");
    }

    #[tokio::test]
    async fn unknown_tool_returns_error() {
        let tab_mgr = Arc::new(TabManager::new());
        let handler = VictauriBrowserHandler::new(tab_mgr);
        let result = handler
            .execute_tool("nonexistent", serde_json::json!({}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn tabs_list_empty() {
        let tab_mgr = Arc::new(TabManager::new());
        let handler = VictauriBrowserHandler::new(tab_mgr);
        let result = handler
            .execute_tool("tabs", serde_json::json!({"action": "list"}))
            .await
            .unwrap();
        assert!(result.as_array().unwrap().is_empty());
    }
}
