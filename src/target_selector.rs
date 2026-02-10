use crate::error::{Error, Result};
use colored::Colorize;
use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

/// Read target artifacts list from targets.list file
/// Returns deduplicated list of targets preserving the order from the file
pub fn read_targets_list(project_root: &Path, feature: &str) -> Result<Vec<String>> {
    let targets_file = project_root
        .join(".c2rust")
        .join(feature)
        .join("c")
        .join("targets.list");

    if !targets_file.exists() {
        return Err(Error::TargetsListNotFound(format!(
            "targets.list file not found at {}",
            targets_file.display()
        )));
    }

    let content = fs::read_to_string(&targets_file).map_err(|e| {
        Error::IoError(format!(
            "Failed to read targets.list from {}: {}",
            targets_file.display(),
            e
        ))
    })?;

    // Read targets line by line, deduplicating while preserving order
    let mut targets = Vec::new();
    let mut seen = HashSet::new();

    for line in content.lines() {
        let trimmed = line.trim();
        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Add only if not seen before (deduplication)
        if seen.insert(trimmed.to_string()) {
            targets.push(trimmed.to_string());
        }
    }

    if targets.is_empty() {
        return Err(Error::NoTargetsFound(
            "No valid targets found in targets.list".to_string(),
        ));
    }

    Ok(targets)
}

/// Parse user input for target selection (1-based indexing)
/// Returns 0-based index of the selected target
fn parse_target_selection(input: &str, total_targets: usize) -> Result<usize> {
    let input = input.trim();

    if input.is_empty() {
        return Err(Error::InvalidInput(
            "No input provided. Please select a target.".to_string(),
        ));
    }

    let index: usize = input.parse().map_err(|_| {
        Error::InvalidInput(format!("Invalid number: {}", input))
    })?;

    if index < 1 || index > total_targets {
        return Err(Error::InvalidInput(format!(
            "Selection {} is out of bounds (valid: 1-{})",
            index, total_targets
        )));
    }

    Ok(index - 1)
}

/// Prompt user to select a target from the list
pub fn prompt_target_selection(project_root: &Path, feature: &str) -> Result<String> {
    let targets = read_targets_list(project_root, feature)?;

    // If there's only one target, auto-select it
    if targets.len() == 1 {
        let target = &targets[0];
        println!(
            "\n{} {}",
            "Only one target available, auto-selecting:".bright_cyan(),
            target.bright_yellow()
        );
        return Ok(target.clone());
    }

    // Display available targets
    println!("\n{}", "Available target artifacts:".bright_cyan().bold());
    for (idx, target) in targets.iter().enumerate() {
        println!("  {}. {}", idx + 1, target.bright_yellow());
    }

    println!();
    println!(
        "{}",
        "Select a target artifact to translate:".bright_yellow()
    );
    println!("  - Enter the number of the target");
    print!("\n{} ", "Your selection:".bright_green().bold());
    io::stdout().flush().map_err(|e| {
        Error::IoError(format!("Failed to flush stdout: {}", e))
    })?;

    // Read user input
    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(|e| {
        Error::IoError(format!("Failed to read user input: {}", e))
    })?;

    let selected_idx = parse_target_selection(&input, targets.len())?;
    let selected_target = &targets[selected_idx];

    println!(
        "{} {}",
        "Selected target:".bright_green(),
        selected_target.bright_yellow().bold()
    );

    Ok(selected_target.clone())
}

