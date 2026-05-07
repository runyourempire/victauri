pub fn current_stats() -> serde_json::Value {
    #[cfg(windows)]
    {
        process_memory_windows()
    }

    #[cfg(not(windows))]
    {
        process_memory_fallback()
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn process_memory_windows() -> serde_json::Value {
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::Threading::GetCurrentProcess;

    // SAFETY: `GetCurrentProcess()` returns a pseudo-handle that is always valid
    // for the calling process. `PROCESS_MEMORY_COUNTERS` is a plain data struct
    // that is safe to zero-initialize. `GetProcessMemoryInfo` reads into the
    // provided buffer and does not take ownership of any resources.
    unsafe {
        let process = GetCurrentProcess();
        let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;

        if GetProcessMemoryInfo(process, &mut counters, counters.cb).is_ok() {
            return serde_json::json!({
                "working_set_bytes": counters.WorkingSetSize,
                "peak_working_set_bytes": counters.PeakWorkingSetSize,
                "page_file_bytes": counters.PagefileUsage,
                "peak_page_file_bytes": counters.PeakPagefileUsage,
                "page_fault_count": counters.PageFaultCount,
            });
        }

        serde_json::json!({ "error": "failed to query process memory" })
    }
}

#[cfg(not(windows))]
fn process_memory_fallback() -> serde_json::Value {
    #[cfg(target_os = "linux")]
    {
        if let Ok(statm) = std::fs::read_to_string("/proc/self/statm") {
            let fields: Vec<&str> = statm.split_whitespace().collect();
            if fields.len() >= 2 {
                let page_size = 4096_u64;
                let total_pages: u64 = fields[0].parse().unwrap_or(0);
                let resident_pages: u64 = fields[1].parse().unwrap_or(0);
                return serde_json::json!({
                    "virtual_bytes": total_pages * page_size,
                    "resident_bytes": resident_pages * page_size,
                });
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(stats) = process_memory_macos() {
            return stats;
        }
    }

    serde_json::json!({ "error": "memory stats not available on this platform" })
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn process_memory_macos() -> Option<serde_json::Value> {
    use std::mem;

    #[repr(C)]
    struct MachTaskBasicInfo {
        virtual_size: u64,
        resident_size: u64,
        resident_size_max: u64,
        user_time: [u32; 2],
        system_time: [u32; 2],
        policy: i32,
        suspend_count: i32,
    }

    unsafe extern "C" {
        fn mach_task_self() -> u32;
        fn task_info(
            target_task: u32,
            flavor: u32,
            task_info_out: *mut MachTaskBasicInfo,
            task_info_count: *mut u32,
        ) -> i32;
    }

    const MACH_TASK_BASIC_INFO: u32 = 20;
    const KERN_SUCCESS: i32 = 0;

    // SAFETY: `mach_task_self()` returns the current task port (always valid).
    // `task_info` writes into the provided buffer within the declared size.
    // `MachTaskBasicInfo` is a plain data struct safe to zero-initialize.
    unsafe {
        let mut info: MachTaskBasicInfo = mem::zeroed();
        let mut count = (mem::size_of::<MachTaskBasicInfo>() / mem::size_of::<u32>()) as u32;
        let kr = task_info(
            mach_task_self(),
            MACH_TASK_BASIC_INFO,
            &mut info,
            &mut count,
        );
        if kr == KERN_SUCCESS {
            return Some(serde_json::json!({
                "virtual_bytes": info.virtual_size,
                "resident_bytes": info.resident_size,
                "resident_max_bytes": info.resident_size_max,
            }));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_stats_returns_json_object() {
        let stats = current_stats();
        assert!(
            stats.is_object(),
            "current_stats() should return a JSON object"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_stats_has_working_set_key() {
        let stats = current_stats();
        assert!(
            stats.get("working_set_bytes").is_some(),
            "Windows stats should contain working_set_bytes"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_stats_values_are_reasonable() {
        let stats = current_stats();
        let working_set = stats["working_set_bytes"]
            .as_u64()
            .expect("working_set_bytes should be a u64");
        let peak = stats["peak_working_set_bytes"]
            .as_u64()
            .expect("peak_working_set_bytes should be a u64");

        assert!(working_set > 0, "working_set_bytes should be > 0");
        assert!(
            peak >= working_set,
            "peak_working_set_bytes should be >= working_set_bytes"
        );
    }
}
