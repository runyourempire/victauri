use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use victauri_core::*;

#[test]
fn event_log_push_and_snapshot() {
    let log = EventLog::new(100);
    assert!(log.is_empty());

    log.push(AppEvent::Ipc(IpcCall {
        id: "1".to_string(),
        command: "test_cmd".to_string(),
        timestamp: Utc::now(),
        duration_ms: Some(5),
        result: event::IpcResult::Ok(serde_json::json!(42)),
        arg_size_bytes: 10,
        webview_label: "main".to_string(),
    }));

    assert_eq!(log.len(), 1);
    assert!(!log.is_empty());

    let snapshot = log.snapshot();
    assert_eq!(snapshot.len(), 1);
    match &snapshot[0] {
        AppEvent::Ipc(call) => {
            assert_eq!(call.command, "test_cmd");
            assert_eq!(call.webview_label, "main");
        }
        _ => panic!("expected Ipc event"),
    }
}

#[test]
fn event_log_ring_buffer_eviction() {
    let log = EventLog::new(3);

    for i in 0..5 {
        log.push(AppEvent::Ipc(IpcCall {
            id: i.to_string(),
            command: format!("cmd_{i}"),
            timestamp: Utc::now(),
            duration_ms: None,
            result: event::IpcResult::Pending,
            arg_size_bytes: 0,
            webview_label: "main".to_string(),
        }));
    }

    assert_eq!(log.len(), 3);
    let calls = log.ipc_calls();
    assert_eq!(calls[0].command, "cmd_2");
    assert_eq!(calls[1].command, "cmd_3");
    assert_eq!(calls[2].command, "cmd_4");
}

#[test]
fn event_log_ipc_calls_filter() {
    let log = EventLog::new(100);

    log.push(AppEvent::Ipc(IpcCall {
        id: "1".to_string(),
        command: "save".to_string(),
        timestamp: Utc::now(),
        duration_ms: None,
        result: event::IpcResult::Pending,
        arg_size_bytes: 0,
        webview_label: "main".to_string(),
    }));

    log.push(AppEvent::StateChange {
        key: "user".to_string(),
        timestamp: Utc::now(),
        caused_by: None,
    });

    log.push(AppEvent::Ipc(IpcCall {
        id: "2".to_string(),
        command: "load".to_string(),
        timestamp: Utc::now(),
        duration_ms: Some(10),
        result: event::IpcResult::Ok(serde_json::json!("data")),
        arg_size_bytes: 5,
        webview_label: "main".to_string(),
    }));

    assert_eq!(log.len(), 3);
    let ipc_only = log.ipc_calls();
    assert_eq!(ipc_only.len(), 2);
    assert_eq!(ipc_only[0].command, "save");
    assert_eq!(ipc_only[1].command, "load");
}

#[test]
fn event_log_clear() {
    let log = EventLog::new(100);
    log.push(AppEvent::WindowEvent {
        label: "main".to_string(),
        event: "focus".to_string(),
        timestamp: Utc::now(),
    });
    assert_eq!(log.len(), 1);
    log.clear();
    assert!(log.is_empty());
}

#[test]
fn command_registry_register_and_list() {
    let registry = CommandRegistry::new();
    assert_eq!(registry.count(), 0);

    registry.register(CommandInfo {
        name: "save_file".to_string(),
        plugin: None,
        description: Some("Save a file to disk".to_string()),
        args: vec![CommandArg {
            name: "path".to_string(),
            type_name: "String".to_string(),
            required: true,
            schema: None,
        }],
        return_type: Some("Result<(), String>".to_string()),
        is_async: true,
        intent: None,
        category: None,
        examples: vec![],
    });

    assert_eq!(registry.count(), 1);
    let cmd = registry.get("save_file").unwrap();
    assert_eq!(cmd.description.as_deref(), Some("Save a file to disk"));
    assert_eq!(cmd.args.len(), 1);
    assert!(cmd.is_async);
}

#[test]
fn command_registry_search() {
    let registry = CommandRegistry::new();

    registry.register(CommandInfo {
        name: "get_users".to_string(),
        plugin: None,
        description: Some("Fetch all users".to_string()),
        args: vec![],
        return_type: None,
        is_async: false,
        intent: None,
        category: None,
        examples: vec![],
    });

    registry.register(CommandInfo {
        name: "save_settings".to_string(),
        plugin: None,
        description: Some("Save app settings".to_string()),
        args: vec![],
        return_type: None,
        is_async: false,
        intent: None,
        category: None,
        examples: vec![],
    });

    registry.register(CommandInfo {
        name: "delete_user".to_string(),
        plugin: None,
        description: Some("Remove a user".to_string()),
        args: vec![],
        return_type: None,
        is_async: false,
        intent: None,
        category: None,
        examples: vec![],
    });

    let results = registry.search("user");
    assert_eq!(results.len(), 2);

    let results = registry.search("save");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "save_settings");

    let results = registry.search("nonexistent");
    assert!(results.is_empty());
}

#[test]
fn dom_snapshot_accessible_text() {
    let snapshot = DomSnapshot {
        webview_label: "main".to_string(),
        elements: vec![DomElement {
            ref_id: "e0".to_string(),
            tag: "div".to_string(),
            role: Some("main".to_string()),
            name: Some("Content".to_string()),
            text: None,
            value: None,
            enabled: true,
            visible: true,
            focusable: false,
            bounds: None,
            children: vec![
                DomElement {
                    ref_id: "e1".to_string(),
                    tag: "button".to_string(),
                    role: Some("button".to_string()),
                    name: Some("Submit".to_string()),
                    text: Some("Submit".to_string()),
                    value: None,
                    enabled: true,
                    visible: true,
                    focusable: true,
                    bounds: None,
                    children: vec![],
                    attributes: HashMap::new(),
                },
                DomElement {
                    ref_id: "e2".to_string(),
                    tag: "input".to_string(),
                    role: Some("textbox".to_string()),
                    name: Some("Email".to_string()),
                    text: None,
                    value: None,
                    enabled: true,
                    visible: true,
                    focusable: true,
                    bounds: None,
                    children: vec![],
                    attributes: HashMap::new(),
                },
            ],
            attributes: HashMap::new(),
        }],
        ref_map: HashMap::new(),
    };

    let text = snapshot.to_accessible_text(0);
    assert!(text.contains("main \"Content\""));
    assert!(text.contains("button \"Submit\" [ref=e1]"));
    assert!(text.contains("textbox \"Email\" [ref=e2]"));
}

