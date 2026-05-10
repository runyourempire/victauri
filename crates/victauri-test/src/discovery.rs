//! Per-process server discovery for CI parallelism.
//!
//! Victauri servers write discovery files to `<temp>/victauri/<pid>/` with
//! port, token, and metadata. This module scans those directories and returns
//! the live server(s). Stale directories from dead processes are cleaned up
//! by checking TCP connectivity on the advertised port.

use std::path::PathBuf;

fn victauri_base_dir() -> PathBuf {
    std::env::temp_dir().join("victauri")
}

/// Scan per-process discovery directories and return the port of a live server.
///
/// Returns `None` if no live server is found, or if multiple are found (ambiguous).
pub fn scan_discovery_dirs_for_port() -> Option<u16> {
    let servers = find_live_servers();
    if servers.len() == 1 {
        return Some(servers[0].port);
    }
    None
}

/// Scan per-process discovery directories and return the token of a live server.
pub fn scan_discovery_dirs_for_token() -> Option<String> {
    let servers = find_live_servers();
    if servers.len() == 1 {
        return servers[0].token.clone();
    }
    None
}

struct DiscoveredServer {
    port: u16,
    token: Option<String>,
}

fn find_live_servers() -> Vec<DiscoveredServer> {
    let base = victauri_base_dir();
    let Ok(entries) = std::fs::read_dir(&base) else {
        return Vec::new();
    };

    let mut servers = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(pid_str) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if pid_str.parse::<u32>().is_err() {
            continue;
        }
        let port_path = path.join("port");
        let Ok(port_str) = std::fs::read_to_string(&port_path) else {
            continue;
        };
        let Ok(port) = port_str.trim().parse::<u16>() else {
            continue;
        };
        // Check if the port is reachable — if not, the server is dead
        if std::net::TcpStream::connect_timeout(
            &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
            std::time::Duration::from_millis(100),
        )
        .is_err()
        {
            let _ = std::fs::remove_dir_all(&path);
            continue;
        }
        let token = std::fs::read_to_string(path.join("token"))
            .ok()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
        servers.push(DiscoveredServer { port, token });
    }
    servers
}