/// Store the selected target in configuration using c2rust-config
pub fn store_target_in_config(
    project_root: &Path,
    feature: &str,
    target: &str,
) -> Result<()> {
    let c2rust_dir = project_root.join(".c2rust");

    // Use c2rust-config to set build.target
    let output = Command::new("c2rust-config")
        .current_dir(&c2rust_dir)
        .args([
            "config",
            "--make",
            "--feature",
            feature,
            "--set",
            "build.target",
            target,
        ])
        .output()
        .map_err(|e| {
            Error::CommandExecutionFailed(format!(
                "Failed to execute c2rust-config to store target: {}",
                e
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::CommandExecutionFailed(format!(
            "Failed to store target in config: {}",
            stderr
        )));
    }

    // Verify the value was actually persisted
    let verify_output = Command::new("c2rust-config")
        .current_dir(&c2rust_dir)
        .args([
            "config",
            "--make",
            "--feature",
            feature,
            "--list",
            "build.target",
        ])
        .output()
        .map_err(|e| {
            Error::CommandExecutionFailed(format!(
                "Failed to verify build.target in config: {}",
                e
            ))
        })?;

    if !verify_output.status.success() {
        let stdout = String::from_utf8_lossy(&verify_output.stdout);
        let stderr = String::from_utf8_lossy(&verify_output.stderr);
        return Err(Error::CommandExecutionFailed(format!(
            "Failed to verify build.target was stored correctly (status: {}): stdout: {} stderr: {}",
            verify_output.status, stdout, stderr
        )));
    }

    let stored_value = String::from_utf8_lossy(&verify_output.stdout)
        .trim()
        .to_string();
    if stored_value != target {
        return Err(Error::ConfigError(format!(
            "build.target verification failed: expected '{}', got '{}'",
            target, stored_value
        )));
    }

    println!(
        "{} {} = {}",
        "✓ Stored in config:".bright_green(),
        "build.target".cyan(),
        target.bright_yellow()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_read_targets_list_basic() {
        let temp_dir = TempDir::new().unwrap();

        // Create .c2rust/test_feature/c directory structure
        let c2rust_dir = temp_dir.path().join(".c2rust");
        let feature_dir = c2rust_dir.join("test_feature");
        let c_dir = feature_dir.join("c");
        fs::create_dir_all(&c_dir).unwrap();

        let targets_file = c_dir.join("targets.list");
        let mut file = fs::File::create(&targets_file).unwrap();
        writeln!(file, "target1").unwrap();
        writeln!(file, "target2").unwrap();
        writeln!(file, "target3").unwrap();

        let result = read_targets_list(temp_dir.path(), "test_feature");
        assert!(result.is_ok());
        let targets = result.unwrap();
        assert_eq!(targets.len(), 3);
        assert_eq!(targets[0], "target1");
        assert_eq!(targets[1], "target2");
        assert_eq!(targets[2], "target3");
    }

    #[test]
    fn test_read_targets_list_with_duplicates() {
        let temp_dir = TempDir::new().unwrap();

        // Create .c2rust/test_feature/c directory structure
        let c2rust_dir = temp_dir.path().join(".c2rust");
        let feature_dir = c2rust_dir.join("test_feature");
        let c_dir = feature_dir.join("c");
        fs::create_dir_all(&c_dir).unwrap();

        let targets_file = c_dir.join("targets.list");
        let mut file = fs::File::create(&targets_file).unwrap();
        writeln!(file, "target1").unwrap();
        writeln!(file, "target2").unwrap();
        writeln!(file, "target1").unwrap(); // Duplicate
        writeln!(file, "target3").unwrap();
        writeln!(file, "target2").unwrap(); // Duplicate

        let result = read_targets_list(temp_dir.path(), "test_feature");
        assert!(result.is_ok());
        let targets = result.unwrap();
        // Should have only 3 unique targets, in order of first appearance
        assert_eq!(targets.len(), 3);
        assert_eq!(targets[0], "target1");
        assert_eq!(targets[1], "target2");
        assert_eq!(targets[2], "target3");
    }

    #[test]
    fn test_read_targets_list_with_empty_lines_and_comments() {
        let temp_dir = TempDir::new().unwrap();

        // Create .c2rust/test_feature/c directory structure
        let c2rust_dir = temp_dir.path().join(".c2rust");
        let feature_dir = c2rust_dir.join("test_feature");
        let c_dir = feature_dir.join("c");
        fs::create_dir_all(&c_dir).unwrap();

        let targets_file = c_dir.join("targets.list");
        let mut file = fs::File::create(&targets_file).unwrap();
        writeln!(file, "# This is a comment").unwrap();
        writeln!(file, "target1").unwrap();
        writeln!(file, "").unwrap(); // Empty line
        writeln!(file, "  target2  ").unwrap(); // With spaces
        writeln!(file, "# Another comment").unwrap();
        writeln!(file, "target3").unwrap();

        let result = read_targets_list(temp_dir.path(), "test_feature");
        assert!(result.is_ok());
        let targets = result.unwrap();
        assert_eq!(targets.len(), 3);
        assert_eq!(targets[0], "target1");
        assert_eq!(targets[1], "target2");
        assert_eq!(targets[2], "target3");
    }

    #[test]
    fn test_parse_target_selection_valid() {
        assert_eq!(parse_target_selection("1", 3).unwrap(), 0);
        assert_eq!(parse_target_selection("2", 3).unwrap(), 1);
        assert_eq!(parse_target_selection("3", 3).unwrap(), 2);
        assert_eq!(parse_target_selection("  2  ", 3).unwrap(), 1);
    }

    #[test]
    fn test_parse_target_selection_invalid() {
        assert!(parse_target_selection("0", 3).is_err());
        assert!(parse_target_selection("4", 3).is_err());
        assert!(parse_target_selection("abc", 3).is_err());
        assert!(parse_target_selection("", 3).is_err());
        assert!(parse_target_selection("  ", 3).is_err());
    }

    #[test]
    fn test_read_targets_list_file_not_found() {
        let temp_dir = TempDir::new().unwrap();

        // Create .c2rust but no targets.list
        let c2rust_dir = temp_dir.path().join(".c2rust");
        let feature_dir = c2rust_dir.join("test_feature");
        let c_dir = feature_dir.join("c");
        fs::create_dir_all(&c_dir).unwrap();

        let result = read_targets_list(temp_dir.path(), "test_feature");
        assert!(result.is_err());
    }

    #[test]
    fn test_read_targets_list_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let c2rust_dir = temp_dir.path().join(".c2rust");
        let feature_dir = c2rust_dir.join("test_feature");
        let c_dir = feature_dir.join("c");
        fs::create_dir_all(&c_dir).unwrap();

        // Create empty targets.list
        fs::File::create(c_dir.join("targets.list")).unwrap();

        let result = read_targets_list(temp_dir.path(), "test_feature");
        assert!(result.is_err());
    }

    #[test]
    fn test_read_targets_list_only_comments() {
        let temp_dir = TempDir::new().unwrap();
        let c2rust_dir = temp_dir.path().join(".c2rust");
        let feature_dir = c2rust_dir.join("test_feature");
        let c_dir = feature_dir.join("c");
        fs::create_dir_all(&c_dir).unwrap();

        let targets_file = c_dir.join("targets.list");
        let mut file = fs::File::create(&targets_file).unwrap();
        writeln!(file, "# Comment 1").unwrap();
        writeln!(file, "").unwrap();
        writeln!(file, "# Comment 2").unwrap();

        let result = read_targets_list(temp_dir.path(), "test_feature");
        assert!(result.is_err());
    }
}