#[test]
fn dom_snapshot_hides_invisible() {
    let snapshot = DomSnapshot {
        webview_label: "main".to_string(),
        elements: vec![DomElement {
            ref_id: "e0".to_string(),
            tag: "div".to_string(),
            role: None,
            name: None,
            text: None,
            value: None,
            enabled: true,
            visible: false,
            focusable: false,
            bounds: None,
            children: vec![],
            attributes: HashMap::new(),
        }],
        ref_map: HashMap::new(),
    };

    let text = snapshot.to_accessible_text(0);
    assert!(text.is_empty());
}

#[test]
fn window_state_serialization() {
    let state = WindowState {
        label: "main".to_string(),
        title: "My App".to_string(),
        url: "tauri://localhost".to_string(),
        visible: true,
        focused: true,
        maximized: false,
        minimized: false,
        fullscreen: false,
        position: (100, 200),
        size: (800, 600),
    };

    let json = serde_json::to_string(&state).unwrap();
    let deserialized: WindowState = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.label, "main");
    assert_eq!(deserialized.size, (800, 600));
    assert!(deserialized.visible);
}

#[test]
fn verification_result_with_divergences() {
    use victauri_core::types::{Divergence, DivergenceSeverity, VerificationResult};

    let result = VerificationResult {
        passed: false,
        frontend_state: serde_json::json!({"count": 5}),
        backend_state: serde_json::json!({"count": 3}),
        divergences: vec![Divergence {
            path: "count".to_string(),
            frontend_value: serde_json::json!(5),
            backend_value: serde_json::json!(3),
            severity: DivergenceSeverity::Error,
        }],
    };

    assert!(!result.passed);
    assert_eq!(result.divergences.len(), 1);
    assert_eq!(result.divergences[0].path, "count");
}

// ── Phase 2: Cross-boundary state verification ─────────────────────────────

#[test]
fn verify_state_identical() {
    let state = serde_json::json!({"count": 5, "name": "test"});
    let result = victauri_core::verify_state(state.clone(), state);
    assert!(result.passed);
    assert!(result.divergences.is_empty());
}

#[test]
fn verify_state_scalar_divergence() {
    let frontend = serde_json::json!({"count": 5});
    let backend = serde_json::json!({"count": 3});
    let result = victauri_core::verify_state(frontend, backend);

    assert!(!result.passed);
    assert_eq!(result.divergences.len(), 1);
    assert_eq!(result.divergences[0].path, "count");
    assert_eq!(result.divergences[0].frontend_value, serde_json::json!(5));
    assert_eq!(result.divergences[0].backend_value, serde_json::json!(3));
}

#[test]
fn verify_state_missing_keys() {
    let frontend = serde_json::json!({"a": 1, "b": 2});
    let backend = serde_json::json!({"b": 2, "c": 3});
    let result = victauri_core::verify_state(frontend, backend);

    assert!(!result.passed);
    assert_eq!(result.divergences.len(), 2);
    let paths: Vec<&str> = result.divergences.iter().map(|d| d.path.as_str()).collect();
    assert!(paths.contains(&"a"));
    assert!(paths.contains(&"c"));
}

#[test]
fn verify_state_nested_objects() {
    let frontend = serde_json::json!({"user": {"name": "Alice", "age": 30}});
    let backend = serde_json::json!({"user": {"name": "Alice", "age": 31}});
    let result = victauri_core::verify_state(frontend, backend);

    assert!(!result.passed);
    assert_eq!(result.divergences.len(), 1);
    assert_eq!(result.divergences[0].path, "user.age");
}

#[test]
fn verify_state_array_divergence() {
    let frontend = serde_json::json!({"items": [1, 2, 3]});
    let backend = serde_json::json!({"items": [1, 2, 4]});
    let result = victauri_core::verify_state(frontend, backend);

    assert!(!result.passed);
    assert_eq!(result.divergences.len(), 1);
    assert_eq!(result.divergences[0].path, "items[2]");
}

#[test]
fn verify_state_array_length_mismatch() {
    let frontend = serde_json::json!({"items": [1, 2, 3]});
    let backend = serde_json::json!({"items": [1, 2]});
    let result = victauri_core::verify_state(frontend, backend);

    assert!(!result.passed);
    assert_eq!(result.divergences.len(), 1);
    assert_eq!(result.divergences[0].path, "items[2]");
    assert_eq!(result.divergences[0].backend_value, serde_json::Value::Null);
}

#[test]
fn verify_state_type_mismatch() {
    let frontend = serde_json::json!({"value": "five"});
    let backend = serde_json::json!({"value": 5});
    let result = victauri_core::verify_state(frontend, backend);

    assert!(!result.passed);
    assert_eq!(result.divergences.len(), 1);
    matches!(
        result.divergences[0].severity,
        victauri_core::types::DivergenceSeverity::Error
    );
}

// ── Phase 2: Ghost command detection ────────────────────────────────────────

