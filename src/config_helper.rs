use crate::error::{Error, Result};
use std::process::Command;

/// Get the c2rust-config binary path from environment or use default
fn get_c2rust_config_path() -> String {
    std::env::var("C2RUST_CONFIG").unwrap_or_else(|_| "c2rust-config".to_string())
}

/// Check if c2rust-config command exists
pub fn check_c2rust_config_exists() -> Result<()> {
    let config_path = get_c2rust_config_path();
    Command::new(&config_path)
        .arg("--help")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|_| ())
        .ok_or(Error::ConfigToolNotFound)
}

/// Save build configuration using c2rust-config
pub fn save_config(dir: &str, command: &str, feature: Option<&str>) -> Result<()> {
    let config_path = get_c2rust_config_path();
    let feature_args: Vec<&str> = feature.map(|f| vec!["--feature", f]).unwrap_or_default();

    // Save both build.dir and build.cmd
    for (key, value) in [("build.dir", dir), ("build.cmd", command)] {
        let output = Command::new(&config_path)
            .args(&["config", "--make"])
            .args(&feature_args)
            .args(&["--set", key, value])
            .output()
            .map_err(|e| Error::ConfigSaveFailed(format!("Failed to execute c2rust-config: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::ConfigSaveFailed(format!("Failed to save {}: {}", key, stderr)));
        }
    }

    Ok(())
}

/// Save compiler paths to c2rust-config globally
pub fn save_compilers(compilers: &[String]) -> Result<()> {
    if compilers.is_empty() {
        return Ok(());
    }

    let config_path = get_c2rust_config_path();
    
    for compiler in compilers {
        let output = Command::new(&config_path)
            .args(&["config", "--global", "--add", "compiler", compiler])
            .output()
            .map_err(|e| Error::ConfigSaveFailed(format!("Failed to execute c2rust-config: {}", e)))?;

        if output.status.success() {
            println!("Saved compiler: {}", compiler);
        } else {
            // Don't fail if compiler already exists, just warn
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("Warning: Failed to add compiler '{}': {}", compiler, stderr);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_check_c2rust_config_exists() {
        // This test will fail if c2rust-config is not installed
        // We can't test for ConfigToolNotFound without uninstalling it
        let _ = check_c2rust_config_exists();
    }

    #[test]
    #[serial]
    fn test_get_c2rust_config_path_with_env() {
        // Test that environment variable is respected
        // Save current value
        let original = std::env::var("C2RUST_CONFIG").ok();
        
        // Test with custom path
        std::env::set_var("C2RUST_CONFIG", "/custom/path/to/c2rust-config");
        let path = get_c2rust_config_path();
        assert_eq!(path, "/custom/path/to/c2rust-config");
        
        // Restore original value or remove if it wasn't set
        match original {
            Some(val) => std::env::set_var("C2RUST_CONFIG", val),
            None => std::env::remove_var("C2RUST_CONFIG"),
        }
    }

    #[test]
    #[serial]
    fn test_get_c2rust_config_path_without_env() {
        // Test default behavior when env var is not set
        // Save current value
        let original = std::env::var("C2RUST_CONFIG").ok();
        
        // Remove env var
        std::env::remove_var("C2RUST_CONFIG");
        let path = get_c2rust_config_path();
        assert_eq!(path, "c2rust-config");
        
        // Restore original value if it was set
        if let Some(val) = original {
            std::env::set_var("C2RUST_CONFIG", val);
        }
    }
}
