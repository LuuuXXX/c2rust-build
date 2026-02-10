use crate::error::{Error, Result};
use dialoguer::{theme::ColorfulTheme, Select};
use std::fs;
use std::io::Write;
use std::path::Path;

/// ANSI escape code to show cursor (restore terminal visibility)
const ANSI_SHOW_CURSOR: &str = "\x1B[?25h";

/// Read target artifacts from targets.list file
/// Returns a list of target paths (relative to project root)
pub fn read_targets_list(project_root: &Path, feature: &str) -> Result<Vec<String>> {
    let targets_list_path = project_root
        .join(".c2rust")
        .join(feature)
        .join("c")
        .join("targets.list");

    let content = match fs::read_to_string(&targets_list_path) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Vec::new());
        }
        Err(e) => return Err(e.into()),
    };

    let targets: Vec<String> = content
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .map(|line| line.to_string())
        .collect();

    Ok(targets)
}

/// Present an interactive target selection UI to the user
/// Returns the selected target path
/// If in non-interactive mode (no_interactive=true or not a TTY), selects the first target
pub fn select_target_interactive(targets: Vec<String>, no_interactive: bool) -> Result<String> {
    if targets.is_empty() {
        return Err(Error::TargetSelectionCancelled(
            "No targets found in targets.list".to_string(),
        ));
    }

    // Check if we should skip interactive selection
    let should_skip_interactive = no_interactive || !is_terminal();

    if should_skip_interactive {
        println!(
            "Non-interactive mode: selecting first target: {}",
            targets[0]
        );
        return Ok(targets[0].clone());
    }

    println!("\n=== Target Artifact Selection ===");
    println!("Found {} target artifact(s)", targets.len());
    println!("Use arrow keys to navigate, ENTER to confirm, ESC to cancel");
    println!();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select target artifact")
        .items(&targets)
        .default(0)
        .interact()
        .map_err(|e| {
            // Restore terminal state, ensure cursor is visible
            print!("{}", ANSI_SHOW_CURSOR);
            if let Err(flush_err) = std::io::stdout().flush() {
                eprintln!(
                    "Warning: Failed to flush terminal output during restoration: {}",
                    flush_err
                );
            }
            eprintln!(); // Add newline for cleaner terminal output after error
            Error::TargetSelectionCancelled(format!("{}", e))
        })?;

    let selected_target = targets[selection].clone();
    println!("\nSelected target: {}", selected_target);

    Ok(selected_target)
}

/// Check if the current process is running in a terminal
fn is_terminal() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal()
}

/// Process and select target artifact
/// This is a high-level function that:
/// 1. Reads targets from targets.list
/// 2. Presents interactive selection UI (or auto-selects in non-interactive mode)
/// 3. Returns the selected target path
///
/// # Returns
/// - `Ok(Some(target))` - A target was selected
/// - `Ok(None)` - No targets available
/// - `Err` - If selection fails
pub fn process_and_select_target(
    project_root: &Path,
    feature: &str,
    no_interactive: bool,
) -> Result<Option<String>> {
    println!("\nReading target artifacts list...");

    let targets = read_targets_list(project_root, feature)?;

    if targets.is_empty() {
        println!("Warning: No target artifacts found in targets.list");
        println!("Skipping target selection.");
        return Ok(None);
    }

    let selected_target = select_target_interactive(targets, no_interactive)?;
    Ok(Some(selected_target))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_read_targets_list_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();
        let feature = "default";

        let targets_dir = project_root.join(".c2rust").join(feature).join("c");
        fs::create_dir_all(&targets_dir).unwrap();
        fs::write(targets_dir.join("targets.list"), "").unwrap();

        let targets = read_targets_list(project_root, feature).unwrap();
        assert_eq!(targets.len(), 0);
    }

    #[test]
    fn test_read_targets_list_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();
        let feature = "default";

        let targets = read_targets_list(project_root, feature).unwrap();
        assert_eq!(targets.len(), 0);
    }

    #[test]
    fn test_read_targets_list_with_targets() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();
        let feature = "default";

        let targets_dir = project_root.join(".c2rust").join(feature).join("c");
        fs::create_dir_all(&targets_dir).unwrap();

        let content = "bin/myapp\nlib/libfoo.a\nlib/libbar.so\n";
        fs::write(targets_dir.join("targets.list"), content).unwrap();

        let targets = read_targets_list(project_root, feature).unwrap();
        assert_eq!(targets.len(), 3);
        assert_eq!(targets[0], "bin/myapp");
        assert_eq!(targets[1], "lib/libfoo.a");
        assert_eq!(targets[2], "lib/libbar.so");
    }

    #[test]
    fn test_read_targets_list_filters_empty_lines() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();
        let feature = "default";

        let targets_dir = project_root.join(".c2rust").join(feature).join("c");
        fs::create_dir_all(&targets_dir).unwrap();

        let content = "bin/myapp\n\n\nlib/libfoo.a\n  \n";
        fs::write(targets_dir.join("targets.list"), content).unwrap();

        let targets = read_targets_list(project_root, feature).unwrap();
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0], "bin/myapp");
        assert_eq!(targets[1], "lib/libfoo.a");
    }

    #[test]
    fn test_read_targets_list_trims_whitespace() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();
        let feature = "default";

        let targets_dir = project_root.join(".c2rust").join(feature).join("c");
        fs::create_dir_all(&targets_dir).unwrap();

        let content = "  bin/myapp  \n\tlib/libfoo.a\t\n";
        fs::write(targets_dir.join("targets.list"), content).unwrap();

        let targets = read_targets_list(project_root, feature).unwrap();
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0], "bin/myapp");
        assert_eq!(targets[1], "lib/libfoo.a");
    }

    #[test]
    fn test_process_and_select_target_no_targets() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();
        let feature = "default";

        let result = process_and_select_target(project_root, feature, true).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_process_and_select_target_non_interactive_selects_first() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();
        let feature = "default";

        let targets_dir = project_root.join(".c2rust").join(feature).join("c");
        fs::create_dir_all(&targets_dir).unwrap();

        let content = "bin/myapp\nlib/libfoo.a\n";
        fs::write(targets_dir.join("targets.list"), content).unwrap();

        let result = process_and_select_target(project_root, feature, true).unwrap();
        assert_eq!(result, Some("bin/myapp".to_string()));
    }
}