#[test]
fn ghost_commands_all_matched() {
    let registry = CommandRegistry::new();
    registry.register(CommandInfo {
        name: "save".to_string(),
        plugin: None,
        description: None,
        args: vec![],
        return_type: None,
        is_async: false,
        intent: None,
        category: None,
        examples: vec![],
    });
    registry.register(CommandInfo {
        name: "load".to_string(),
        plugin: None,
        description: None,
        args: vec![],
        return_type: None,
        is_async: false,
        intent: None,
        category: None,
        examples: vec![],
    });

    let frontend = vec!["save".to_string(), "load".to_string()];
    let report = victauri_core::detect_ghost_commands(&frontend, &registry);

    assert!(report.ghost_commands.is_empty());
    assert_eq!(report.total_frontend_commands, 2);
    assert_eq!(report.total_registry_commands, 2);
}

#[test]
fn ghost_commands_frontend_only() {
    let registry = CommandRegistry::new();
    registry.register(CommandInfo {
        name: "save".to_string(),
        plugin: None,
        description: None,
        args: vec![],
        return_type: None,
        is_async: false,
        intent: None,
        category: None,
        examples: vec![],
    });

    let frontend = vec!["save".to_string(), "unknown_cmd".to_string()];
    let report = victauri_core::detect_ghost_commands(&frontend, &registry);

    assert_eq!(report.ghost_commands.len(), 1);
    assert_eq!(report.ghost_commands[0].name, "unknown_cmd");
    matches!(
        report.ghost_commands[0].source,
        victauri_core::GhostSource::FrontendOnly
    );
}

#[test]
fn ghost_commands_registry_only() {
    let registry = CommandRegistry::new();
    registry.register(CommandInfo {
        name: "save".to_string(),
        plugin: None,
        description: Some("Save data".to_string()),
        args: vec![],
        return_type: None,
        is_async: false,
        intent: None,
        category: None,
        examples: vec![],
    });
    registry.register(CommandInfo {
        name: "unused_cmd".to_string(),
        plugin: None,
        description: Some("Never called".to_string()),
        args: vec![],
        return_type: None,
        is_async: false,
        intent: None,
        category: None,
        examples: vec![],
    });

    let frontend = vec!["save".to_string()];
    let report = victauri_core::detect_ghost_commands(&frontend, &registry);

    assert_eq!(report.ghost_commands.len(), 1);
    assert_eq!(report.ghost_commands[0].name, "unused_cmd");
    matches!(
        report.ghost_commands[0].source,
        victauri_core::GhostSource::RegistryOnly
    );
}

#[test]
fn ghost_commands_bidirectional() {
    let registry = CommandRegistry::new();
    registry.register(CommandInfo {
        name: "shared".to_string(),
        plugin: None,
        description: None,
        args: vec![],
        return_type: None,
        is_async: false,
        intent: None,
        category: None,
        examples: vec![],
    });
    registry.register(CommandInfo {
        name: "backend_only".to_string(),
        plugin: None,
        description: None,
        args: vec![],
        return_type: None,
        is_async: false,
        intent: None,
        category: None,
        examples: vec![],
    });

    let frontend = vec!["shared".to_string(), "frontend_only".to_string()];
    let report = victauri_core::detect_ghost_commands(&frontend, &registry);

    assert_eq!(report.ghost_commands.len(), 2);
    let names: Vec<&str> = report
        .ghost_commands
        .iter()
        .map(|g| g.name.as_str())
        .collect();
    assert!(names.contains(&"backend_only"));
    assert!(names.contains(&"frontend_only"));
}

// ── Phase 2: IPC round-trip integrity ───────────────────────────────────────

#[test]
fn ipc_integrity_healthy() {
    let log = EventLog::new(100);
    log.push(AppEvent::Ipc(IpcCall {
        id: "1".to_string(),
        command: "save".to_string(),
        timestamp: Utc::now(),
        duration_ms: Some(5),
        result: event::IpcResult::Ok(serde_json::json!("ok")),
        arg_size_bytes: 10,
        webview_label: "main".to_string(),
    }));
    log.push(AppEvent::Ipc(IpcCall {
        id: "2".to_string(),
        command: "load".to_string(),
        timestamp: Utc::now(),
        duration_ms: Some(3),
        result: event::IpcResult::Ok(serde_json::json!("data")),
        arg_size_bytes: 0,
        webview_label: "main".to_string(),
    }));

    let report = victauri_core::check_ipc_integrity(&log, 5000);
    assert!(report.healthy);
    assert_eq!(report.total_calls, 2);
    assert_eq!(report.completed, 2);
    assert_eq!(report.pending, 0);
    assert_eq!(report.errored, 0);
    assert!(report.stale_calls.is_empty());
    assert!(report.error_calls.is_empty());
}

#[test]
fn ipc_integrity_with_errors() {
    let log = EventLog::new(100);
    log.push(AppEvent::Ipc(IpcCall {
        id: "1".to_string(),
        command: "save".to_string(),
        timestamp: Utc::now(),
        duration_ms: Some(5),
        result: event::IpcResult::Err("permission denied".to_string()),
        arg_size_bytes: 10,
        webview_label: "main".to_string(),
    }));

    let report = victauri_core::check_ipc_integrity(&log, 5000);
    assert!(!report.healthy);
    assert_eq!(report.errored, 1);
    assert_eq!(report.error_calls.len(), 1);
    assert_eq!(report.error_calls[0].error, "permission denied");
    assert_eq!(report.error_calls[0].command, "save");
}

#[test]
fn ipc_integrity_stale_pending() {
    let log = EventLog::new(100);
    let old_timestamp = Utc::now() - chrono::Duration::seconds(10);
    log.push(AppEvent::Ipc(IpcCall {
        id: "1".to_string(),
        command: "slow_cmd".to_string(),
        timestamp: old_timestamp,
        duration_ms: None,
        result: event::IpcResult::Pending,
        arg_size_bytes: 0,
        webview_label: "main".to_string(),
    }));

    let report = victauri_core::check_ipc_integrity(&log, 5000);
    assert!(!report.healthy);
    assert_eq!(report.pending, 1);
    assert_eq!(report.stale_calls.len(), 1);
    assert_eq!(report.stale_calls[0].command, "slow_cmd");
    assert!(report.stale_calls[0].age_ms >= 9000);
}

