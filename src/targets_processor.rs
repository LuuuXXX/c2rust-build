use crate::error::{Error, Result};
use std::fs;
use std::io::Read;
use std::path::Path;

// Combined list of all non-binary file extensions (source, headers, objects, scripts)
const NON_BINARY_EXTENSIONS: &[&str] = &[
    // Source files
    ".c", ".cpp", ".cc", ".cxx",
    // Headers
    ".h", ".hpp", ".hxx",
    // Object files
    ".o",
    // Scripts
    ".sh", ".bash", ".py", ".pl", ".rb", ".lua", ".js", ".ts",
];

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

    // Scan project directory for binaries - make this the authoritative source
    // This ensures stale entries from previous builds are removed
    let targets = scan_for_binaries(project_root)?;

    // Write the authoritative list to targets.list
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
    _project_root: &Path,
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
        
        // Use file_type() directly from DirEntry to avoid following symlinks
        let file_type = entry.file_type().map_err(|e| {
            Error::CommandExecutionFailed(format!("Failed to get file type for {}: {}", path.display(), e))
        })?;

        // Skip symlinks to avoid cycles and redundant processing
        if file_type.is_symlink() {
            continue;
        }

        let metadata = entry.metadata().map_err(|e| {
            Error::CommandExecutionFailed(format!("Failed to get metadata for {}: {}", path.display(), e))
        })?;

        if metadata.is_dir() {
            // Check if this directory should be skipped
            if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                if skip_dirs.contains(&dir_name) {
                    continue;
                }
            }

            // Recursively visit subdirectory
            visit_dir(&path, _project_root, binaries, skip_dirs)?;
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

    // Skip files with non-binary extensions
    if NON_BINARY_EXTENSIONS.iter().any(|ext| file_name.ends_with(ext)) {
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
            // Check if this is a script by looking for shebang
            if is_script_file(path)? {
                return Ok(false);
            }
            return Ok(true);
        }
    }

    Ok(false)
}

/// Check if a file is a script by looking for shebang (#!)
fn is_script_file(path: &Path) -> Result<bool> {
    let mut file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Ok(false), // If we can't open it, assume it's not a script
    };
    
    let mut buffer = [0u8; 2];
    match file.read_exact(&mut buffer) {
        Ok(_) => Ok(buffer == *b"#!"),
        Err(_) => Ok(false), // If we can't read it, assume it's not a script
    }
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
        // Static libraries are identified by name pattern, no file access needed
        let temp_dir = TempDir::new().unwrap();
        let lib_path = temp_dir.path().join("libfoo.a");
        fs::write(&lib_path, "dummy").unwrap();
        
        assert!(is_binary_target("libfoo.a", &lib_path).unwrap());
        
        // Files not starting with "lib" should not be considered static libs
        // but since they don't match source extensions and aren't executable,
        // they won't be accepted as binaries either
        let non_lib_path = temp_dir.path().join("foo.a");
        fs::write(&non_lib_path, "dummy").unwrap();
        assert!(!is_binary_target("foo.a", &non_lib_path).unwrap());
    }

    #[test]
    fn test_is_binary_target_shared_lib() {
        let temp_dir = TempDir::new().unwrap();
        let so_path = temp_dir.path().join("libfoo.so");
        fs::write(&so_path, "dummy").unwrap();
        assert!(is_binary_target("libfoo.so", &so_path).unwrap());
        
        let versioned_so_path = temp_dir.path().join("libfoo.so.1");
        fs::write(&versioned_so_path, "dummy").unwrap();
        assert!(is_binary_target("libfoo.so.1", &versioned_so_path).unwrap());
    }

    #[test]
    fn test_is_binary_target_object_file() {
        let temp_dir = TempDir::new().unwrap();
        let obj_path = temp_dir.path().join("foo.o");
        fs::write(&obj_path, "dummy").unwrap();
        assert!(!is_binary_target("foo.o", &obj_path).unwrap());
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
