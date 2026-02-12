use crate::error::{Error, Result};
use dialoguer::{theme::ColorfulTheme, MultiSelect};
use std::collections::{HashMap, HashSet};
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

/// Represents an item that can be selected (either a file or a directory)
#[derive(Debug, Clone)]
pub enum SelectableItem {
    /// A file item
    File {
        info: PreprocessedFileInfo,
        depth: usize,
    },
    /// A directory item
    Directory {
        path: PathBuf,
        display_name: String,
        depth: usize,
        /// Indices of child items in the items list
        child_indices: Vec<usize>,
    },
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

/// Build a hierarchical tree structure from collected files
/// Returns a list of SelectableItems with proper depth and parent-child relationships
/// Items are returned in preorder (parent -> children) for proper tree display
fn build_hierarchical_items(
    files: &[PreprocessedFileInfo],
    base_dir: &Path,
) -> Vec<SelectableItem> {
    /// Helper to calculate depth for a path relative to base_dir
    /// First-level items (directly under base_dir) have depth 0
    fn depth_from_base(path: &Path, base_dir: &Path) -> usize {
        path.strip_prefix(base_dir)
            .ok()
            .map(|rel| rel.components().count().saturating_sub(1))
            .unwrap_or(0)
    }
    
    // Temporary node representing a directory and its immediate children
    struct TempDirNode {
        basename: String,
        depth: usize,
        child_dirs: Vec<PathBuf>,
        file_infos: Vec<PreprocessedFileInfo>,
    }

    let mut items: Vec<SelectableItem> = Vec::new();
    let mut dir_index_map: HashMap<PathBuf, usize> = HashMap::new();
    
    // Collect all unique directories from the files
    let mut all_dirs: HashSet<PathBuf> = HashSet::new();
    for file_info in files {
        if let Some(parent) = file_info.path.parent() {
            let mut current = parent;
            while current != base_dir {
                all_dirs.insert(current.to_path_buf());
                if let Some(p) = current.parent() {
                    current = p;
                } else {
                    break;
                }
            }
        }
    }
    
    // Sort directories by depth and path for deterministic ordering
    let mut sorted_dirs: Vec<PathBuf> = all_dirs.into_iter().collect();
    sorted_dirs.sort_by(|a, b| {
        let depth_a = a.components().count();
        let depth_b = b.components().count();
        depth_a
            .cmp(&depth_b)
            .then_with(|| {
                let rel_a = a.strip_prefix(base_dir).unwrap_or(a.as_path());
                let rel_b = b.strip_prefix(base_dir).unwrap_or(b.as_path());
                rel_a.to_string_lossy().cmp(&rel_b.to_string_lossy())
            })
    });
    
    // Map each directory path to its TempDirNode
    let mut dir_nodes: HashMap<PathBuf, TempDirNode> = HashMap::new();

    // Initialize directory nodes with basic metadata
    for dir_path in &sorted_dirs {
        let depth = depth_from_base(dir_path, base_dir);

        let basename = dir_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| dir_path.to_string_lossy().into_owned());

        dir_nodes.insert(
            dir_path.clone(),
            TempDirNode {
                basename,
                depth,
                child_dirs: Vec::new(),
                file_infos: Vec::new(),
            },
        );
    }

    // Populate child directory relationships
    for dir_path in &sorted_dirs {
        if let Some(parent) = dir_path.parent() {
            if parent != base_dir && dir_nodes.contains_key(parent) {
                if let Some(parent_node) = dir_nodes.get_mut(parent) {
                    parent_node.child_dirs.push(dir_path.clone());
                }
            }
        }
    }
    
    // Sort child directories in each node for deterministic ordering
    for node in dir_nodes.values_mut() {
        node.child_dirs.sort_by(|a, b| {
            let rel_a = a.strip_prefix(base_dir).unwrap_or(a.as_path());
            let rel_b = b.strip_prefix(base_dir).unwrap_or(b.as_path());
            rel_a.to_string_lossy().cmp(&rel_b.to_string_lossy())
        });
    }

    // Associate files with their parent directories or track as root-level files
    let mut root_files: Vec<PreprocessedFileInfo> = Vec::new();
    for file_info in files {
        if let Some(parent) = file_info.path.parent() {
            if let Some(node) = dir_nodes.get_mut(parent) {
                node.file_infos.push(file_info.clone());
            } else {
                root_files.push(file_info.clone());
            }
        } else {
            root_files.push(file_info.clone());
        }
    }
    