#[test]
fn ipc_integrity_recent_pending_not_stale() {
    let log = EventLog::new(100);
    log.push(AppEvent::Ipc(IpcCall {
        id: "1".to_string(),
        command: "fast_cmd".to_string(),
        timestamp: Utc::now(),
        duration_ms: None,
        result: event::IpcResult::Pending,
        arg_size_bytes: 0,
        webview_label: "main".to_string(),
    }));

    let report = victauri_core::check_ipc_integrity(&log, 5000);
    assert!(report.healthy);
    assert_eq!(report.pending, 1);
    assert!(report.stale_calls.is_empty());
}

#[test]
fn ipc_integrity_mixed_status() {
    let log = EventLog::new(100);
    let old = Utc::now() - chrono::Duration::seconds(30);

    log.push(AppEvent::Ipc(IpcCall {
        id: "1".to_string(),
        command: "ok_cmd".to_string(),
        timestamp: Utc::now(),
        duration_ms: Some(1),
        result: event::IpcResult::Ok(serde_json::json!(null)),
        arg_size_bytes: 0,
        webview_label: "main".to_string(),
    }));
    log.push(AppEvent::Ipc(IpcCall {
        id: "2".to_string(),
        command: "stuck_cmd".to_string(),
        timestamp: old,
        duration_ms: None,
        result: event::IpcResult::Pending,
        arg_size_bytes: 0,
        webview_label: "main".to_string(),
    }));
    log.push(AppEvent::Ipc(IpcCall {
        id: "3".to_string(),
        command: "err_cmd".to_string(),
        timestamp: Utc::now(),
        duration_ms: Some(2),
        result: event::IpcResult::Err("boom".to_string()),
        arg_size_bytes: 0,
        webview_label: "main".to_string(),
    }));

    let report = victauri_core::check_ipc_integrity(&log, 5000);
    assert!(!report.healthy);
    assert_eq!(report.total_calls, 3);
    assert_eq!(report.completed, 1);
    assert_eq!(report.pending, 1);
    assert_eq!(report.errored, 1);
    assert_eq!(report.stale_calls.len(), 1);
    assert_eq!(report.error_calls.len(), 1);
}

// ── Phase 4: Intent annotations and NL resolution ──────────────────────────

#[test]
fn command_info_with_intent_fields() {
    let registry = CommandRegistry::new();
    registry.register(CommandInfo {
        name: "save_settings".to_string(),
        plugin: None,
        description: Some("Persist user settings".to_string()),
        args: vec![],
        return_type: None,
        is_async: true,
        intent: Some("persist user preferences to storage".to_string()),
        category: Some("settings".to_string()),
        examples: vec![
            "save my settings".to_string(),
            "persist preferences".to_string(),
        ],
    });

    let cmd = registry.get("save_settings").unwrap();
    assert_eq!(
        cmd.intent.as_deref(),
        Some("persist user preferences to storage")
    );
    assert_eq!(cmd.category.as_deref(), Some("settings"));
    assert_eq!(cmd.examples.len(), 2);
}

#[test]
fn resolve_command_by_name() {
    let registry = CommandRegistry::new();
    registry.register(CommandInfo {
        name: "save_file".to_string(),
        plugin: None,
        description: Some("Save a file to disk".to_string()),
        args: vec![],
        return_type: None,
        is_async: false,
        intent: None,
        category: None,
        examples: vec![],
    });
    registry.register(CommandInfo {
        name: "delete_file".to_string(),
        plugin: None,
        description: Some("Delete a file".to_string()),
        args: vec![],
        return_type: None,
        is_async: false,
        intent: None,
        category: None,
        examples: vec![],
    });

    let results = registry.resolve("save file");
    assert!(!results.is_empty());
    assert_eq!(results[0].command.name, "save_file");
    assert!(results[0].score > results.get(1).map_or(0.0, |r| r.score));
}

#[test]
fn resolve_command_by_intent() {
    let registry = CommandRegistry::new();
    registry.register(CommandInfo {
        name: "update_profile".to_string(),
        plugin: None,
        description: Some("Update user profile".to_string()),
        args: vec![],
        return_type: None,
        is_async: false,
        intent: Some("modify the current user's profile information".to_string()),
        category: Some("user".to_string()),
        examples: vec!["change my name".to_string()],
    });
    registry.register(CommandInfo {
        name: "get_profile".to_string(),
        plugin: None,
        description: Some("Fetch user profile".to_string()),
        args: vec![],
        return_type: None,
        is_async: false,
        intent: Some("retrieve the current user's profile data".to_string()),
        category: Some("user".to_string()),
        examples: vec![],
    });

    let results = registry.resolve("modify profile");
    assert!(!results.is_empty());
    assert_eq!(results[0].command.name, "update_profile");
}

#[test]
fn resolve_command_by_example() {
    let registry = CommandRegistry::new();
    registry.register(CommandInfo {
        name: "export_data".to_string(),
        plugin: None,
        description: Some("Export data to CSV".to_string()),
        args: vec![],
        return_type: None,
        is_async: false,
        intent: None,
        category: None,
        examples: vec!["download my data as csv".to_string()],
    });

    let results = registry.resolve("download my data as csv");
    assert!(!results.is_empty());
    assert_eq!(results[0].command.name, "export_data");
}

#[test]
fn resolve_command_no_match() {
    let registry = CommandRegistry::new();
    registry.register(CommandInfo {
        name: "save".to_string(),
        plugin: None,
        description: Some("Save data".to_string()),
        args: vec![],
        return_type: None,
        is_async: false,
        intent: None,
        category: None,
        examples: vec![],
    });

    let results = registry.resolve("zzz_nonexistent_zzz");
    assert!(results.is_empty());
}

