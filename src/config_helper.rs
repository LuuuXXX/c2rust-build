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
        .args(&["--set", "build.cmd", command]);

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

/// Remove surrounding quotes from a string
/// Note: Does not handle escaped quotes within quoted strings (e.g., "echo \"hello\"")
fn remove_quotes(s: &str) -> String {
    if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2) 
        || (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2) {
        s[1..s.len()-1].to_string()
    } else {
        s.to_string()
    }
}

/// Parse the build configuration output from c2rust-config
/// Expected format: { cmd = "make", dir = "build" }
fn parse_build_config(s: &str) -> Result<BuildConfig> {
    let mut config = BuildConfig::default();
    
    // Remove surrounding braces: "{ ... }" -> "..."
    let content = s.trim()
        .strip_prefix('{').unwrap_or(s.trim())
        .strip_suffix('}').unwrap_or(s.trim())
        .trim();
    
    // Split by comma to get individual key-value pairs
    for part in content.split(',') {
        let part = part.trim();
        
        // Split by '=' to get key and value
        if let Some((key, value)) = part.split_once('=') {
            let key = key.trim();
            let value = remove_quotes(value.trim());
            
            match key {
                "cmd" => config.command = Some(value),
                "dir" => config.dir = Some(value),
                _ => {} // Ignore unknown keys
            }
        }
    }
    
    Ok(config)
}

/// Read build configuration from c2rust-config
/// 
/// Queries the 'build' key directly which returns a structured format like:
/// { cmd = "make", dir = "build" }
pub fn read_config(feature: Option<&str>) -> Result<BuildConfig> {
    let config_path = get_c2rust_config_path();
    let feature_args = if let Some(f) = feature {
        vec!["--feature", f]
    } else {
        vec![]
    };

    // Query the 'build' configuration key
    let mut cmd = Command::new(&config_path);
    cmd.args(&["config", "--make"])
        .args(&feature_args)
        .args(&["--list", "build"]);

    match cmd.output() {
        Ok(output) if output.status.success() => {
            let value = String::from_utf8_lossy(&output.stdout);
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                parse_build_config(trimmed)
            } else {
                Ok(BuildConfig::default())
            }
        }
        Ok(_) => {
            // Config key doesn't exist, return empty config
            Ok(BuildConfig::default())
        }
        Err(e) => Err(Error::ConfigReadFailed(format!(
            "Failed to execute c2rust-config: {}",
            e
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_remove_quotes() {
        // Test with double quotes
        assert_eq!(remove_quotes("\"value\""), "value");
        
        // Test with single quotes
        assert_eq!(remove_quotes("'value'"), "value");
        
        // Test without quotes
        assert_eq!(remove_quotes("value"), "value");
        
        // Test empty string
        assert_eq!(remove_quotes(""), "");
        
        // Test single quote character
        assert_eq!(remove_quotes("\""), "\"");
    }

    #[test]
    fn test_parse_build_config() {
        // Test typical output format
        let result = parse_build_config("{ cmd = \"make\", dir = \"build\" }").unwrap();
        assert_eq!(result.command, Some("make".to_string()));
        assert_eq!(result.dir, Some("build".to_string()));

        // Test with single quotes
        let result = parse_build_config("{ cmd = 'make', dir = 'build' }").unwrap();
        assert_eq!(result.command, Some("make".to_string()));
        assert_eq!(result.dir, Some("build".to_string()));

        // Test without quotes
        let result = parse_build_config("{ cmd = make, dir = build }").unwrap();
        assert_eq!(result.command, Some("make".to_string()));
        assert_eq!(result.dir, Some("build".to_string()));

        // Test with extra whitespace
        let result = parse_build_config("{  cmd  =  \"make\"  ,  dir  =  \"build\"  }").unwrap();
        assert_eq!(result.command, Some("make".to_string()));
        assert_eq!(result.dir, Some("build".to_string()));

        // Test with only cmd
        let result = parse_build_config("{ cmd = \"make\" }").unwrap();
        assert_eq!(result.command, Some("make".to_string()));
        assert_eq!(result.dir, None);

        // Test with only dir
        let result = parse_build_config("{ dir = \"build\" }").unwrap();
        assert_eq!(result.command, None);
        assert_eq!(result.dir, Some("build".to_string()));

        // Test empty braces
        let result = parse_build_config("{}").unwrap();
        assert_eq!(result.command, None);
        assert_eq!(result.dir, None);

        // Test reverse order
        let result = parse_build_config("{ dir = \"build\", cmd = \"make\" }").unwrap();
        assert_eq!(result.command, Some("make".to_string()));
        assert_eq!(result.dir, Some("build".to_string()));

        // Test value containing '=' character
        let result = parse_build_config("{ cmd = \"VAR=value make\", dir = \"build\" }").unwrap();
        assert_eq!(result.command, Some("VAR=value make".to_string()));
        assert_eq!(result.dir, Some("build".to_string()));
    }

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
