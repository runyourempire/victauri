#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

#[tauri::command]
fn greet(name: String) -> String {
    format!("Hello, {name}! This response came from Rust.")
}

#[tauri::command]
fn get_counter(state: tauri::State<'_, CounterState>) -> i32 {
    *state.count.lock().unwrap()
}

#[tauri::command]
fn increment(state: tauri::State<'_, CounterState>) -> i32 {
    let mut count = state.count.lock().unwrap();
    *count += 1;
    *count
}

struct CounterState {
    count: std::sync::Mutex<i32>,
}

fn main() {
    tauri::Builder::default()
        .plugin(victauri_plugin::init())
        .manage(CounterState {
            count: std::sync::Mutex::new(0),
        })
        .invoke_handler(tauri::generate_handler![greet, get_counter, increment])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            window.set_title("Victauri Demo").unwrap();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
