const COMMANDS: &[&str] = &[
    "victauri_eval_js",
    "victauri_eval_callback",
    "victauri_get_window_state",
    "victauri_list_windows",
    "victauri_get_ipc_log",
    "victauri_get_registry",
    "victauri_get_memory_stats",
    "victauri_dom_snapshot",
    "victauri_verify_state",
    "victauri_detect_ghost_commands",
    "victauri_check_ipc_integrity",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
