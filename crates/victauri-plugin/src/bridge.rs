use tauri::{Manager, Runtime};
use victauri_core::WindowState;

pub trait WebviewBridge: Send + Sync {
    fn eval_webview(&self, label: Option<&str>, script: &str) -> Result<(), String>;
    fn get_window_states(&self, label: Option<&str>) -> Vec<WindowState>;
    fn list_window_labels(&self) -> Vec<String>;
    fn get_native_handle(&self, label: Option<&str>) -> Result<isize, String>;
    fn manage_window(&self, label: Option<&str>, action: &str) -> Result<String, String>;
    fn resize_window(&self, label: Option<&str>, width: u32, height: u32) -> Result<(), String>;
    fn move_window(&self, label: Option<&str>, x: i32, y: i32) -> Result<(), String>;
    fn set_window_title(&self, label: Option<&str>, title: &str) -> Result<(), String>;
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
                .or_else(|| windows.values().find(|w| w.is_visible().unwrap_or(false)))
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

    fn get_native_handle(&self, label: Option<&str>) -> Result<isize, String> {
        let windows = self.webview_windows();
        let _webview = match label {
            Some(l) => windows
                .get(l)
                .ok_or_else(|| format!("window not found: {l}"))?,
            None => windows
                .get("main")
                .or_else(|| windows.values().find(|w| w.is_visible().unwrap_or(false)))
                .or_else(|| windows.values().next())
                .ok_or_else(|| "no webview available".to_string())?,
        };

        #[cfg(windows)]
        {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            let handle = _webview.window_handle().map_err(|e| e.to_string())?;
            match handle.as_raw() {
                RawWindowHandle::Win32(h) => Ok(h.hwnd.get()),
                _ => Err("unexpected window handle type".to_string()),
            }
        }

        #[cfg(not(windows))]
        {
            Err("native handle not yet supported on this platform".to_string())
        }
    }

    fn manage_window(&self, label: Option<&str>, action: &str) -> Result<String, String> {
        let windows = self.webview_windows();
        let window = match label {
            Some(l) => windows
                .get(l)
                .ok_or_else(|| format!("window not found: {l}"))?,
            None => windows
                .get("main")
                .or_else(|| windows.values().find(|w| w.is_visible().unwrap_or(false)))
                .or_else(|| windows.values().next())
                .ok_or_else(|| "no window available".to_string())?,
        };

        match action {
            "minimize" => window.minimize().map_err(|e| e.to_string())?,
            "unminimize" => window.unminimize().map_err(|e| e.to_string())?,
            "maximize" => window.maximize().map_err(|e| e.to_string())?,
            "unmaximize" => window.unmaximize().map_err(|e| e.to_string())?,
            "close" => window.close().map_err(|e| e.to_string())?,
            "focus" => window.set_focus().map_err(|e| e.to_string())?,
            "show" => window.show().map_err(|e| e.to_string())?,
            "hide" => window.hide().map_err(|e| e.to_string())?,
            "fullscreen" => window.set_fullscreen(true).map_err(|e| e.to_string())?,
            "unfullscreen" => window.set_fullscreen(false).map_err(|e| e.to_string())?,
            "always_on_top" => window.set_always_on_top(true).map_err(|e| e.to_string())?,
            "not_always_on_top" => window.set_always_on_top(false).map_err(|e| e.to_string())?,
            _ => return Err(format!("unknown action: {action}")),
        }

        Ok(format!("{action} executed"))
    }

    fn resize_window(&self, label: Option<&str>, width: u32, height: u32) -> Result<(), String> {
        let windows = self.webview_windows();
        let window = match label {
            Some(l) => windows
                .get(l)
                .ok_or_else(|| format!("window not found: {l}"))?,
            None => windows
                .get("main")
                .or_else(|| windows.values().find(|w| w.is_visible().unwrap_or(false)))
                .or_else(|| windows.values().next())
                .ok_or_else(|| "no window available".to_string())?,
        };

        window
            .set_size(tauri::LogicalSize::new(width, height))
            .map_err(|e| e.to_string())
    }

    fn move_window(&self, label: Option<&str>, x: i32, y: i32) -> Result<(), String> {
        let windows = self.webview_windows();
        let window = match label {
            Some(l) => windows
                .get(l)
                .ok_or_else(|| format!("window not found: {l}"))?,
            None => windows
                .get("main")
                .or_else(|| windows.values().find(|w| w.is_visible().unwrap_or(false)))
                .or_else(|| windows.values().next())
                .ok_or_else(|| "no window available".to_string())?,
        };

        window
            .set_position(tauri::LogicalPosition::new(x, y))
            .map_err(|e| e.to_string())
    }

    fn set_window_title(&self, label: Option<&str>, title: &str) -> Result<(), String> {
        let windows = self.webview_windows();
        let window = match label {
            Some(l) => windows
                .get(l)
                .ok_or_else(|| format!("window not found: {l}"))?,
            None => windows
                .get("main")
                .or_else(|| windows.values().find(|w| w.is_visible().unwrap_or(false)))
                .or_else(|| windows.values().next())
                .ok_or_else(|| "no window available".to_string())?,
        };

        window.set_title(title).map_err(|e| e.to_string())
    }
}
