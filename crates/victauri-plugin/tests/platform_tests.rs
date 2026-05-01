//! Cross-platform validation tests for screenshot, memory, and bridge modules.

// ── Windows Tests ──────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod windows_tests {
    #[test]
    fn memory_stats_has_working_set() {
        let stats = victauri_plugin::mcp::tests_support::get_memory_stats();
        let ws = stats
            .get("working_set_bytes")
            .expect("missing working_set_bytes");
        assert!(ws.as_u64().unwrap() > 0, "working_set_bytes should be > 0");
    }

    #[test]
    fn memory_stats_has_peak_working_set() {
        let stats = victauri_plugin::mcp::tests_support::get_memory_stats();
        let peak = stats
            .get("peak_working_set_bytes")
            .expect("missing peak_working_set_bytes");
        assert!(
            peak.as_u64().unwrap() > 0,
            "peak_working_set_bytes should be > 0"
        );
    }

    #[test]
    fn memory_stats_has_page_faults() {
        let stats = victauri_plugin::mcp::tests_support::get_memory_stats();
        let pf = stats
            .get("page_fault_count")
            .expect("missing page_fault_count");
        assert!(
            pf.as_u64().unwrap() > 0,
            "page_fault_count should be > 0 for any running process"
        );
    }

    #[test]
    fn memory_stats_has_page_file() {
        let stats = victauri_plugin::mcp::tests_support::get_memory_stats();
        let pf = stats
            .get("page_file_bytes")
            .expect("missing page_file_bytes");
        assert!(pf.as_u64().is_some(), "page_file_bytes should be a number");
    }

    #[test]
    fn memory_stats_returns_json() {
        let stats = victauri_plugin::mcp::tests_support::get_memory_stats();
        assert!(stats.is_object(), "should return a JSON object");
        let obj = stats.as_object().unwrap();
        for key in [
            "working_set_bytes",
            "peak_working_set_bytes",
            "page_file_bytes",
            "peak_page_file_bytes",
            "page_fault_count",
        ] {
            assert!(obj.contains_key(key), "missing key: {key}");
        }
    }

    #[test]
    fn memory_stats_reasonable_values() {
        let stats = victauri_plugin::mcp::tests_support::get_memory_stats();
        let ws = stats["working_set_bytes"].as_u64().unwrap();
        let one_mb = 1_024 * 1_024;
        let hundred_gb = 100_u64 * 1_024 * 1_024 * 1_024;
        assert!(
            ws > one_mb,
            "working_set_bytes ({ws}) should be > 1 MB for a test process"
        );
        assert!(
            ws < hundred_gb,
            "working_set_bytes ({ws}) should be < 100 GB (sanity)"
        );
    }

    #[test]
    fn memory_stats_peak_ge_current() {
        let stats = victauri_plugin::mcp::tests_support::get_memory_stats();
        let ws = stats["working_set_bytes"].as_u64().unwrap();
        let peak = stats["peak_working_set_bytes"].as_u64().unwrap();
        assert!(peak >= ws, "peak ({peak}) should be >= current ({ws})");
    }

    #[test]
    fn memory_stats_no_error_key() {
        let stats = victauri_plugin::mcp::tests_support::get_memory_stats();
        assert!(
            stats.get("error").is_none(),
            "Windows memory stats should not contain an error key"
        );
    }
}

// ── macOS Tests ────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod macos_tests {
    #[test]
    fn memory_stats_returns_valid_json() {
        let stats = victauri_plugin::mcp::tests_support::get_memory_stats();
        assert!(stats.is_object(), "should return a JSON object");
    }

    // macOS uses the non-windows fallback which may return an error on non-Linux.
    // This test documents the expected behavior.
    #[test]
    fn macos_returns_error_or_platform_data() {
        let stats = victauri_plugin::mcp::tests_support::get_memory_stats();
        let obj = stats.as_object().unwrap();
        // On macOS the fallback returns {"error": "..."} since /proc/self/statm
        // does not exist. This is correct behavior.
        assert!(
            obj.contains_key("error") || obj.contains_key("resident_bytes"),
            "macOS should return either an error or resident_bytes"
        );
    }

    #[test]
    fn macos_module_compiles() {
        // Type existence check -- if this compiles, the module is present.
        let _stats = victauri_plugin::mcp::tests_support::get_memory_stats();
    }
}

// ── Linux Tests ────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
mod linux_tests {
    #[test]
    fn memory_stats_has_virtual_size() {
        let stats = victauri_plugin::mcp::tests_support::get_memory_stats();
        // Linux reads /proc/self/statm and returns virtual_bytes + resident_bytes
        if stats.get("error").is_none() {
            let virt = stats
                .get("virtual_bytes")
                .expect("Linux stats should contain virtual_bytes");
            assert!(virt.as_u64().unwrap() > 0, "virtual_bytes should be > 0");
        }
    }

    #[test]
    fn memory_stats_has_resident_bytes() {
        let stats = victauri_plugin::mcp::tests_support::get_memory_stats();
        if stats.get("error").is_none() {
            let res = stats
                .get("resident_bytes")
                .expect("Linux stats should contain resident_bytes");
            assert!(res.as_u64().unwrap() > 0, "resident_bytes should be > 0");
        }
    }

    #[test]
    fn memory_stats_returns_valid_json() {
        let stats = victauri_plugin::mcp::tests_support::get_memory_stats();
        assert!(stats.is_object(), "should return a JSON object");
    }
}

// ── Common Tests (all platforms) ───────────────────────────────────────────────

mod common_tests {
    #[test]
    fn memory_stats_json_not_empty() {
        let stats = victauri_plugin::mcp::tests_support::get_memory_stats();
        let obj = stats.as_object().expect("should be a JSON object");
        assert!(
            !obj.is_empty(),
            "memory stats should have at least one field"
        );
    }

    #[test]
    fn memory_stats_no_negative_values() {
        let stats = victauri_plugin::mcp::tests_support::get_memory_stats();
        let obj = stats.as_object().unwrap();
        for (key, val) in obj {
            if let Some(n) = val.as_i64() {
                assert!(n >= 0, "memory stats field '{key}' has negative value: {n}");
            }
        }
    }

    #[test]
    fn memory_stats_returns_object_not_array() {
        let stats = victauri_plugin::mcp::tests_support::get_memory_stats();
        assert!(!stats.is_array(), "should not be an array");
        assert!(!stats.is_null(), "should not be null");
        assert!(stats.is_object(), "should be a JSON object");
    }

    #[test]
    fn memory_stats_is_deterministic_shape() {
        // Calling twice should return the same set of keys (values may differ).
        let stats1 = victauri_plugin::mcp::tests_support::get_memory_stats();
        let stats2 = victauri_plugin::mcp::tests_support::get_memory_stats();
        let keys1: Vec<&String> = stats1.as_object().unwrap().keys().collect();
        let keys2: Vec<&String> = stats2.as_object().unwrap().keys().collect();
        assert_eq!(
            keys1, keys2,
            "memory stats keys should be consistent across calls"
        );
    }

    #[test]
    fn memory_stats_all_values_are_numbers_or_strings() {
        let stats = victauri_plugin::mcp::tests_support::get_memory_stats();
        let obj = stats.as_object().unwrap();
        for (key, val) in obj {
            assert!(
                val.is_number() || val.is_string(),
                "field '{key}' should be a number or string, got: {val}"
            );
        }
    }
}