    // Sort files in each node for deterministic ordering
    for node in dir_nodes.values_mut() {
        node.file_infos.sort_by(|a, b| {
            a.path.to_string_lossy().cmp(&b.path.to_string_lossy())
        });
    }

    // Helper to recursively build items in preorder for a directory subtree
    fn build_dir_tree(
        dir_path: &Path,
        dir_nodes: &HashMap<PathBuf, TempDirNode>,
        base_dir: &Path,
        items: &mut Vec<SelectableItem>,
        dir_index_map: &mut HashMap<PathBuf, usize>,
    ) {
        if let Some(node) = dir_nodes.get(dir_path) {
            // Insert the directory itself
            let dir_index = items.len();
            items.push(SelectableItem::Directory {
                path: dir_path.to_path_buf(),
                display_name: node.basename.clone(),
                depth: node.depth,
                child_indices: Vec::new(),
            });
            dir_index_map.insert(dir_path.to_path_buf(), dir_index);

            let mut child_indices: Vec<usize> = Vec::new();

            // First, recursively add child directories (already sorted in the node)
            for child_dir in &node.child_dirs {
                build_dir_tree(child_dir, dir_nodes, base_dir, items, dir_index_map);
                if let Some(&child_idx) = dir_index_map.get(child_dir) {
                    child_indices.push(child_idx);
                }
            }

            // Then, add this directory's files (already sorted in the node)
            for file_info in &node.file_infos {
                let depth = depth_from_base(&file_info.path, base_dir);

                let file_index = items.len();
                items.push(SelectableItem::File {
                    info: file_info.clone(),
                    depth,
                });
                child_indices.push(file_index);
            }

            // Update the directory's child_indices to reflect the final order
            if let Some(SelectableItem::Directory { child_indices: existing_child_indices, .. }) =
                items.get_mut(dir_index)
            {
                *existing_child_indices = child_indices;
            }
        }
    }

    // Determine root directories (those whose parent is base_dir or not in the map)
    let mut root_dirs: Vec<PathBuf> = Vec::new();
    for dir_path in &sorted_dirs {
        let is_root = dir_path
            .parent()
            .map(|p| p == base_dir || !dir_nodes.contains_key(p))
            .unwrap_or(true);
        if is_root {
            root_dirs.push(dir_path.clone());
        }
    }

    // Sort root directories for deterministic ordering
    root_dirs.sort_by(|a, b| {
        let rel_a = a.strip_prefix(base_dir).unwrap_or(a.as_path());
        let rel_b = b.strip_prefix(base_dir).unwrap_or(b.as_path());
        rel_a.to_string_lossy().cmp(&rel_b.to_string_lossy())
    });

    // Build the final items list in preorder starting from each root directory
    for dir_path in root_dirs {
        build_dir_tree(&dir_path, &dir_nodes, base_dir, &mut items, &mut dir_index_map);
    }

    // Finally, append root-level files (sorted for determinism)
    root_files.sort_by(|a, b| {
        a.path.to_string_lossy().cmp(&b.path.to_string_lossy())
    });
    
    for file_info in root_files {
        let depth = depth_from_base(&file_info.path, base_dir);

        items.push(SelectableItem::File {
            info: file_info,
            depth,
        });
    }
    
    items
}

/// Format a selectable item for display with tree characters
fn format_item_display(item: &SelectableItem) -> String {
    match item {
        SelectableItem::File { info, depth } => {
            let indent = "  ".repeat(*depth);
            let file_name = info
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| info.display_name.clone());
            format!("{}📄 {}", indent, file_name)
        }
        SelectableItem::Directory { display_name, depth, .. } => {
            let indent = "  ".repeat(*depth);
            format!("{}📁 {}/", indent, display_name)
        }
    }
}

