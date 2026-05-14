use std::path::PathBuf;

const HOST_NAME: &str = "com.victauri.browser";

/// Platform-specific native messaging host manifest location.
///
/// # Errors
///
/// Returns an error if the home directory cannot be determined.
pub fn host_manifest_path() -> Result<PathBuf, InstallerError> {
    let home = home_dir()?;

    #[cfg(target_os = "windows")]
    {
        Ok(home.join(".victauri").join("native-host-manifest.json"))
    }

    #[cfg(target_os = "macos")]
    {
        Ok(home
            .join("Library")
            .join("Application Support")
            .join("Google")
            .join("Chrome")
            .join("NativeMessagingHosts")
            .join(format!("{HOST_NAME}.json")))
    }

    #[cfg(target_os = "linux")]
    {
        Ok(home
            .join(".config")
            .join("google-chrome")
            .join("NativeMessagingHosts")
            .join(format!("{HOST_NAME}.json")))
    }
}

/// Generate the native messaging host manifest JSON.
#[must_use]
pub fn host_manifest(binary_path: &str, extension_id: &str) -> serde_json::Value {
    serde_json::json!({
        "name": HOST_NAME,
        "description": "Victauri Browser — MCP inspection for web pages",
        "path": binary_path,
        "type": "stdio",
        "allowed_origins": [
            format!("chrome-extension://{extension_id}/")
        ]
    })
}

/// Directory where the native host binary should be installed.
///
/// # Errors
///
/// Returns an error if the home directory cannot be determined.
pub fn install_dir() -> Result<PathBuf, InstallerError> {
    let home = home_dir()?;
    Ok(home.join(".victauri").join("bin"))
}

/// Install the native messaging host manifest.
///
/// 1. Writes the manifest JSON to the platform-specific location
/// 2. On Windows, creates the registry key
///
/// # Errors
///
/// Returns an error if file I/O or registry operations fail.
pub fn install(binary_path: &str, extension_id: &str) -> Result<String, InstallerError> {
    let manifest_path = host_manifest_path()?;
    let manifest = host_manifest(binary_path, extension_id);

    if let Some(parent) = manifest_path.parent() {
        std::fs::create_dir_all(parent).map_err(InstallerError::Io)?;
    }

    let json = serde_json::to_string_pretty(&manifest).map_err(InstallerError::Json)?;
    std::fs::write(&manifest_path, &json).map_err(InstallerError::Io)?;

    #[cfg(target_os = "windows")]
    {
        register_windows_host(&manifest_path)?;
    }

    Ok(manifest_path.to_string_lossy().to_string())
}

/// Uninstall the native messaging host manifest.
///
/// # Errors
///
/// Returns an error if file I/O or registry operations fail.
pub fn uninstall() -> Result<(), InstallerError> {
    let manifest_path = host_manifest_path()?;
    if manifest_path.exists() {
        std::fs::remove_file(&manifest_path).map_err(InstallerError::Io)?;
    }

    #[cfg(target_os = "windows")]
    {
        unregister_windows_host();
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn register_windows_host(manifest_path: &std::path::Path) -> Result<(), InstallerError> {
    use std::process::Command;

    let key = format!(
        r"HKCU\Software\Google\Chrome\NativeMessagingHosts\{HOST_NAME}"
    );
    let value = manifest_path.to_string_lossy();

    Command::new("reg")
        .args(["add", &key, "/ve", "/t", "REG_SZ", "/d", &value, "/f"])
        .output()
        .map_err(InstallerError::Io)?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn unregister_windows_host() {
    use std::process::Command;

    let key = format!(
        r"HKCU\Software\Google\Chrome\NativeMessagingHosts\{HOST_NAME}"
    );

    let _ = Command::new("reg").args(["delete", &key, "/f"]).output();
}

fn home_dir() -> Result<PathBuf, InstallerError> {
    #[cfg(target_os = "windows")]
    {
        std::env::var("USERPROFILE")
            .map(PathBuf::from)
            .map_err(|_| InstallerError::NoHomeDir)
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME")
            .map(PathBuf::from)
            .map_err(|_| InstallerError::NoHomeDir)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InstallerError {
    #[error("cannot determine home directory")]
    NoHomeDir,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_has_correct_name() {
        let manifest = host_manifest("/usr/local/bin/victauri-browser-host", "abcdef123456");
        assert_eq!(manifest["name"], HOST_NAME);
        assert_eq!(manifest["type"], "stdio");
    }

    #[test]
    fn manifest_has_allowed_origin() {
        let manifest = host_manifest("/path/to/binary", "test_extension_id");
        let origins = manifest["allowed_origins"].as_array().unwrap();
        assert_eq!(origins.len(), 1);
        assert!(origins[0].as_str().unwrap().contains("test_extension_id"));
    }

    #[test]
    fn manifest_path_is_deterministic() {
        let p1 = host_manifest_path();
        let p2 = host_manifest_path();
        assert!(p1.is_ok());
        assert_eq!(p1.unwrap(), p2.unwrap());
    }

    #[test]
    fn install_dir_is_in_home() {
        let dir = install_dir().unwrap();
        assert!(dir.to_string_lossy().contains(".victauri"));
        assert!(dir.to_string_lossy().contains("bin"));
    }
}
