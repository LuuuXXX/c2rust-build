use crate::error::{Error, Result};
use dialoguer::{theme::ColorfulTheme, MultiSelect};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// ANSI escape code to show cursor (restore terminal visibility)
const ANSI_SHOW_CURSOR: &str = "\x1B[?25h";

/// Represents a preprocessed file available for selection
#[derive(Debug, Clone)]
pub struct PreprocessedFileInfo {
    /// Absolute path to the preprocessed file
    pub path: PathBuf,
    /// Display name (relative to the c directory)
    pub display_name: String,
}

/// Recursively collect all preprocessed files from the c directory
pub fn collect_preprocessed_files(c_dir: &Path) -> Result<Vec<PreprocessedFileInfo>> {
    let mut files = Vec::new();

    if !c_dir.exists() {
        return Ok(files);
    }

    collect_files_recursive(c_dir, c_dir, &mut files)?;

    // Sort files by display name for consistent ordering
    files.sort_by(|a, b| a.display_name.cmp(&b.display_name));

    Ok(files)
}

/// Helper function to recursively collect files
fn collect_files_recursive(
    base_dir: &Path,
    current_dir: &Path,
    files: &mut Vec<PreprocessedFileInfo>,
) -> Result<()> {
    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect_files_recursive(base_dir, &path, files)?;
        } else if path.is_file() {
            // Only include preprocessed files (.c2rust, .i, .ii extensions)
            let has_valid_extension = path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext == "c2rust" || ext == "i" || ext == "ii");

            if has_valid_extension {
                if let Ok(relative_path) = path.strip_prefix(base_dir) {
                    let display_name = relative_path.display().to_string();
                    files.push(PreprocessedFileInfo { path, display_name });
                }
            }
        }
    }

    Ok(())
}

/// Present an interactive file selection UI to the user
/// Returns the list of selected file paths
/// If in non-interactive mode (no_interactive=true or not a TTY), selects all files
pub fn select_files_interactive(
    files: Vec<PreprocessedFileInfo>,
    no_interactive: bool,
    selected_target: Option<&str>,
) -> Result<Vec<PathBuf>> {
    if files.is_empty() {
        println!("No preprocessed files found.");
        return Ok(Vec::new());
    }

    // Check if we should skip interactive selection
    let should_skip_interactive = no_interactive || !is_terminal();

    if should_skip_interactive {
        println!(
            "Non-interactive mode: selecting all {} file(s)",
            files.len()
        );
        let all_files: Vec<PathBuf> = files.into_iter().map(|f| f.path).collect();
        return Ok(all_files);
    }

    println!("\n=== File Selection ===");
    println!("Found {} preprocessed file(s)", files.len());

    // Show different prompts based on whether a target was selected
    if let Some(target) = selected_target {
        println!(
            "\x1b[1m选择参与构建 target '{}' 的文件 | Select files that participate in building target '{}'\x1b[0m",
            target, target
        );
    } else {
        println!("\x1b[1m选择要翻译的文件 | Select files to translate\x1b[0m");
    }
    println!("Use SPACE to select/deselect, ENTER to confirm, ESC to cancel");
    println!();

    let items: Vec<String> = files.iter().map(|f| f.display_name.clone()).collect();

    // All files are selected by default
    let defaults: Vec<bool> = vec![true; files.len()];

    let prompt_text = if let Some(target) = selected_target {
        format!(
            "Select files that participate in building target '{}'",
            target
        )
    } else {
        "Select files to translate".to_string()
    };

    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt(&prompt_text)
        .items(&items)
        .defaults(&defaults)
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
            Error::FileSelectionCancelled(format!("{}", e))
        })?;

    let selected_files: Vec<PathBuf> = selections
        .into_iter()
        .map(|idx| files[idx].path.clone())
        .collect();

    if let Some(target) = selected_target {
        println!(
            "\nSelected {} file(s) that participate in building target '{}'",
            selected_files.len(),
            target
        );
    } else {
        println!("\nSelected {} file(s)", selected_files.len());
    }

    Ok(selected_files)
}

/// Check if the current process is running in a terminal
fn is_terminal() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal()
}