/// Present an interactive file selection UI to the user
/// Returns the list of selected file paths
/// If in non-interactive mode (no_interactive=true or not a TTY), selects all files
/// 
/// # Parameters
/// - `files`: List of preprocessed files
/// - `c_dir`: Base directory for building hierarchical structure
/// - `no_interactive`: Whether to skip interactive mode
/// - `selected_target`: Optional target name for display
pub fn select_files_interactive(
    files: Vec<PreprocessedFileInfo>,
    c_dir: &Path,
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
            "\x1b[1m选择参与构建 target '{}' 的文件或文件夹 | Select files or folders that participate in building target '{}'\x1b[0m",
            target, target
        );
    } else {
        println!("\x1b[1m选择要翻译的文件或文件夹 | Select files or folders to translate\x1b[0m");
    }
    println!("Use SPACE to select/deselect, ENTER to confirm, ESC to cancel");
    println!("Selecting a folder (📁) means all files inside it will be included after you confirm");
    println!();

    // Build hierarchical structure
    let selectable_items = build_hierarchical_items(&files, c_dir);
    
    // Format items for display
    let display_items: Vec<String> = selectable_items
        .iter()
        .map(format_item_display)
        .collect();

    // All items are selected by default
    let defaults: Vec<bool> = vec![true; selectable_items.len()];

    let prompt_text = if let Some(target) = selected_target {
        format!(
            "Select files/folders that participate in building target '{}'",
            target
        )
    } else {
        "Select files/folders to translate".to_string()
    };

    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt(&prompt_text)
        .items(&display_items)
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

    // Process selections: expand directories to include their files, 
    // but respect explicitly deselected items
    let total_items = selectable_items.len();
    let selected_set: HashSet<usize> = selections.into_iter().collect();
    
    // Compute explicitly deselected indices (were defaults but not in selection)
    let deselected_indices: HashSet<usize> = (0..total_items)
        .filter(|i| !selected_set.contains(i))
        .collect();
    
    let mut final_selected: HashSet<usize> = selected_set.clone();
    
    // Optimize: If all items are selected, skip expansion to avoid quadratic behavior
    if selected_set.len() < total_items {
        // Expand directory selections to include child files, but skip explicitly deselected items
        // Only expand directories that don't have a selected ancestor to avoid redundant work
        for &idx in &selected_set {
            if let SelectableItem::Directory { child_indices, .. } = &selectable_items[idx] {
                let mut descendants = Vec::new();
                collect_all_descendants(&selectable_items, child_indices, &mut descendants);
                
                // Add descendants that weren't explicitly deselected
                for desc_idx in descendants {
                    if !deselected_indices.contains(&desc_idx) {
                        final_selected.insert(desc_idx);
                    }
                }
            }
        }
    }
    
    // Extract file paths from selected items in deterministic order (by index)
    let mut selected_files: Vec<PathBuf> = Vec::new();
    for (idx, item) in selectable_items.iter().enumerate() {
        if final_selected.contains(&idx) {
            if let SelectableItem::File { info, .. } = item {
                selected_files.push(info.path.clone());
            }
        }
    }

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

