use crate::error::{Error, Result};
use std::process::Command;

#[derive(Debug, Default, Clone)]
pub struct BuildConfig {
    pub dir: Option<String>,
    pub command: Option<String>,
}

/// Get the c2rust-config binary path from environment or use default
fn get_c2rust_config_path() -> String {
    std::env::var("C2RUST_CONFIG").unwrap_or_else(|_| "c2rust-config".to_string())
}

/// Check if c2rust-config command exists
pub fn check_c2rust_config_exists() -> Result<()> {
    let config_path = get_c2rust_config_path();
    let result = Command::new(&config_path)
        .arg("--help")
        .output();

    match result {
        Ok(output) if output.status.success() => Ok(()),
        _ => Err(Error::ConfigToolNotFound),
    }
}

/// Save configuration using c2rust-config
pub fn save_config(dir: &str, command: &str, feature: Option<&str>) -> Result<()> {
    let config_path = get_c2rust_config_path();
    let feature_args = if let Some(f) = feature {
        vec!["--feature", f]
    } else {
        vec![]
    };

    // Save build.dir configuration
    let mut cmd = Command::new(&config_path);
    cmd.args(&["config", "--make"])
        .args(&feature_args)
        .args(&["--set", "build.dir", dir]);

    let output = cmd.output().map_err(|e| {
        Error::ConfigSaveFailed(format!("Failed to execute c2rust-config: {}", e))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::ConfigSaveFailed(format!(
            "Failed to save build.dir: {}",
            stderr
        )));
    }

    // Save build command configuration
    let mut cmd = Command::new(&config_path);
    cmd.args(&["config", "--make"])
        .args(&feature_args)
        .args(&["--set", "build", command]);

    let output = cmd.output().map_err(|e| {
        Error::ConfigSaveFailed(format!("Failed to execute c2rust-config: {}", e))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::ConfigSaveFailed(format!(
            "Failed to save build command: {}",
            stderr
        )));
    }

    Ok(())
}

/// Extract config value from a line like "build.dir = /path/to/dir"
fn extract_config_value(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.splitn(2, '=').collect();
    if parts.len() != 2 {
        return None;
    }
    
    let value = parts[1].trim();
    // Remove quotes if present
    let value = value.trim_matches('"').trim_matches('\'');
    
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

/// Read configuration from c2rust-config
pub fn read_config(feature: Option<&str>) -> Result<BuildConfig> {
    let config_path = get_c2rust_config_path();
    let feature_args = if let Some(f) = feature {
        vec!["--feature", f]
    } else {
        vec![]
    };

    let mut cmd = Command::new(&config_path);
    cmd.args(&["config", "--make"])
        .args(&feature_args)
        .args(&["--list"]);

    let output = cmd.output().map_err(|e| {
        Error::ConfigReadFailed(format!("Failed to execute c2rust-config: {}", e))
    })?;
    
    // If config is not initialized (exit code 1 with specific message), return default
    // Otherwise, treat as a real error
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Check if this is a "config not initialized" case vs a real error.
        // Only treat "No such file" as benign when it clearly references the c2rust
        // config directory or config file, to avoid masking unrelated failures.
        if stderr.contains("not initialized")
            || (stderr.contains("No such file")
                && (stderr.contains(".c2rust/config.toml") || stderr.contains(".c2rust")))
        {
            return Ok(BuildConfig::default());
        }
        return Err(Error::ConfigReadFailed(format!(
            "c2rust-config failed with exit code {}: {}",
            output.status.code().unwrap_or(-1),
            stderr
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut config = BuildConfig::default();

    // Parse output
    for line in stdout.lines() {
        let line = line.trim();
        
        // Extract key from the line (before '=')
        let key = line.split('=').next().unwrap_or_default().trim();
        let normalized_key = key.trim_matches('"').trim_matches('\'');
        
        match normalized_key {
            "build.dir" => {
                config.dir = extract_config_value(line);
            }
            "build" => {
                config.command = extract_config_value(line);
            }
            _ => {}
        }
    }

    Ok(config)
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
