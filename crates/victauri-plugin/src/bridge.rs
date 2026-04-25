use tauri::{Manager, Runtime};
use victauri_core::WindowState;

pub trait WebviewBridge: Send + Sync {
    fn eval_webview(&self, label: Option<&str>, script: &str) -> Result<(), String>;
    fn get_window_states(&self, label: Option<&str>) -> Vec<WindowState>;
    fn list_window_labels(&self) -> Vec<String>;
}

impl<R: Runtime> WebviewBridge for tauri::AppHandle<R> {
    fn eval_webview(&self, label: Option<&str>, script: &str) -> Result<(), String> {
        let windows = self.webview_windows();
        let webview = match label {
            Some(l) => windows
                .get(l)
                .ok_or_else(|| format!("window not found: {l}"))?,
            None => windows
                .get("main")
                .or_else(|| {
                    windows.values().find(|w| w.is_visible().unwrap_or(false))
                })
                .or_else(|| windows.values().next())
                .ok_or_else(|| "no webview available".to_string())?,
        };
        webview.eval(script).map_err(|e| e.to_string())
    }

    fn get_window_states(&self, label: Option<&str>) -> Vec<WindowState> {
        let windows = self.webview_windows();
        let mut states = Vec::new();

        for (win_label, window) in &windows {
            if let Some(filter) = label {
                if win_label != filter {
                    continue;
                }
            }

            let pos = window.outer_position().unwrap_or_default();
            let size = window.inner_size().unwrap_or_default();

            states.push(WindowState {
                label: win_label.clone(),
                title: window.title().unwrap_or_default(),
                url: window.url().map(|u| u.to_string()).unwrap_or_default(),
                visible: window.is_visible().unwrap_or(false),
                focused: window.is_focused().unwrap_or(false),
                maximized: window.is_maximized().unwrap_or(false),
                minimized: window.is_minimized().unwrap_or(false),
                fullscreen: window.is_fullscreen().unwrap_or(false),
                position: (pos.x, pos.y),
                size: (size.width, size.height),
            });
        }

        states
    }

    fn list_window_labels(&self) -> Vec<String> {
        self.webview_windows().keys().cloned().collect()
    }
}