// ── Phase 4: Semantic assertions ────────────────────────────────────────────

#[test]
fn semantic_assertion_equals() {
    let assertion = victauri_core::SemanticAssertion {
        label: "count is 5".to_string(),
        condition: "equals".to_string(),
        expected: serde_json::json!(5),
    };

    let pass = victauri_core::evaluate_assertion(serde_json::json!(5), &assertion);
    assert!(pass.passed);
    assert!(pass.message.is_none());

    let fail = victauri_core::evaluate_assertion(serde_json::json!(3), &assertion);
    assert!(!fail.passed);
    assert!(fail.message.is_some());
}

#[test]
fn semantic_assertion_truthy_falsy() {
    let truthy = victauri_core::SemanticAssertion {
        label: "value is truthy".to_string(),
        condition: "truthy".to_string(),
        expected: serde_json::Value::Null,
    };

    assert!(victauri_core::evaluate_assertion(serde_json::json!(true), &truthy).passed);
    assert!(victauri_core::evaluate_assertion(serde_json::json!("hello"), &truthy).passed);
    assert!(victauri_core::evaluate_assertion(serde_json::json!(42), &truthy).passed);
    assert!(!victauri_core::evaluate_assertion(serde_json::Value::Null, &truthy).passed);

    let falsy = victauri_core::SemanticAssertion {
        label: "value is falsy".to_string(),
        condition: "falsy".to_string(),
        expected: serde_json::Value::Null,
    };

    assert!(victauri_core::evaluate_assertion(serde_json::Value::Null, &falsy).passed);
    assert!(victauri_core::evaluate_assertion(serde_json::json!(false), &falsy).passed);
    assert!(victauri_core::evaluate_assertion(serde_json::json!(0), &falsy).passed);
    assert!(!victauri_core::evaluate_assertion(serde_json::json!(1), &falsy).passed);
}

#[test]
fn semantic_assertion_contains() {
    let assertion = victauri_core::SemanticAssertion {
        label: "string contains hello".to_string(),
        condition: "contains".to_string(),
        expected: serde_json::json!("hello"),
    };

    assert!(
        victauri_core::evaluate_assertion(serde_json::json!("say hello world"), &assertion).passed
    );
    assert!(!victauri_core::evaluate_assertion(serde_json::json!("goodbye"), &assertion).passed);
}

#[test]
fn semantic_assertion_comparisons() {
    let gt = victauri_core::SemanticAssertion {
        label: "greater than 10".to_string(),
        condition: "greater_than".to_string(),
        expected: serde_json::json!(10),
    };

    assert!(victauri_core::evaluate_assertion(serde_json::json!(15), &gt).passed);
    assert!(!victauri_core::evaluate_assertion(serde_json::json!(5), &gt).passed);

    let lt = victauri_core::SemanticAssertion {
        label: "less than 10".to_string(),
        condition: "less_than".to_string(),
        expected: serde_json::json!(10),
    };

    assert!(victauri_core::evaluate_assertion(serde_json::json!(5), &lt).passed);
    assert!(!victauri_core::evaluate_assertion(serde_json::json!(15), &lt).passed);
}

#[test]
fn semantic_assertion_type_is() {
    let assertion = victauri_core::SemanticAssertion {
        label: "is a string".to_string(),
        condition: "type_is".to_string(),
        expected: serde_json::json!("string"),
    };

    assert!(victauri_core::evaluate_assertion(serde_json::json!("hello"), &assertion).passed);
    assert!(!victauri_core::evaluate_assertion(serde_json::json!(42), &assertion).passed);
}

#[test]
fn semantic_assertion_exists() {
    let assertion = victauri_core::SemanticAssertion {
        label: "value exists".to_string(),
        condition: "exists".to_string(),
        expected: serde_json::Value::Null,
    };

    assert!(victauri_core::evaluate_assertion(serde_json::json!("something"), &assertion).passed);
    assert!(!victauri_core::evaluate_assertion(serde_json::Value::Null, &assertion).passed);
}

// ── Phase 5: Time-Travel (recording, checkpoints, replay) ──────────────────

#[test]
fn recorder_start_stop() {
    let recorder = EventRecorder::new(1000);
    assert!(!recorder.is_recording());

    assert!(recorder.start("session-1".to_string()));
    assert!(recorder.is_recording());
    assert!(!recorder.start("session-2".to_string()));

    let session = recorder.stop().unwrap();
    assert_eq!(session.id, "session-1");
    assert!(session.events.is_empty());
    assert!(!recorder.is_recording());
}

#[test]
fn recorder_record_events() {
    let recorder = EventRecorder::new(1000);
    recorder.start("s1".to_string());

    recorder.record_event(AppEvent::Ipc(IpcCall {
        id: "1".to_string(),
        command: "save".to_string(),
        timestamp: Utc::now(),
        duration_ms: Some(5),
        result: event::IpcResult::Ok(serde_json::json!("ok")),
        arg_size_bytes: 10,
        webview_label: "main".to_string(),
    }));

    recorder.record_event(AppEvent::StateChange {
        key: "user".to_string(),
        timestamp: Utc::now(),
        caused_by: Some("save".to_string()),
    });

    assert_eq!(recorder.event_count(), 2);

    let session = recorder.stop().unwrap();
    assert_eq!(session.events.len(), 2);
    assert_eq!(session.events[0].index, 0);
    assert_eq!(session.events[1].index, 1);
}