/// Recursively collect all descendant indices from a list of child indices
fn collect_all_descendants(
    items: &[SelectableItem],
    child_indices: &[usize],
    result: &mut Vec<usize>,
) {
    for &idx in child_indices {
        result.push(idx);
        if let SelectableItem::Directory { child_indices: nested_children, .. } = &items[idx] {
            collect_all_descendants(items, nested_children, result);
        }
    }
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
        select_files_interactive(preprocessed_files.clone(), c_dir, no_interactive, selected_target)?;

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

    #[test]
    fn test_build_hierarchical_items_empty() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        fs::create_dir_all(&c_dir).unwrap();

        let files: Vec<PreprocessedFileInfo> = vec![];
        let items = build_hierarchical_items(&files, &c_dir);
        
        assert_eq!(items.len(), 0);
    }

    #[test]
    fn test_build_hierarchical_items_flat_structure() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        fs::create_dir_all(&c_dir).unwrap();

        let file1 = c_dir.join("file1.c.c2rust");
        let file2 = c_dir.join("file2.c.c2rust");
        
        let files = vec![
            PreprocessedFileInfo {
                path: file1.clone(),
                display_name: "file1.c.c2rust".to_string(),
            },
            PreprocessedFileInfo {
                path: file2.clone(),
                display_name: "file2.c.c2rust".to_string(),
            },
        ];

        let items = build_hierarchical_items(&files, &c_dir);
        
        // Should only have 2 files, no directories
        assert_eq!(items.len(), 2);
        
        // Both should be files
        for item in items {
            assert!(matches!(item, SelectableItem::File { .. }));
        }
    }

    #[test]
    fn test_build_hierarchical_items_nested_structure() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        fs::create_dir_all(&c_dir).unwrap();

        let subdir = c_dir.join("src");
        fs::create_dir_all(&subdir).unwrap();
        
        let file1 = c_dir.join("main.c.c2rust");
        let file2 = subdir.join("helper.c.c2rust");
        
        let files = vec![
            PreprocessedFileInfo {
                path: file1.clone(),
                display_name: "main.c.c2rust".to_string(),
            },
            PreprocessedFileInfo {
                path: file2.clone(),
                display_name: "src/helper.c.c2rust".to_string(),
            },
        ];

        let items = build_hierarchical_items(&files, &c_dir);
        
        // Should have: 1 directory (src) + 2 files = 3 items
        assert_eq!(items.len(), 3);
        
        // Count directories and files
        let dir_count = items.iter().filter(|i| matches!(i, SelectableItem::Directory { .. })).count();
        let file_count = items.iter().filter(|i| matches!(i, SelectableItem::File { .. })).count();
        
        assert_eq!(dir_count, 1);
        assert_eq!(file_count, 2);
        
        // Check that the directory has child indices
        let dir_item = items.iter().find(|i| matches!(i, SelectableItem::Directory { .. })).unwrap();
        if let SelectableItem::Directory { child_indices, .. } = dir_item {
            assert_eq!(child_indices.len(), 1); // Should have 1 child file
        }
    }

    #[test]
    fn test_build_hierarchical_items_deeply_nested() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        fs::create_dir_all(&c_dir).unwrap();

        let deep_dir = c_dir.join("a").join("b").join("c");
        fs::create_dir_all(&deep_dir).unwrap();
        
        let file1 = deep_dir.join("file.c.c2rust");
        
        let files = vec![
            PreprocessedFileInfo {
                path: file1.clone(),
                display_name: "a/b/c/file.c.c2rust".to_string(),
            },
        ];

        let items = build_hierarchical_items(&files, &c_dir);
        
        // Should have: 3 directories (a, b, c) + 1 file = 4 items
        assert_eq!(items.len(), 4);
        
        let dir_count = items.iter().filter(|i| matches!(i, SelectableItem::Directory { .. })).count();
        let file_count = items.iter().filter(|i| matches!(i, SelectableItem::File { .. })).count();
        
        assert_eq!(dir_count, 3);
        assert_eq!(file_count, 1);
    }

    #[test]
    fn test_format_item_display_file() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        let file_path = c_dir.join("test.c.c2rust");
        
        let item = SelectableItem::File {
            info: PreprocessedFileInfo {
                path: file_path,
                display_name: "test.c.c2rust".to_string(),
            },
            depth: 0,
        };
        
        let display = format_item_display(&item);
        assert_eq!(display, "📄 test.c.c2rust");
    }

    #[test]
    fn test_format_item_display_file_with_depth() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        let file_path = c_dir.join("src").join("test.c.c2rust");
        
        let item = SelectableItem::File {
            info: PreprocessedFileInfo {
                path: file_path,
                display_name: "src/test.c.c2rust".to_string(),
            },
            depth: 2,
        };
        
        let display = format_item_display(&item);
        assert_eq!(display, "    📄 test.c.c2rust");
    }

    #[test]
    fn test_format_item_display_directory() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        let dir_path = c_dir.join("src");
        
        let item = SelectableItem::Directory {
            path: dir_path,
            display_name: "src".to_string(),
            depth: 1,
            child_indices: vec![],
        };
        
        let display = format_item_display(&item);
        assert_eq!(display, "  📁 src/");
    }

    #[test]
    fn test_collect_all_descendants_empty() {
        let items: Vec<SelectableItem> = vec![];
        let child_indices: Vec<usize> = vec![];
        let mut result = Vec::new();
        
        collect_all_descendants(&items, &child_indices, &mut result);
        
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_collect_all_descendants_flat() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        
        let items = vec![
            SelectableItem::File {
                info: PreprocessedFileInfo {
                    path: c_dir.join("file1.c.c2rust"),
                    display_name: "file1.c.c2rust".to_string(),
                },
                depth: 0,
            },
            SelectableItem::File {
                info: PreprocessedFileInfo {
                    path: c_dir.join("file2.c.c2rust"),
                    display_name: "file2.c.c2rust".to_string(),
                },
                depth: 0,
            },
        ];
        
        let child_indices = vec![0, 1];
        let mut result = Vec::new();
        
        collect_all_descendants(&items, &child_indices, &mut result);
        
        assert_eq!(result.len(), 2);
        assert_eq!(result, vec![0, 1]);
    }

    #[test]
    fn test_collect_all_descendants_nested() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        
        let items = vec![
            // Index 0: Directory with children at indices 1 and 2
            SelectableItem::Directory {
                path: c_dir.join("src"),
                display_name: "src".to_string(),
                depth: 1,
                child_indices: vec![1, 2],
            },
            // Index 1: File
            SelectableItem::File {
                info: PreprocessedFileInfo {
                    path: c_dir.join("src").join("file1.c.c2rust"),
                    display_name: "src/file1.c.c2rust".to_string(),
                },
                depth: 2,
            },
            // Index 2: File
            SelectableItem::File {
                info: PreprocessedFileInfo {
                    path: c_dir.join("src").join("file2.c.c2rust"),
                    display_name: "src/file2.c.c2rust".to_string(),
                },
                depth: 2,
            },
        ];
        
        let child_indices = vec![0]; // Start with directory
        let mut result = Vec::new();
        
        collect_all_descendants(&items, &child_indices, &mut result);
        
        // Should collect directory (0) and its children (1, 2)
        assert_eq!(result.len(), 3);
        assert!(result.contains(&0));
        assert!(result.contains(&1));
        assert!(result.contains(&2));
    }

    #[test]
    fn test_hierarchical_items_preorder_display() {
        // Test that items are in proper tree order (parent immediately followed by children)
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        fs::create_dir_all(&c_dir).unwrap();

        let src_dir = c_dir.join("src");
        let utils_dir = src_dir.join("utils");
        fs::create_dir_all(&utils_dir).unwrap();
        
        let file1 = utils_dir.join("helper.c.c2rust");
        let file2 = c_dir.join("main.c.c2rust");
        
        let files = vec![
            PreprocessedFileInfo {
                path: file1.clone(),
                display_name: "src/utils/helper.c.c2rust".to_string(),
            },
            PreprocessedFileInfo {
                path: file2.clone(),
                display_name: "main.c.c2rust".to_string(),
            },
        ];

        let items = build_hierarchical_items(&files, &c_dir);
        
        // Verify preorder: src -> utils -> helper.c.c2rust -> main.c.c2rust
        // Items should be: [src_dir, utils_dir, helper file, main file]
        assert_eq!(items.len(), 4);
        
        // First item should be src directory
        if let SelectableItem::Directory { display_name, depth, .. } = &items[0] {
            assert_eq!(display_name, "src");
            assert_eq!(*depth, 0); // First-level directory should be depth 0
        } else {
            panic!("Expected first item to be src directory");
        }
        
        // Second item should be utils directory (child of src)
        if let SelectableItem::Directory { display_name, depth, .. } = &items[1] {
            assert_eq!(display_name, "utils");
            assert_eq!(*depth, 1); // Second-level directory
        } else {
            panic!("Expected second item to be utils directory");
        }
        
        // Third item should be helper file (child of utils)
        if let SelectableItem::File { depth, .. } = &items[2] {
            assert_eq!(*depth, 2); // File at third level
        } else {
            panic!("Expected third item to be helper file");
        }
        
        // Fourth item should be main file (at root level)
        if let SelectableItem::File { depth, .. } = &items[3] {
            assert_eq!(*depth, 0); // Root-level file
        } else {
            panic!("Expected fourth item to be main file");
        }
    }

    #[test]
    fn test_directory_basename_formatting() {
        // Test that directory labels show only basename, not full path
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        fs::create_dir_all(&c_dir).unwrap();

        let nested_dir = c_dir.join("src").join("utils");
        fs::create_dir_all(&nested_dir).unwrap();
        
        let file = nested_dir.join("test.c.c2rust");
        
        let files = vec![
            PreprocessedFileInfo {
                path: file.clone(),
                display_name: "src/utils/test.c.c2rust".to_string(),
            },
        ];

        let items = build_hierarchical_items(&files, &c_dir);
        
        // Check that directories have basenames, not full paths
        for item in &items {
            if let SelectableItem::Directory { display_name, .. } = item {
                // Should be "src" or "utils", not "src/utils" or full path
                assert!(!display_name.contains('/'), 
                    "Directory display_name should be basename only, got: {}", display_name);
            }
        }
    }

    #[test]
    fn test_stable_deterministic_ordering() {
        // Test that the same file structure produces the same order every time
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        fs::create_dir_all(&c_dir).unwrap();

        let src_dir = c_dir.join("src");
        let lib_dir = c_dir.join("lib");
        fs::create_dir_all(&src_dir).unwrap();
        fs::create_dir_all(&lib_dir).unwrap();
        
        // Create files in non-alphabetical order
        let file1 = src_dir.join("zzz.c.c2rust");
        let file2 = src_dir.join("aaa.c.c2rust");
        let file3 = lib_dir.join("bbb.c.c2rust");
        
        let files = vec![
            PreprocessedFileInfo {
                path: file1.clone(),
                display_name: "src/zzz.c.c2rust".to_string(),
            },
            PreprocessedFileInfo {
                path: file2.clone(),
                display_name: "src/aaa.c.c2rust".to_string(),
            },
            PreprocessedFileInfo {
                path: file3.clone(),
                display_name: "lib/bbb.c.c2rust".to_string(),
            },
        ];

        // Build hierarchy multiple times
        let items1 = build_hierarchical_items(&files, &c_dir);
        let items2 = build_hierarchical_items(&files, &c_dir);
        
        // Should produce exactly the same order
        assert_eq!(items1.len(), items2.len());
        for (i, (item1, item2)) in items1.iter().zip(items2.iter()).enumerate() {
            match (item1, item2) {
                (SelectableItem::Directory { display_name: d1, .. }, 
                 SelectableItem::Directory { display_name: d2, .. }) => {
                    assert_eq!(d1, d2, "Directory order mismatch at index {}", i);
                }
                (SelectableItem::File { info: f1, .. }, 
                 SelectableItem::File { info: f2, .. }) => {
                    assert_eq!(f1.path, f2.path, "File order mismatch at index {}", i);
                }
                _ => panic!("Item type mismatch at index {}", i),
            }
        }
        
        // Verify lib comes before src (alphabetical)
        if let SelectableItem::Directory { display_name, .. } = &items1[0] {
            assert_eq!(display_name, "lib");
        }
        if let SelectableItem::Directory { display_name, .. } = &items1[2] {
            assert_eq!(display_name, "src");
        }
    }

    #[test]
    fn test_depth_starts_at_zero() {
        // Test that first-level items have depth 0
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        fs::create_dir_all(&c_dir).unwrap();

        let src_dir = c_dir.join("src");
        fs::create_dir_all(&src_dir).unwrap();
        
        let file1 = c_dir.join("root.c.c2rust"); // Root-level file
        let file2 = src_dir.join("nested.c.c2rust"); // Nested file
        
        let files = vec![
            PreprocessedFileInfo {
                path: file1.clone(),
                display_name: "root.c.c2rust".to_string(),
            },
            PreprocessedFileInfo {
                path: file2.clone(),
                display_name: "src/nested.c.c2rust".to_string(),
            },
        ];

        let items = build_hierarchical_items(&files, &c_dir);
        
        // First-level directory should have depth 0
        if let SelectableItem::Directory { depth, .. } = &items[0] {
            assert_eq!(*depth, 0, "First-level directory should have depth 0");
        }
        
        // Root-level file should have depth 0
        let root_file = items.iter().find(|item| {
            matches!(item, SelectableItem::File { info, .. } if info.path == file1)
        }).expect("Should find root file");
        
        if let SelectableItem::File { depth, .. } = root_file {
            assert_eq!(*depth, 0, "Root-level file should have depth 0");
        }
    }

    #[test]
    fn test_directory_expansion_includes_all_descendants() {
        // Test that selecting a directory includes all files within it
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        fs::create_dir_all(&c_dir).unwrap();

        let src_dir = c_dir.join("src");
        let utils_dir = src_dir.join("utils");
        fs::create_dir_all(&utils_dir).unwrap();
        
        let file1 = utils_dir.join("helper.c.c2rust");
        let file2 = utils_dir.join("util.c.c2rust");
        let file3 = c_dir.join("main.c.c2rust");
        
        let files = vec![
            PreprocessedFileInfo {
                path: file1.clone(),
                display_name: "src/utils/helper.c.c2rust".to_string(),
            },
            PreprocessedFileInfo {
                path: file2.clone(),
                display_name: "src/utils/util.c.c2rust".to_string(),
            },
            PreprocessedFileInfo {
                path: file3.clone(),
                display_name: "main.c.c2rust".to_string(),
            },
        ];

        let items = build_hierarchical_items(&files, &c_dir);
        
        // Find the src directory index
        let src_idx = items.iter().position(|item| {
            matches!(item, SelectableItem::Directory { display_name, .. } if display_name == "src")
        }).expect("Should find src directory");
        
        // Collect all descendants of src directory
        let mut descendants = Vec::new();
        if let SelectableItem::Directory { child_indices, .. } = &items[src_idx] {
            collect_all_descendants(&items, child_indices, &mut descendants);
        }
        
        // Should include utils directory and both files within
        assert!(descendants.len() >= 3, "Should have at least 3 descendants (utils dir + 2 files)");
        
        // Verify both files from utils are included
        let file_paths: Vec<PathBuf> = descendants.iter()
            .filter_map(|&idx| {
                if let SelectableItem::File { info, .. } = &items[idx] {
                    Some(info.path.clone())
                } else {
                    None
                }
            })
            .collect();
        
        assert!(file_paths.contains(&file1), "Should include helper.c.c2rust");
        assert!(file_paths.contains(&file2), "Should include util.c.c2rust");
    }

    #[test]
    fn test_explicit_deselection_respected() {
        // Test that explicitly deselected files stay deselected even if parent folder is selected
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        fs::create_dir_all(&c_dir).unwrap();

        let src_dir = c_dir.join("src");
        fs::create_dir_all(&src_dir).unwrap();
        
        let file1 = src_dir.join("keep.c.c2rust");
        let file2 = src_dir.join("exclude.c.c2rust");
        
        let files = vec![
            PreprocessedFileInfo {
                path: file1.clone(),
                display_name: "src/keep.c.c2rust".to_string(),
            },
            PreprocessedFileInfo {
                path: file2.clone(),
                display_name: "src/exclude.c.c2rust".to_string(),
            },
        ];

        let items = build_hierarchical_items(&files, &c_dir);
        
        // Simulate user selecting src directory but deselecting exclude.c.c2rust
        let src_idx = items.iter().position(|item| {
            matches!(item, SelectableItem::Directory { display_name, .. } if display_name == "src")
        }).expect("Should find src directory");
        
        let exclude_idx = items.iter().position(|item| {
            matches!(item, SelectableItem::File { info, .. } if info.path == file2)
        }).expect("Should find exclude file");
        
        let mut selected_set: HashSet<usize> = HashSet::new();
        selected_set.insert(src_idx);
        
        // Simulate all items selected by default except the one we deselected
        let deselected_indices: HashSet<usize> = vec![exclude_idx].into_iter().collect();
        
        let mut final_selected: HashSet<usize> = selected_set.clone();
        
        // Expand directory selections
        for &idx in &selected_set {
            if let SelectableItem::Directory { child_indices, .. } = &items[idx] {
                let mut descendants = Vec::new();
                collect_all_descendants(&items, child_indices, &mut descendants);
                
                for desc_idx in descendants {
                    if !deselected_indices.contains(&desc_idx) {
                        final_selected.insert(desc_idx);
                    }
                }
            }
        }
        
        // Verify exclude file is NOT in final selection
        assert!(!final_selected.contains(&exclude_idx), "Explicitly deselected file should not be included");
        
        // Verify keep file IS in final selection
        let keep_idx = items.iter().position(|item| {
            matches!(item, SelectableItem::File { info, .. } if info.path == file1)
        }).expect("Should find keep file");
        assert!(final_selected.contains(&keep_idx), "Non-deselected file should be included");
    }
}