/// Save the list of selected files to a JSON file
pub fn save_selected_files(
    selected_files: &[PathBuf],
    feature: &str,
    project_root: &Path,
) -> Result<()> {
    let selection_file = project_root
        .join(".c2rust")
        .join(feature)
        .join("selected_files.json");

    // Create parent directory if needed
    if let Some(parent) = selection_file.parent() {
        fs::create_dir_all(parent)?;
    }

    // Convert paths to strings for serialization
    let file_strings: Vec<String> = selected_files
        .iter()
        .map(|p| p.display().to_string())
        .collect();

    let json = serde_json::to_string_pretty(&file_strings)?;
    fs::write(&selection_file, json)?;

    println!("Selection saved to: {}", selection_file.display());

    Ok(())
}

/// Remove preprocessed files that were not selected by the user
/// This function deletes all preprocessed files except those in the selected list
/// After file deletion, it also removes empty directories recursively within the base directory
///
/// Safety: If selected_files is empty, no cleanup is performed to prevent accidental deletion
///
/// # Arguments
/// * `all_files` - All preprocessed files found
/// * `selected_files` - Files selected by the user to keep
/// * `base_dir` - Root directory for preprocessing files; cleanup will not traverse above this boundary
pub fn cleanup_unselected_files(
    all_files: &[PreprocessedFileInfo],
    selected_files: &[PathBuf],
    base_dir: &Path,
) -> Result<()> {
    if all_files.is_empty() || selected_files.is_empty() {
        // Safety: Don't delete all files if nothing was selected
        return Ok(());
    }

    // Convert to HashSet for O(1) lookup performance
    let selected_set: HashSet<&PathBuf> = selected_files.iter().collect();

    let mut removed_count = 0;
    let mut failed_removals = Vec::new();
    let mut parent_dirs = HashSet::new();

    for file_info in all_files {
        // Skip if this file is in the selected list
        if selected_set.contains(&file_info.path) {
            continue;
        }

        // Try to remove the unselected file
        match fs::remove_file(&file_info.path) {
            Ok(_) => {
                removed_count += 1;
                // Collect parent directory for cleanup
                if let Some(parent) = file_info.path.parent() {
                    parent_dirs.insert(parent.to_path_buf());
                }
            }
            Err(e) => {
                // Record failures but continue processing
                failed_removals.push((file_info.path.clone(), e));
            }
        }
    }

    if removed_count > 0 {
        println!("Removed {} unselected preprocessed file(s)", removed_count);
    }

    if !failed_removals.is_empty() {
        eprintln!(
            "Warning: Failed to remove {} file(s):",
            failed_removals.len()
        );
        for (path, err) in failed_removals {
            eprintln!("  - {}: {}", path.display(), err);
        }
    }

    // Clean up empty directories recursively, bounded by base_dir
    let dirs_removed = cleanup_empty_directories(parent_dirs, base_dir)?;
    if dirs_removed > 0 {
        println!("Removed {} empty directories", dirs_removed);
    }

    Ok(())
}

/// Recursively remove empty directories within a bounded root
/// This function processes directories bottom-up to handle nested empty directories.
/// It will not traverse or attempt to remove directories above the specified base_dir.
///
/// # Arguments
/// * `dirs` - Initial set of directories to check (typically parent dirs of deleted files)
/// * `base_dir` - Root boundary for cleanup; ancestor traversal stops at this directory
fn cleanup_empty_directories(dirs: HashSet<PathBuf>, base_dir: &Path) -> Result<usize> {
    let mut removed_count = 0;
    let mut all_parent_dirs = HashSet::new();
    let mut failed_removals = Vec::new();

    // Collect all parent directories up the tree, but stop at base_dir and never traverse above it
    for dir in &dirs {
        let mut current = dir.as_path();
        while let Some(parent) = current.parent() {
            // Stop traversing if we're about to leave the base_dir subtree
            if !parent.starts_with(base_dir) {
                break;
            }
            // Stop traversing once we've reached the base_dir boundary
            if parent == base_dir {
                break;
            }
            all_parent_dirs.insert(parent.to_path_buf());
            current = parent;
        }
    }

    // Combine original dirs with all parent dirs
    let mut all_dirs: Vec<PathBuf> = dirs.union(&all_parent_dirs).cloned().collect();

    // Sort by depth (deepest first) to process bottom-up
    all_dirs.sort_by(|a, b| {
        let depth_a = a.components().count();
        let depth_b = b.components().count();
        depth_b.cmp(&depth_a) // Reverse order: deepest first
    });

    // Try to remove each directory if it's empty and within bounds
    for dir in all_dirs {
        // Skip if this directory is the base_dir itself or above it
        if dir == base_dir || !dir.starts_with(base_dir) {
            continue;
        }

        match is_directory_empty(&dir) {
            Ok(true) => {
                // Directory is empty, try to remove it
                match fs::remove_dir(&dir) {
                    Ok(_) => {
                        removed_count += 1;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        // Directory was already removed, ignore
                    }
                    Err(e) => {
                        // Record other failures (e.g., permission denied, directory not actually empty)
                        failed_removals.push((dir.clone(), e));
                    }
                }
            }
            Ok(false) => {
                // Directory is not empty, skip
            }
            Err(e) => {
                // Failed to check if directory is empty (e.g., permission denied)
                eprintln!(
                    "Warning: Could not check if directory is empty: {}: {}",
                    dir.display(),
                    e
                );
            }
        }
    }

    if !failed_removals.is_empty() {
        let count = failed_removals.len();
        let word = if count == 1 {
            "directory"
        } else {
            "directories"
        };
        eprintln!("Warning: Failed to remove {} empty {}:", count, word);
        for (path, err) in failed_removals {
            eprintln!("  - {}: {}", path.display(), err);
        }
    }

    Ok(removed_count)
}