#[test]
fn recorder_checkpoints() {
    let recorder = EventRecorder::new(1000);
    assert!(!recorder.checkpoint("cp1".to_string(), None, serde_json::json!({})));

    recorder.start("s1".to_string());

    recorder.record_event(AppEvent::Ipc(IpcCall {
        id: "1".to_string(),
        command: "load".to_string(),
        timestamp: Utc::now(),
        duration_ms: None,
        result: event::IpcResult::Pending,
        arg_size_bytes: 0,
        webview_label: "main".to_string(),
    }));

    assert!(recorder.checkpoint(
        "before-save".to_string(),
        Some("Before saving".to_string()),
        serde_json::json!({"count": 0}),
    ));

    recorder.record_event(AppEvent::Ipc(IpcCall {
        id: "2".to_string(),
        command: "save".to_string(),
        timestamp: Utc::now(),
        duration_ms: Some(5),
        result: event::IpcResult::Ok(serde_json::json!("ok")),
        arg_size_bytes: 10,
        webview_label: "main".to_string(),
    }));

    assert!(recorder.checkpoint(
        "after-save".to_string(),
        Some("After saving".to_string()),
        serde_json::json!({"count": 1}),
    ));

    assert_eq!(recorder.checkpoint_count(), 2);

    let checkpoints = recorder.get_checkpoints();
    assert_eq!(checkpoints.len(), 2);
    assert_eq!(checkpoints[0].id, "before-save");
    assert_eq!(checkpoints[1].id, "after-save");
    assert_eq!(checkpoints[0].state, serde_json::json!({"count": 0}));
    assert_eq!(checkpoints[1].state, serde_json::json!({"count": 1}));
}

#[test]
fn recorder_events_since() {
    let recorder = EventRecorder::new(1000);
    recorder.start("s1".to_string());

    for i in 0..5 {
        recorder.record_event(AppEvent::Ipc(IpcCall {
            id: i.to_string(),
            command: format!("cmd_{i}"),
            timestamp: Utc::now(),
            duration_ms: None,
            result: event::IpcResult::Pending,
            arg_size_bytes: 0,
            webview_label: "main".to_string(),
        }));
    }

    let since_3 = recorder.events_since(3);
    assert_eq!(since_3.len(), 2);
    assert_eq!(since_3[0].index, 3);
    assert_eq!(since_3[1].index, 4);
}

#[test]
fn recorder_events_between_checkpoints() {
    let recorder = EventRecorder::new(1000);
    recorder.start("s1".to_string());

    recorder.record_event(AppEvent::Ipc(IpcCall {
        id: "0".to_string(),
        command: "before".to_string(),
        timestamp: Utc::now(),
        duration_ms: None,
        result: event::IpcResult::Pending,
        arg_size_bytes: 0,
        webview_label: "main".to_string(),
    }));

    recorder.checkpoint("cp1".to_string(), None, serde_json::json!(null));

    recorder.record_event(AppEvent::Ipc(IpcCall {
        id: "1".to_string(),
        command: "between".to_string(),
        timestamp: Utc::now(),
        duration_ms: None,
        result: event::IpcResult::Pending,
        arg_size_bytes: 0,
        webview_label: "main".to_string(),
    }));

    recorder.record_event(AppEvent::Ipc(IpcCall {
        id: "2".to_string(),
        command: "between2".to_string(),
        timestamp: Utc::now(),
        duration_ms: None,
        result: event::IpcResult::Pending,
        arg_size_bytes: 0,
        webview_label: "main".to_string(),
    }));

    recorder.checkpoint("cp2".to_string(), None, serde_json::json!(null));

    let between = recorder.events_between_checkpoints("cp1", "cp2").unwrap();
    assert_eq!(between.len(), 2);

    assert!(
        recorder
            .events_between_checkpoints("cp1", "nonexistent")
            .is_none()
    );
}

#[test]
fn recorder_ipc_replay_sequence() {
    let recorder = EventRecorder::new(1000);
    recorder.start("s1".to_string());

    recorder.record_event(AppEvent::Ipc(IpcCall {
        id: "1".to_string(),
        command: "save".to_string(),
        timestamp: Utc::now(),
        duration_ms: Some(5),
        result: event::IpcResult::Ok(serde_json::json!("ok")),
        arg_size_bytes: 10,
        webview_label: "main".to_string(),
    }));

    recorder.record_event(AppEvent::StateChange {
        key: "user".to_string(),
        timestamp: Utc::now(),
        caused_by: None,
    });

    recorder.record_event(AppEvent::Ipc(IpcCall {
        id: "2".to_string(),
        command: "load".to_string(),
        timestamp: Utc::now(),
        duration_ms: Some(3),
        result: event::IpcResult::Ok(serde_json::json!("data")),
        arg_size_bytes: 0,
        webview_label: "main".to_string(),
    }));

    let replay = recorder.ipc_replay_sequence();
    assert_eq!(replay.len(), 2);
    assert_eq!(replay[0].command, "save");
    assert_eq!(replay[1].command, "load");
}

#[test]
fn recorder_not_recording_returns_empty() {
    let recorder = EventRecorder::new(1000);
    assert_eq!(recorder.event_count(), 0);
    assert_eq!(recorder.checkpoint_count(), 0);
    assert!(recorder.events_since(0).is_empty());
    assert!(recorder.ipc_replay_sequence().is_empty());
    assert!(recorder.stop().is_none());
}

// ── Adversarial Tests ─────────────────────────────────────────────────────

mod adversarial {
    use super::*;
    use std::thread;

    fn make_ipc(id: &str, cmd: &str) -> AppEvent {
        AppEvent::Ipc(IpcCall {
            id: id.to_string(),
            command: cmd.to_string(),
            timestamp: Utc::now(),
            duration_ms: Some(1),
            result: event::IpcResult::Ok(serde_json::json!("ok")),
            arg_size_bytes: 0,
            webview_label: "main".to_string(),
        })
    }

    // ── Mutex poisoning recovery ────────────────────────────────────────

    #[test]
    fn event_log_survives_poisoned_mutex() {
        let log = EventLog::new(10);
        let log2 = log.clone();
        let _ = thread::spawn(move || {
            let _guard = log2.snapshot();
            panic!("intentional panic while holding lock");
        })
        .join();
        // After poisoning, operations should still work
        log.push(make_ipc("1", "after_poison"));
        assert_eq!(log.len(), 1);
        assert!(!log.is_empty());
        let snap = log.snapshot();
        assert_eq!(snap.len(), 1);
    }

