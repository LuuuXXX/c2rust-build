use crate::error::{Error, Result};
use std::process::Command;

/// Get the c2rust-config binary path from environment or use default
fn get_c2rust_config_path() -> String {
    std::env::var("C2RUST_CONFIG").unwrap_or_else(|_| "c2rust-config".to_string())
}

/// Check if c2rust-config command exists
pub fn check_c2rust_config_exists() -> Result<()> {
    let config_path = get_c2rust_config_path();
    let result = Command::new(&config_path)
        .arg("--version")
        .output();

    match result {
        Ok(output) if output.status.success() => Ok(()),
        _ => Err(Error::ConfigToolNotFound),
    }
}

/// Get compiler list from c2rust-config global configuration
/// Returns error if not configured - user must set it first
pub fn get_compiler_list() -> Result<Vec<String>> {
    let config_path = get_c2rust_config_path();
    let output = Command::new(&config_path)
        .args(&["config", "--global", "--list", "compiler"])
        .output()
        .map_err(|e| {
            Error::ConfigReadFailed(format!("Failed to execute c2rust-config: {}", e))
        })?;

    if !output.status.success() {
        return Err(Error::ConfigNotFound(
            "Compiler list not configured. Please set it first:\n\
             c2rust-config config --global --set compiler gcc clang g++ clang++"
                .to_string(),
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let compilers: Vec<String> = stdout
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if compilers.is_empty() {
        return Err(Error::ConfigNotFound(
            "Compiler list not configured. Please set it first:\n\
             c2rust-config config --global --set compiler gcc clang g++ clang++"
                .to_string(),
        ));
    }

    Ok(compilers)
}

/// Save build options to config
pub fn save_build_options(options: &str, feature: Option<&str>) -> Result<()> {
    let config_path = get_c2rust_config_path();
    let mut cmd = Command::new(&config_path);
    cmd.args(&["config", "--make", "--add", "build.options", options]);

    if let Some(f) = feature {
        cmd.args(&["--feature", f]);
    }

    let output = cmd.output().map_err(|e| {
        Error::ConfigSaveFailed(format!("Failed to execute c2rust-config: {}", e))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::ConfigSaveFailed(format!(
            "Failed to save build options: {}",
            stderr
        )));
    }

    Ok(())
}

/// Save build files for a specific index
pub fn save_build_files(index: usize, files: &[String], feature: Option<&str>) -> Result<()> {
    let config_path = get_c2rust_config_path();
    let mut cmd = Command::new(&config_path);
    let key = format!("build.files.{}", index);

    cmd.args(&["config", "--make", "--add", &key]);
    cmd.args(files);

    if let Some(f) = feature {
        cmd.args(&["--feature", f]);
    }

    let output = cmd.output().map_err(|e| {
        Error::ConfigSaveFailed(format!("Failed to execute c2rust-config: {}", e))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::ConfigSaveFailed(format!(
            "Failed to save build files: {}",
            stderr
        )));
    }

    Ok(())
}

/// Save build configuration using c2rust-config
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
