use crate::error::{Error, Result};
use std::path::Path;
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
pub fn save_config(
    dir: &str,
    command: &str,
    feature: Option<&str>,
    project_root: &Path,
) -> Result<()> {
    let config_path = get_c2rust_config_path();
    let feature_args: Vec<&str> = feature.map(|f| vec!["--feature", f]).unwrap_or_default();

    for (key, value) in [("build.dir", dir), ("build.cmd", command)] {
        let output = Command::new(&config_path)
            .args(["config", "--make"])
            .args(&feature_args)
            .args(["--set", key, value])
            .current_dir(project_root)
            .output()
            .map_err(|e| {
                Error::ConfigSaveFailed(format!("Failed to execute c2rust-config: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::ConfigSaveFailed(format!(
                "Failed to save {}: {}",
                key, stderr
            )));
        }
    }

    Ok(())
}

/// Save compiler paths to c2rust-config globally
pub fn save_compilers(compilers: &[String], project_root: &Path) -> Result<()> {
    if compilers.is_empty() {
        return Ok(());
    }

    let config_path = get_c2rust_config_path();

    for compiler in compilers {
        let output = Command::new(&config_path)
            .args(["config", "--global", "--add", "compiler", compiler])
            .current_dir(project_root)
            .output()
            .map_err(|e| {
                Error::ConfigSaveFailed(format!("Failed to execute c2rust-config: {}", e))
            })?;

        if output.status.success() {
            println!("Saved compiler: {}", compiler);
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("Warning: Failed to add compiler '{}': {}", compiler, stderr);
        }
    }

    Ok(())
}

/// Save target artifact to c2rust-config
pub fn save_target(target: &str, feature: Option<&str>, project_root: &Path) -> Result<()> {
    let config_path = get_c2rust_config_path();
    let feature_args: Vec<&str> = feature.map(|f| vec!["--feature", f]).unwrap_or_default();

    let output = Command::new(&config_path)
        .args(["config", "--make"])
        .args(&feature_args)
        .args(["--set", "target", target])
        .current_dir(project_root)
        .output()
        .map_err(|e| {
            Error::ConfigSaveFailed(format!("Failed to execute c2rust-config: {}", e))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::ConfigSaveFailed(format!(
            "Failed to save target: {}",
            stderr
        )));
    }

    println!("Saved target artifact: {}", target);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_check_c2rust_config_exists() {
        let _ = check_c2rust_config_exists();
    }

    #[test]
    #[serial]
    fn test_get_c2rust_config_path_with_env() {
        let original = std::env::var("C2RUST_CONFIG").ok();

        std::env::set_var("C2RUST_CONFIG", "/custom/path/to/c2rust-config");
        let path = get_c2rust_config_path();
        assert_eq!(path, "/custom/path/to/c2rust-config");

        match original {
            Some(val) => std::env::set_var("C2RUST_CONFIG", val),
            None => std::env::remove_var("C2RUST_CONFIG"),
        }
    }

    #[test]
    #[serial]
    fn test_get_c2rust_config_path_without_env() {
        let original = std::env::var("C2RUST_CONFIG").ok();

        std::env::remove_var("C2RUST_CONFIG");
        let path = get_c2rust_config_path();
        assert_eq!(path, "c2rust-config");

        if let Some(val) = original {
            std::env::set_var("C2RUST_CONFIG", val);
        }
    }
}
