# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-04-26

### Added

- Workspace with 4 crates: victauri-core, victauri-macros, victauri-plugin, victauri-watchdog
- 29 MCP tools: eval_js, dom_snapshot, click, fill, type_text, press_key, get_window_state, list_windows, screenshot, invoke_command, get_ipc_log, get_registry, get_memory_stats, get_console_logs, verify_state, detect_ghost_commands, check_ipc_integrity, get_event_stream, resolve_command, assert_semantic, start_recording, stop_recording, checkpoint, list_checkpoints, get_replay_sequence, get_recorded_events, events_between_checkpoints
- 3 MCP resources: victauri://ipc-log, victauri://windows, victauri://state
- `#[inspectable]` proc macro for Tauri command instrumentation
- JS bridge with DOM walking, ref handles, console capture, mutation observer
- `VictauriBuilder` for port/capacity configuration
- `VICTAURI_PORT` environment variable support
- Release-safe: zero overhead in release builds via `#[cfg(debug_assertions)]`
- Cross-platform CI (Linux, Windows, macOS)
- 78 tests (44 core + 4 macro + 30 integration)
- Windows screenshot via PrintWindow + custom PNG encoder
- OS-level process memory stats (Windows `GetProcessMemoryInfo`, Linux `/proc/self/statm`)
- victauri-watchdog crash-recovery sidecar
- Demo app example

[0.1.0]: https://github.com/4da-systems/victauri/releases/tag/v0.1.0