/// Process and select files for translation
/// This is a high-level function that:
/// 1. Collects preprocessed files from the c directory
/// 2. Presents interactive selection UI (or auto-selects all in non-interactive mode)
/// 3. Saves the selected files to a JSON file
/// 4. Cleans up unselected files
///
/// # Parameters
/// - `selected_target`: Optional target name to include in prompts
///
/// # Returns
/// - `Ok(usize)` - The number of files selected (0 if no files were found or selected)
/// - `Err` - If any file operation fails
pub fn process_and_select_files(
    c_dir: &Path,
    feature: &str,
    project_root: &Path,
    no_interactive: bool,
    selected_target: Option<&str>,
) -> Result<usize> {
    println!("\nCollecting preprocessed files from: {}", c_dir.display());

    let preprocessed_files = collect_preprocessed_files(c_dir)?;

    if preprocessed_files.is_empty() {
        println!(
            "Warning: No preprocessed files found in {}",
            c_dir.display()
        );
        println!("Make sure libhook.so is configured to generate preprocessing files.");
        return Ok(0);
    }

    let selected_files =
        select_files_interactive(preprocessed_files.clone(), no_interactive, selected_target)?;

    if !selected_files.is_empty() {
        // First save the selection
        save_selected_files(&selected_files, feature, project_root)?;
        let count = selected_files.len();
        if let Some(target) = selected_target {
            println!(
                "Selected {} file(s) that participate in building target '{}'",
                count, target
            );
        } else {
            println!("Selected {} file(s)", count);
        }

        // Then cleanup unselected files
        cleanup_unselected_files(&preprocessed_files, &selected_files, c_dir)?;

        Ok(count)
    } else {
        println!("No files selected.");
        Ok(0)
    }
}

