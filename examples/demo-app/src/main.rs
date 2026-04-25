#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::Manager;
use victauri_plugin::inspectable;

#[derive(Default)]
struct AppState {
    counter: Mutex<i32>,
    todos: Mutex<Vec<Todo>>,
    settings: Mutex<Settings>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Todo {
    id: u32,
    title: String,
    completed: bool,
}

#[derive(Clone, Serialize, Deserialize)]
struct Settings {
    theme: String,
    notifications_enabled: bool,
    language: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            notifications_enabled: true,
            language: "en".to_string(),
        }
    }
}

#[tauri::command]
#[inspectable(
    description = "Greet a person by name",
    intent = "generate greeting",
    category = "general",
    example = "greet someone"
)]
fn greet(name: String) -> String {
    format!("Hello, {name}! This response came from Rust.")
}

#[tauri::command]
#[inspectable(
    description = "Get the current counter value",
    intent = "read counter state",
    category = "counter"
)]
fn get_counter(state: tauri::State<'_, AppState>) -> i32 {
    *state.counter.lock().unwrap()
}

#[tauri::command]
#[inspectable(
    description = "Increment the counter by 1 and return the new value",
    intent = "increase counter",
    category = "counter",
    example = "increment the counter"
)]
fn increment(state: tauri::State<'_, AppState>) -> i32 {
    let mut count = state.counter.lock().unwrap();
    *count += 1;
    *count
}

#[tauri::command]
#[inspectable(
    description = "Decrement the counter by 1 and return the new value",
    intent = "decrease counter",
    category = "counter",
    example = "decrement the counter"
)]
fn decrement(state: tauri::State<'_, AppState>) -> i32 {
    let mut count = state.counter.lock().unwrap();
    *count -= 1;
    *count
}

#[tauri::command]
#[inspectable(
    description = "Reset the counter to zero",
    intent = "reset counter state",
    category = "counter",
    example = "reset the counter"
)]
fn reset_counter(state: tauri::State<'_, AppState>) -> i32 {
    let mut count = state.counter.lock().unwrap();
    *count = 0;
    *count
}

#[tauri::command]
#[inspectable(
    description = "Add a new todo item",
    intent = "create todo",
    category = "todos",
    example = "add a todo"
)]
fn add_todo(state: tauri::State<'_, AppState>, title: String) -> Todo {
    let mut todos = state.todos.lock().unwrap();
    let id = todos.len() as u32 + 1;
    let todo = Todo {
        id,
        title,
        completed: false,
    };
    todos.push(todo.clone());
    todo
}

#[tauri::command]
#[inspectable(
    description = "List all todo items",
    intent = "read todos",
    category = "todos",
    example = "show all todos"
)]
fn list_todos(state: tauri::State<'_, AppState>) -> Vec<Todo> {
    state.todos.lock().unwrap().clone()
}

#[tauri::command]
#[inspectable(
    description = "Toggle the completion status of a todo item",
    intent = "update todo status",
    category = "todos",
    example = "mark todo as done"
)]
fn toggle_todo(state: tauri::State<'_, AppState>, id: u32) -> Result<Todo, String> {
    let mut todos = state.todos.lock().unwrap();
    let todo = todos
        .iter_mut()
        .find(|t| t.id == id)
        .ok_or_else(|| format!("todo {id} not found"))?;
    todo.completed = !todo.completed;
    Ok(todo.clone())
}

#[tauri::command]
#[inspectable(
    description = "Delete a todo item by ID",
    intent = "remove todo",
    category = "todos",
    example = "delete a todo"
)]
fn delete_todo(state: tauri::State<'_, AppState>, id: u32) -> Result<(), String> {
    let mut todos = state.todos.lock().unwrap();
    let pos = todos
        .iter()
        .position(|t| t.id == id)
        .ok_or_else(|| format!("todo {id} not found"))?;
    todos.remove(pos);
    Ok(())
}

#[tauri::command]
#[inspectable(
    description = "Get current application settings",
    intent = "read settings",
    category = "settings",
    example = "show settings"
)]
fn get_settings(state: tauri::State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
#[inspectable(
    description = "Update application settings",
    intent = "modify settings",
    category = "settings",
    example = "change the theme"
)]
fn update_settings(
    state: tauri::State<'_, AppState>,
    theme: Option<String>,
    notifications_enabled: Option<bool>,
    language: Option<String>,
) -> Settings {
    let mut settings = state.settings.lock().unwrap();
    if let Some(t) = theme {
        settings.theme = t;
    }
    if let Some(n) = notifications_enabled {
        settings.notifications_enabled = n;
    }
    if let Some(l) = language {
        settings.language = l;
    }
    settings.clone()
}

#[tauri::command]
#[inspectable(
    description = "Get a summary of all application state for verification",
    intent = "read full state",
    category = "debug",
    example = "show app state"
)]
fn get_app_state(state: tauri::State<'_, AppState>) -> serde_json::Value {
    serde_json::json!({
        "counter": *state.counter.lock().unwrap(),
        "todos": *state.todos.lock().unwrap(),
        "settings": *state.settings.lock().unwrap(),
    })
}

fn main() {
    tauri::Builder::default()
        .plugin(victauri_plugin::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            greet,
            get_counter,
            increment,
            decrement,
            reset_counter,
            add_todo,
            list_todos,
            toggle_todo,
            delete_todo,
            get_settings,
            update_settings,
            get_app_state,
        ])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            window.set_title("Victauri Demo").unwrap();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
