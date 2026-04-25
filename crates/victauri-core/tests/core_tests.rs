use chrono::Utc;
use std::collections::HashMap;
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
    });

    registry.register(CommandInfo {
        name: "save_settings".to_string(),
        plugin: None,
        description: Some("Save app settings".to_string()),
        args: vec![],
        return_type: None,
        is_async: false,
    });

    registry.register(CommandInfo {
        name: "delete_user".to_string(),
        plugin: None,
        description: Some("Remove a user".to_string()),
        args: vec![],
        return_type: None,
        is_async: false,
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