/// Check if a directory is empty (contains no files or subdirectories)
/// Returns an error if the directory cannot be read (e.g., permission denied, doesn't exist)
fn is_directory_empty(path: &Path) -> Result<bool> {
    let mut entries = fs::read_dir(path)?;
    Ok(entries.next().is_none())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_collect_preprocessed_files_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        fs::create_dir_all(&c_dir).unwrap();

        let files = collect_preprocessed_files(&c_dir).unwrap();
        assert_eq!(files.len(), 0);
    }

    #[test]
    fn test_collect_preprocessed_files_with_files() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        fs::create_dir_all(&c_dir).unwrap();

        // Create some test files
        fs::write(c_dir.join("main.c.c2rust"), "content1").unwrap();
        fs::create_dir_all(c_dir.join("src")).unwrap();
        fs::write(c_dir.join("src").join("helper.c.c2rust"), "content2").unwrap();

        let files = collect_preprocessed_files(&c_dir).unwrap();
        assert_eq!(files.len(), 2);

        // Check that files are sorted
        assert_eq!(files[0].display_name, "main.c.c2rust");
        assert_eq!(files[1].display_name, "src/helper.c.c2rust");
    }

    #[test]
    fn test_collect_preprocessed_files_nonexistent_directory() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("nonexistent");

        let files = collect_preprocessed_files(&c_dir).unwrap();
        assert_eq!(files.len(), 0);
    }

    #[test]
    fn test_collect_preprocessed_files_nested_structure() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        fs::create_dir_all(c_dir.join("a/b/c")).unwrap();

        fs::write(c_dir.join("a/file1.c.c2rust"), "content").unwrap();
        fs::write(c_dir.join("a/b/file2.c.c2rust"), "content").unwrap();
        fs::write(c_dir.join("a/b/c/file3.c.c2rust"), "content").unwrap();

        let files = collect_preprocessed_files(&c_dir).unwrap();
        assert_eq!(files.len(), 3);

        // Verify all paths are relative to c_dir
        for file in &files {
            assert!(!file.display_name.contains(&c_dir.display().to_string()));
            assert!(file.path.starts_with(&c_dir));
        }
    }

    #[test]
    fn test_save_selected_files() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();
        let feature = "test_feature";

        let files = vec![
            PathBuf::from("/path/to/file1.c.c2rust"),
            PathBuf::from("/path/to/file2.c.c2rust"),
        ];

        save_selected_files(&files, feature, project_root).unwrap();

        let selection_file = project_root
            .join(".c2rust")
            .join(feature)
            .join("selected_files.json");

        assert!(selection_file.exists());

        let content = fs::read_to_string(&selection_file).unwrap();
        let loaded: Vec<String> = serde_json::from_str(&content).unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0], "/path/to/file1.c.c2rust");
        assert_eq!(loaded[1], "/path/to/file2.c.c2rust");
    }

    #[test]
    fn test_collect_preprocessed_files_filters_by_extension() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        fs::create_dir_all(&c_dir).unwrap();

        // Create valid preprocessed files
        fs::write(c_dir.join("valid1.c.c2rust"), "content1").unwrap();
        fs::write(c_dir.join("valid2.i"), "content2").unwrap();
        fs::write(c_dir.join("valid3.ii"), "content3").unwrap();

        // Create files that should be filtered out
        fs::write(c_dir.join("invalid.txt"), "content").unwrap();
        fs::write(c_dir.join("invalid.c"), "content").unwrap();
        fs::write(c_dir.join("invalid.json"), "content").unwrap();
        fs::write(c_dir.join(".hidden"), "content").unwrap();

        let files = collect_preprocessed_files(&c_dir).unwrap();

        // Only the 3 valid preprocessed files should be collected
        assert_eq!(files.len(), 3);

        let names: Vec<&str> = files.iter().map(|f| f.display_name.as_str()).collect();
        assert!(names.contains(&"valid1.c.c2rust"));
        assert!(names.contains(&"valid2.i"));
        assert!(names.contains(&"valid3.ii"));
    }

    #[test]
    fn test_cleanup_unselected_files_removes_only_unselected() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        fs::create_dir_all(&c_dir).unwrap();

        // Create test files
        let file1 = c_dir.join("file1.c.c2rust");
        let file2 = c_dir.join("file2.c.c2rust");
        let file3 = c_dir.join("file3.c.c2rust");

        fs::write(&file1, "content1").unwrap();
        fs::write(&file2, "content2").unwrap();
        fs::write(&file3, "content3").unwrap();

        let all_files = vec![
            PreprocessedFileInfo {
                path: file1.clone(),
                display_name: "file1.c.c2rust".to_string(),
            },
            PreprocessedFileInfo {
                path: file2.clone(),
                display_name: "file2.c.c2rust".to_string(),
            },
            PreprocessedFileInfo {
                path: file3.clone(),
                display_name: "file3.c.c2rust".to_string(),
            },
        ];

        // Select only file1 and file3
        let selected_files = vec![file1.clone(), file3.clone()];

        cleanup_unselected_files(&all_files, &selected_files, &c_dir).unwrap();

        // file1 and file3 should exist
        assert!(file1.exists());
        assert!(file3.exists());

        // file2 should be removed
        assert!(!file2.exists());
    }

    #[test]
    fn test_cleanup_unselected_files_empty_selection() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        fs::create_dir_all(&c_dir).unwrap();

        let file1 = c_dir.join("file1.c.c2rust");
        fs::write(&file1, "content1").unwrap();

        let all_files = vec![PreprocessedFileInfo {
            path: file1.clone(),
            display_name: "file1.c.c2rust".to_string(),
        }];

        // Empty selection
        let selected_files: Vec<PathBuf> = vec![];

        // Should not fail with empty selection
        cleanup_unselected_files(&all_files, &selected_files, &c_dir).unwrap();

        // File should still exist (cleanup is skipped for empty selection)
        assert!(file1.exists());
    }

    #[test]
    fn test_cleanup_unselected_files_all_selected() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        fs::create_dir_all(&c_dir).unwrap();

        let file1 = c_dir.join("file1.c.c2rust");
        let file2 = c_dir.join("file2.c.c2rust");

        fs::write(&file1, "content1").unwrap();
        fs::write(&file2, "content2").unwrap();

        let all_files = vec![
            PreprocessedFileInfo {
                path: file1.clone(),
                display_name: "file1.c.c2rust".to_string(),
            },
            PreprocessedFileInfo {
                path: file2.clone(),
                display_name: "file2.c.c2rust".to_string(),
            },
        ];

        // Select all files
        let selected_files = vec![file1.clone(), file2.clone()];

        cleanup_unselected_files(&all_files, &selected_files, &c_dir).unwrap();

        // All files should still exist
        assert!(file1.exists());
        assert!(file2.exists());
    }

    #[test]
    fn test_cleanup_unselected_files_removes_empty_directories() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");

        // Create nested directory structure
        let subdir1 = c_dir.join("subdir1");
        let subdir2 = c_dir.join("subdir2");
        fs::create_dir_all(&subdir1).unwrap();
        fs::create_dir_all(&subdir2).unwrap();

        // Create files in subdirectories
        let file1 = subdir1.join("file1.c.c2rust");
        let file2 = subdir2.join("file2.c.c2rust");

        fs::write(&file1, "content1").unwrap();
        fs::write(&file2, "content2").unwrap();

        let all_files = vec![
            PreprocessedFileInfo {
                path: file1.clone(),
                display_name: "subdir1/file1.c.c2rust".to_string(),
            },
            PreprocessedFileInfo {
                path: file2.clone(),
                display_name: "subdir2/file2.c.c2rust".to_string(),
            },
        ];

        // Select only file1, so file2 and subdir2 should be removed
        let selected_files = vec![file1.clone()];

        cleanup_unselected_files(&all_files, &selected_files, &c_dir).unwrap();

        // file1 and subdir1 should exist
        assert!(file1.exists());
        assert!(subdir1.exists());

        // file2 should be removed
        assert!(!file2.exists());

        // subdir2 should be removed (empty after file2 deletion)
        assert!(!subdir2.exists());
    }

    #[test]
    fn test_cleanup_unselected_files_recursive_empty_directory_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");

        // Create deeply nested directory structure
        let deep_dir = c_dir.join("a").join("b").join("c");
        fs::create_dir_all(&deep_dir).unwrap();

        // Create a file in the deepest directory
        let file1 = deep_dir.join("file1.c.c2rust");
        fs::write(&file1, "content1").unwrap();

        // Don't select any files - but we have empty selection safety
        // So let's add another file that we will select
        let another_file = c_dir.join("keep.c.c2rust");
        fs::write(&another_file, "keep").unwrap();

        let all_files = vec![
            PreprocessedFileInfo {
                path: file1.clone(),
                display_name: "a/b/c/file1.c.c2rust".to_string(),
            },
            PreprocessedFileInfo {
                path: another_file.clone(),
                display_name: "keep.c.c2rust".to_string(),
            },
        ];

        // Select only another_file, so file1 should be removed along with all parent dirs
        let selected_files = vec![another_file.clone()];

        cleanup_unselected_files(&all_files, &selected_files, &c_dir).unwrap();

        // file1 should be removed
        assert!(!file1.exists());

        // All parent directories should be removed recursively
        assert!(!deep_dir.exists());
        assert!(!c_dir.join("a").join("b").exists());
        assert!(!c_dir.join("a").exists());

        // But c_dir should still exist (contains another_file)
        assert!(c_dir.exists());
        assert!(another_file.exists());
    }

    #[test]
    fn test_cleanup_unselected_files_partial_directory_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");

        // Create a directory with multiple files
        let subdir = c_dir.join("subdir");
        fs::create_dir_all(&subdir).unwrap();

        let file1 = subdir.join("file1.c.c2rust");
        let file2 = subdir.join("file2.c.c2rust");
        let file3 = subdir.join("file3.c.c2rust");

        fs::write(&file1, "content1").unwrap();
        fs::write(&file2, "content2").unwrap();
        fs::write(&file3, "content3").unwrap();

        let all_files = vec![
            PreprocessedFileInfo {
                path: file1.clone(),
                display_name: "subdir/file1.c.c2rust".to_string(),
            },
            PreprocessedFileInfo {
                path: file2.clone(),
                display_name: "subdir/file2.c.c2rust".to_string(),
            },
            PreprocessedFileInfo {
                path: file3.clone(),
                display_name: "subdir/file3.c.c2rust".to_string(),
            },
        ];

        // Select only file1, so file2 and file3 should be removed but subdir should remain
        let selected_files = vec![file1.clone()];

        cleanup_unselected_files(&all_files, &selected_files, &c_dir).unwrap();

        // file1 should exist
        assert!(file1.exists());

        // file2 and file3 should be removed
        assert!(!file2.exists());
        assert!(!file3.exists());

        // subdir should still exist (contains file1)
        assert!(subdir.exists());
    }

    #[test]
    fn test_cleanup_unselected_files_multiple_nested_directories() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");

        // Create multiple nested directory structures
        let dir1 = c_dir.join("dir1").join("subdir1");
        let dir2 = c_dir.join("dir2").join("subdir2");
        fs::create_dir_all(&dir1).unwrap();
        fs::create_dir_all(&dir2).unwrap();

        let file1 = dir1.join("file1.c.c2rust");
        let file2 = dir2.join("file2.c.c2rust");

        fs::write(&file1, "content1").unwrap();
        fs::write(&file2, "content2").unwrap();

        // Don't select any files - add a keeper file
        let keeper = c_dir.join("keeper.c.c2rust");
        fs::write(&keeper, "keep").unwrap();

        let all_files = vec![
            PreprocessedFileInfo {
                path: file1.clone(),
                display_name: "dir1/subdir1/file1.c.c2rust".to_string(),
            },
            PreprocessedFileInfo {
                path: file2.clone(),
                display_name: "dir2/subdir2/file2.c.c2rust".to_string(),
            },
            PreprocessedFileInfo {
                path: keeper.clone(),
                display_name: "keeper.c.c2rust".to_string(),
            },
        ];

        let selected_files = vec![keeper.clone()];

        cleanup_unselected_files(&all_files, &selected_files, &c_dir).unwrap();

        // Both file1 and file2 should be removed
        assert!(!file1.exists());
        assert!(!file2.exists());

        // All empty directories should be removed
        assert!(!dir1.exists());
        assert!(!c_dir.join("dir1").exists());
        assert!(!dir2.exists());
        assert!(!c_dir.join("dir2").exists());

        // c_dir should still exist
        assert!(c_dir.exists());
        assert!(keeper.exists());
    }

    #[test]
    fn test_cleanup_unselected_files_respects_base_dir_boundary() {
        let temp_dir = TempDir::new().unwrap();
        let parent_dir = temp_dir.path().join("parent");
        let c_dir = parent_dir.join("c");

        // Create nested directory structure
        let subdir = c_dir.join("subdir");
        fs::create_dir_all(&subdir).unwrap();

        // Create a file in subdirectory
        let file1 = subdir.join("file1.c.c2rust");
        fs::write(&file1, "content1").unwrap();

        // Create another file to select (to avoid empty selection safety)
        let keeper = c_dir.join("keeper.c.c2rust");
        fs::write(&keeper, "keep").unwrap();

        let all_files = vec![
            PreprocessedFileInfo {
                path: file1.clone(),
                display_name: "subdir/file1.c.c2rust".to_string(),
            },
            PreprocessedFileInfo {
                path: keeper.clone(),
                display_name: "keeper.c.c2rust".to_string(),
            },
        ];

        // Select only keeper
        let selected_files = vec![keeper.clone()];

        cleanup_unselected_files(&all_files, &selected_files, &c_dir).unwrap();

        // file1 and subdir should be removed
        assert!(!file1.exists());
        assert!(!subdir.exists());

        // c_dir should still exist (it's the base_dir boundary)
        assert!(c_dir.exists());

        // parent_dir should definitely still exist (above base_dir boundary)
        assert!(parent_dir.exists());
        assert!(keeper.exists());
    }
}