    #[test]
    fn recorder_survives_poisoned_mutex() {
        let rec = EventRecorder::new(100);
        let rec2 = rec.clone();
        let _ = thread::spawn(move || {
            let _guard = rec2.event_count();
            panic!("intentional panic while holding lock");
        })
        .join();
        // After poisoning, operations should still work via recovery
        assert!(!rec.is_recording());
        assert_eq!(rec.event_count(), 0);
        assert!(rec.start("after-poison".to_string()));
        rec.record_event(make_ipc("1", "test"));
        assert_eq!(rec.event_count(), 1);
    }

    #[test]
    fn registry_survives_poisoned_rwlock() {
        let reg = CommandRegistry::new();
        let reg2 = reg.clone();
        let _ = thread::spawn(move || {
            let _list = reg2.list();
            panic!("intentional panic while holding read lock");
        })
        .join();
        reg.register(registry::CommandInfo {
            name: "after_poison".to_string(),
            plugin: None,
            description: Some("works after poisoning".to_string()),
            args: vec![],
            return_type: None,
            is_async: false,
            intent: None,
            category: None,
            examples: vec![],
        });
        assert_eq!(reg.count(), 1);
    }

    // ── Concurrent access ───────────────────────────────────────────────

    #[test]
    fn event_log_concurrent_push_and_read() {
        let log = Arc::new(EventLog::new(1000));
        let mut handles = Vec::new();
        for i in 0..10 {
            let log = Arc::clone(&log);
            handles.push(thread::spawn(move || {
                for j in 0..100 {
                    log.push(make_ipc(&format!("{i}-{j}"), "concurrent"));
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(log.len(), 1000);
    }

    #[test]
    fn registry_concurrent_register_and_search() {
        let reg = Arc::new(CommandRegistry::new());
        let mut handles = Vec::new();
        for i in 0..10 {
            let reg = Arc::clone(&reg);
            handles.push(thread::spawn(move || {
                for j in 0..10 {
                    reg.register(registry::CommandInfo {
                        name: format!("cmd_{i}_{j}"),
                        plugin: None,
                        description: Some(format!("Command {i}-{j}")),
                        args: vec![],
                        return_type: None,
                        is_async: false,
                        intent: None,
                        category: None,
                        examples: vec![],
                    });
                }
            }));
        }
        for i in 0..5 {
            let reg = Arc::clone(&reg);
            handles.push(thread::spawn(move || {
                for _ in 0..20 {
                    let _ = reg.search(&format!("cmd_{i}"));
                    let _ = reg.resolve(&format!("cmd {i}"));
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(reg.count(), 100);
    }

    // ── Ring buffer edge cases ──────────────────────────────────────────

    #[test]
    fn event_log_capacity_one() {
        let log = EventLog::new(1);
        log.push(make_ipc("1", "first"));
        log.push(make_ipc("2", "second"));
        assert_eq!(log.len(), 1);
        let snap = log.snapshot();
        match &snap[0] {
            AppEvent::Ipc(call) => assert_eq!(call.id, "2"),
            _ => panic!("expected IPC event"),
        }
    }

    #[test]
    fn recorder_ring_buffer_wraps_correctly() {
        let rec = EventRecorder::new(3);
        rec.start("wrap-test".to_string());
        for i in 0..5 {
            rec.record_event(make_ipc(&i.to_string(), &format!("cmd_{i}")));
        }
        assert_eq!(rec.event_count(), 3);
        let session = rec.stop().unwrap();
        assert_eq!(session.events.len(), 3);
        // Should have events 2, 3, 4 (oldest evicted)
        assert_eq!(session.events[0].index, 2);
        assert_eq!(session.events[2].index, 4);
    }

    #[test]
    fn recorder_event_index_stays_monotonic_after_wrap() {
        let rec = EventRecorder::new(2);
        rec.start("mono-test".to_string());
        for i in 0..10 {
            rec.record_event(make_ipc(&i.to_string(), "wrap"));
        }
        let session = rec.stop().unwrap();
        for pair in session.events.windows(2) {
            assert!(pair[1].index > pair[0].index);
        }
    }

    // ── Verification edge cases ─────────────────────────────────────────

    #[test]
    fn verify_both_empty_objects_pass() {
        let result = verify_state(serde_json::json!({}), serde_json::json!({}));
        assert!(result.passed);
    }

    #[test]
    fn verify_deeply_nested_divergence() {
        let frontend = serde_json::json!({"a": {"b": {"c": {"d": 1}}}});
        let backend = serde_json::json!({"a": {"b": {"c": {"d": 2}}}});
        let result = verify_state(frontend, backend);
        assert!(!result.passed);
        assert_eq!(result.divergences[0].path, "a.b.c.d");
    }

    #[test]
    fn verify_type_mismatch_string_vs_number() {
        let frontend = serde_json::json!({"count": "5"});
        let backend = serde_json::json!({"count": 5});
        let result = verify_state(frontend, backend);
        assert!(!result.passed);
        assert_eq!(
            result.divergences[0].severity,
            types::DivergenceSeverity::Error
        );
    }

    #[test]
    fn verify_null_vs_missing_key() {
        let frontend = serde_json::json!({"a": null});
        let backend = serde_json::json!({});
        let result = verify_state(frontend, backend);
        assert!(!result.passed);
    }

    #[test]
    fn verify_empty_arrays_pass() {
        let result = verify_state(serde_json::json!({"a": []}), serde_json::json!({"a": []}));
        assert!(result.passed);
    }

    #[test]
    fn verify_array_length_mismatch() {
        let frontend = serde_json::json!({"arr": [1, 2, 3]});
        let backend = serde_json::json!({"arr": [1, 2]});
        let result = verify_state(frontend, backend);
        assert!(!result.passed);
        assert!(result.divergences.iter().any(|d| d.path.contains("[2]")));
    }

    // ── Ghost command edge cases ────────────────────────────────────────

    #[test]
    fn ghost_commands_empty_both_sides() {
        let registry = CommandRegistry::new();
        let report = detect_ghost_commands(&[], &registry);
        assert!(report.ghost_commands.is_empty());
        assert_eq!(report.total_frontend_commands, 0);
        assert_eq!(report.total_registry_commands, 0);
    }

    #[test]
    fn ghost_commands_perfect_match() {
        let registry = CommandRegistry::new();
        registry.register(registry::CommandInfo {
            name: "get_settings".to_string(),
            plugin: None,
            description: None,
            args: vec![],
            return_type: None,
            is_async: false,
            intent: None,
            category: None,
            examples: vec![],
        });
        let frontend = vec!["get_settings".to_string()];
        let report = detect_ghost_commands(&frontend, &registry);
        assert!(report.ghost_commands.is_empty());
    }

    // ── Assertion edge cases ────────────────────────────────────────────

    #[test]
    fn assertion_unknown_condition_reports_error() {
        let assertion = verification::SemanticAssertion {
            label: "bogus".to_string(),
            condition: "definitely_not_a_condition".to_string(),
            expected: serde_json::json!(true),
        };
        let result = verification::evaluate_assertion(serde_json::json!(true), &assertion);
        assert!(!result.passed);
        assert!(
            result
                .message
                .as_ref()
                .unwrap()
                .contains("Unknown assertion condition")
        );
    }

    #[test]
    fn assertion_truthy_edge_cases() {
        let truthy = verification::SemanticAssertion {
            label: "t".to_string(),
            condition: "truthy".to_string(),
            expected: serde_json::json!(null),
        };
        // Numbers are truthy (including 0 — this is Rust semantics, not JS)
        assert!(verification::evaluate_assertion(serde_json::json!(0), &truthy).passed);
        assert!(verification::evaluate_assertion(serde_json::json!(42), &truthy).passed);
        // Empty string is falsy
        assert!(!verification::evaluate_assertion(serde_json::json!(""), &truthy).passed);
        // Non-empty string is truthy
        assert!(verification::evaluate_assertion(serde_json::json!("x"), &truthy).passed);
        // null is falsy
        assert!(!verification::evaluate_assertion(serde_json::Value::Null, &truthy).passed);
        // false is falsy
        assert!(!verification::evaluate_assertion(serde_json::json!(false), &truthy).passed);
    }

    #[test]
    fn assertion_contains_in_array() {
        let assertion = verification::SemanticAssertion {
            label: "arr".to_string(),
            condition: "contains".to_string(),
            expected: serde_json::json!(2),
        };
        assert!(verification::evaluate_assertion(serde_json::json!([1, 2, 3]), &assertion).passed);
        assert!(!verification::evaluate_assertion(serde_json::json!([1, 3, 5]), &assertion).passed);
    }

    // ── IPC integrity edge cases ────────────────────────────────────────

    #[test]
    fn ipc_integrity_empty_log() {
        let log = EventLog::new(100);
        let report = check_ipc_integrity(&log, 5000);
        assert!(report.healthy);
        assert_eq!(report.total_calls, 0);
    }

    #[test]
    fn ipc_integrity_detects_errors() {
        let log = EventLog::new(100);
        log.push(AppEvent::Ipc(IpcCall {
            id: "err1".to_string(),
            command: "broken_cmd".to_string(),
            timestamp: Utc::now(),
            duration_ms: None,
            result: event::IpcResult::Err("something failed".to_string()),
            arg_size_bytes: 0,
            webview_label: "main".to_string(),
        }));
        let report = check_ipc_integrity(&log, 5000);
        assert!(!report.healthy);
        assert_eq!(report.errored, 1);
        assert_eq!(report.error_calls[0].command, "broken_cmd");
    }

    // ── Recording checkpoint limits ─────────────────────────────────────

    #[test]
    fn recorder_double_start_returns_false() {
        let rec = EventRecorder::new(100);
        assert!(rec.start("first".to_string()));
        assert!(!rec.start("second".to_string()));
        let session = rec.stop().unwrap();
        assert_eq!(session.id, "first");
    }

    #[test]
    fn recorder_checkpoint_without_recording_returns_false() {
        let rec = EventRecorder::new(100);
        assert!(!rec.checkpoint("cp1".to_string(), None, serde_json::json!({})));
    }

    #[test]
    fn recorder_default_impl() {
        let rec = EventRecorder::default();
        assert!(!rec.is_recording());
        assert!(rec.start("default-test".to_string()));
        for i in 0..100 {
            rec.record_event(make_ipc(&i.to_string(), "default"));
        }
        assert_eq!(rec.event_count(), 100);
        rec.stop();
    }

    // ── Registry sort stability with equal scores ───────────────────────

    #[test]
    fn registry_resolve_with_no_match() {
        let reg = CommandRegistry::new();
        reg.register(registry::CommandInfo {
            name: "alpha".to_string(),
            plugin: None,
            description: None,
            args: vec![],
            return_type: None,
            is_async: false,
            intent: None,
            category: None,
            examples: vec![],
        });
        let results = reg.resolve("zzz_nonexistent_query");
        assert!(results.is_empty());
    }

    #[test]
    fn registry_resolve_empty_query() {
        let reg = CommandRegistry::new();
        reg.register(registry::CommandInfo {
            name: "alpha".to_string(),
            plugin: None,
            description: None,
            args: vec![],
            return_type: None,
            is_async: false,
            intent: None,
            category: None,
            examples: vec![],
        });
        let results = reg.resolve("");
        assert!(results.is_empty());
    }
}
