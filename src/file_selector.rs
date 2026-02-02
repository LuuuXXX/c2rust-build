use crate::error::{Error, Result};
use dialoguer::{theme::ColorfulTheme, MultiSelect};
use std::fs;
use std::path::{Path, PathBuf};

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
            let has_valid_extension = path.extension()
                .and_then(|ext| ext.to_str())
                .map_or(false, |ext| ext == "c2rust" || ext == "i" || ext == "ii");
            
            if has_valid_extension {
                if let Ok(relative_path) = path.strip_prefix(base_dir) {
                    let display_name = relative_path.display().to_string();
                    files.push(PreprocessedFileInfo {
                        path,
                        display_name,
                    });
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
) -> Result<Vec<PathBuf>> {
    if files.is_empty() {
        println!("No preprocessed files found.");
        return Ok(Vec::new());
    }
    
    // Check if we should skip interactive selection
    let should_skip_interactive = no_interactive || !is_terminal();
    
    if should_skip_interactive {
        println!("Non-interactive mode: selecting all {} file(s)", files.len());
        let all_files: Vec<PathBuf> = files.into_iter().map(|f| f.path).collect();
        return Ok(all_files);
    }
    
    println!("\n=== File Selection ===");
    println!("Found {} preprocessed file(s)", files.len());
    println!("Use SPACE to select/deselect, ENTER to confirm, ESC to cancel");
    println!();
    
    let items: Vec<String> = files.iter().map(|f| f.display_name.clone()).collect();
    
    // All files are selected by default
    let defaults: Vec<bool> = vec![true; files.len()];
    
    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select files to translate")
        .items(&items)
        .defaults(&defaults)
        .interact()
        .map_err(|e| {
            Error::FileSelectionCancelled(format!("{}", e))
        })?;
    
    let selected_files: Vec<PathBuf> = selections
        .into_iter()
        .map(|idx| files[idx].path.clone())
        .collect();
    
    println!("\nSelected {} file(s) for translation", selected_files.len());
    
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
}