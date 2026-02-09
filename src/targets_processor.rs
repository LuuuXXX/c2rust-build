use crate::error::{Error, Result};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Process and clean the targets.list file to ensure it only contains valid binary targets
pub fn process_targets_list(project_root: &Path, feature: &str) -> Result<()> {
    let targets_list_path = project_root
        .join(".c2rust")
        .join(feature)
        .join("c")
        .join("targets.list");

    if !targets_list_path.exists() {
        // If targets.list doesn't exist, scan for binaries and create it
        let binaries = scan_for_binaries(project_root)?;
        write_targets_list(&targets_list_path, &binaries)?;
        return Ok(());
    }

    // Read existing targets.list
    let content = fs::read_to_string(&targets_list_path).map_err(|e| {
        Error::CommandExecutionFailed(format!("Failed to read targets.list: {}", e))
    })?;

    // Parse targets
    let mut targets: Vec<String> = content
        .lines()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    // Filter out invalid targets (object files, etc.) and remove duplicates
    let mut seen = HashSet::new();
    targets.retain(|target| {
        if seen.contains(target) {
            return false; // Remove duplicates
        }
        seen.insert(target.clone());

        // Keep only:
        // - Static libraries (.a files starting with "lib")
        // - Shared libraries (.so files or files containing ".so.")
        // - Executables (no extension or not .o/.c/.h/.cpp, etc.)
        if target.ends_with(".o") || target.ends_with(".c") || target.ends_with(".h") {
            return false; // Skip object files and source files
        }

        // Accept .a files (static libraries)
        if target.ends_with(".a") && target.starts_with("lib") {
            return true;
        }

        // Accept .so files (shared libraries)
        if target.ends_with(".so") || target.contains(".so.") {
            return true;
        }

        // Accept executables (files without typical source/intermediate extensions)
        let extensions_to_skip = [
            ".o", ".a", ".so", ".c", ".cpp", ".cc", ".cxx", ".h", ".hpp", ".hxx",
        ];
        let has_skippable_ext = extensions_to_skip
            .iter()
            .any(|ext| target.ends_with(ext) && !target.ends_with(".so"));

        !has_skippable_ext
    });

    // Additionally scan project directory for binaries to ensure we haven't missed any
    let scanned_binaries = scan_for_binaries(project_root)?;

    // Merge with existing targets (avoiding duplicates)
    for binary in scanned_binaries {
        if !targets.contains(&binary) {
            targets.push(binary);
        }
    }

    // Sort for consistency
    targets.sort();

    // Write back to targets.list
    write_targets_list(&targets_list_path, &targets)?;

    Ok(())
}

/// Scan project directory for binary files (.a, .so, and executables)
fn scan_for_binaries(project_root: &Path) -> Result<Vec<String>> {
    let mut binaries = Vec::new();

    // Skip scanning .c2rust directory and common build artifact directories
    let skip_dirs = [".c2rust", "target", ".git", "node_modules"];

    visit_dir(project_root, project_root, &mut binaries, &skip_dirs)?;

    // Deduplicate and sort
    binaries.sort();
    binaries.dedup();

    Ok(binaries)
}

/// Recursively visit directory to find binary files
fn visit_dir(
    dir: &Path,
    project_root: &Path,
    binaries: &mut Vec<String>,
    skip_dirs: &[&str],
) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(dir).map_err(|e| {
        Error::CommandExecutionFailed(format!("Failed to read directory {}: {}", dir.display(), e))
    })? {
        let entry = entry.map_err(|e| {
            Error::CommandExecutionFailed(format!("Failed to read directory entry: {}", e))
        })?;

        let path = entry.path();
        let metadata = entry.metadata().map_err(|e| {
            Error::CommandExecutionFailed(format!("Failed to get metadata for {}: {}", path.display(), e))
        })?;

        // Skip symlinks
        if metadata.file_type().is_symlink() {
            continue;
        }

        if metadata.is_dir() {
            // Check if this directory should be skipped
            if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                if skip_dirs.contains(&dir_name) {
                    continue;
                }
            }

            // Recursively visit subdirectory
            visit_dir(&path, project_root, binaries, skip_dirs)?;
        } else if metadata.is_file() {
            // Check if this is a binary file we should include
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if is_binary_target(file_name, &path)? {
                    binaries.push(file_name.to_string());
                }
            }
        }
    }

    Ok(())
}

/// Check if a file is a binary target (static lib, shared lib, or executable)
fn is_binary_target(file_name: &str, path: &Path) -> Result<bool> {
    // Static libraries (.a files starting with "lib")
    if file_name.ends_with(".a") && file_name.starts_with("lib") {
        return Ok(true);
    }

    // Shared libraries (.so files or files containing ".so.")
    if file_name.ends_with(".so") || file_name.contains(".so.") {
        return Ok(true);
    }

    // Check for executables (files with execute permission and no source extension)
    let source_extensions = [
        ".c", ".cpp", ".cc", ".cxx", ".h", ".hpp", ".hxx", ".o", ".a", ".so",
    ];

    // Skip files with source/intermediate extensions
    if source_extensions.iter().any(|ext| file_name.ends_with(ext)) {
        return Ok(false);
    }

    // Check if file is executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(path).map_err(|e| {
            Error::CommandExecutionFailed(format!("Failed to get metadata for {}: {}", path.display(), e))
        })?;
        let permissions = metadata.permissions();
        let mode = permissions.mode();

        // Check if any execute bit is set (owner, group, or other)
        if (mode & 0o111) != 0 {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Write targets to targets.list file
fn write_targets_list(path: &Path, targets: &[String]) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            Error::CommandExecutionFailed(format!(
                "Failed to create directory {}: {}",
                parent.display(),
                e
            ))
        })?;
    }

    let content = targets.join("\n");
    let content_with_newline = if content.is_empty() {
        String::new()
    } else {
        format!("{}\n", content)
    };

    fs::write(path, content_with_newline).map_err(|e| {
        Error::CommandExecutionFailed(format!("Failed to write targets.list: {}", e))
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_is_binary_target_static_lib() {
        assert!(is_binary_target("libfoo.a", Path::new("libfoo.a")).unwrap());
        assert!(!is_binary_target("foo.a", Path::new("foo.a")).unwrap()); // doesn't start with "lib"
    }

    #[test]
    fn test_is_binary_target_shared_lib() {
        assert!(is_binary_target("libfoo.so", Path::new("libfoo.so")).unwrap());
        assert!(is_binary_target("libfoo.so.1", Path::new("libfoo.so.1")).unwrap());
    }

    #[test]
    fn test_is_binary_target_object_file() {
        assert!(!is_binary_target("foo.o", Path::new("foo.o")).unwrap());
    }

    #[test]
    fn test_write_and_read_targets_list() {
        let temp_dir = TempDir::new().unwrap();
        let targets_list_path = temp_dir.path().join("targets.list");

        let targets = vec!["myapp".to_string(), "libfoo.a".to_string()];
        write_targets_list(&targets_list_path, &targets).unwrap();

        let content = fs::read_to_string(&targets_list_path).unwrap();
        assert_eq!(content, "myapp\nlibfoo.a\n");
    }
}
