use crate::error::Result;
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

    // Always scan project directory for binaries - make this the authoritative source
    // This ensures stale entries from previous builds are removed and initializes
    // targets.list if it does not yet exist.
    let binaries = scan_for_binaries(project_root)?;

    // Write the authoritative list to targets.list
    write_targets_list(&targets_list_path, &binaries)?;

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

    for entry in fs::read_dir(dir)? {
        let entry = entry?;

        let path = entry.path();
        
        // Use file_type() directly from DirEntry to avoid following symlinks
        let file_type = entry.file_type()?;

        // Skip symlinks to avoid cycles and redundant processing
        if file_type.is_symlink() {
            continue;
        }

        let metadata = entry.metadata()?;

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
                // Pass metadata to avoid double syscall
                if is_binary_target(file_name, &path, &metadata)? {
                    // Store relative path instead of just basename to handle duplicates
                    if let Ok(rel_path) = path.strip_prefix(project_root) {
                        // Use display() so paths with non-UTF-8 components are still recorded
                        binaries.push(rel_path.display().to_string());
                    }
                }
            }
        }
    }

    Ok(())
}

/// Check if a file is a binary target (static lib, shared lib, or executable)
fn is_binary_target(file_name: &str, path: &Path, metadata: &fs::Metadata) -> Result<bool> {
    // Static libraries (.a files starting with "lib")
    if file_name.ends_with(".a") && file_name.starts_with("lib") {
        return Ok(true);
    }

    // Shared libraries (.so files or versioned .so.N, .so.N.M, etc.)
    if file_name.ends_with(".so") {
        return Ok(true);
    }
    // Check for versioned shared libraries like .so.1, .so.1.2, etc.
    if let Some(so_pos) = file_name.rfind(".so.") {
        let after_so = &file_name[so_pos + 4..]; // Skip ".so."
        // Check if all remaining parts are numeric (e.g., "1" or "1.2" or "1.2.3")
        if !after_so.is_empty() && after_so.split('.').all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit())) {
            return Ok(true);
        }
    }

    // Skip files with non-binary extensions
    if NON_BINARY_EXTENSIONS.iter().any(|ext| file_name.ends_with(ext)) {
        return Ok(false);
    }

    // Check if file is executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
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
        // If we can't open it, conservatively treat it as a script so it gets excluded
        Err(_) => return Ok(true),
    };
    
    let mut buffer = [0u8; 2];
    match file.read_exact(&mut buffer) {
        Ok(_) => Ok(buffer == *b"#!"),
        // If we can't read it, conservatively treat it as a script so it gets excluded
        Err(_) => Ok(true),
    }
}

/// Write targets to targets.list file
fn write_targets_list(path: &Path, targets: &[String]) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let content = targets.join("\n");
    let content_with_newline = if content.is_empty() {
        String::new()
    } else {
        format!("{}\n", content)
    };

    fs::write(path, content_with_newline)?;

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
        let metadata = fs::metadata(&lib_path).unwrap();
        
        assert!(is_binary_target("libfoo.a", &lib_path, &metadata).unwrap());
        
        // Files not starting with "lib" should not be considered static libs
        // but since they don't match source extensions and aren't executable,
        // they won't be accepted as binaries either
        let non_lib_path = temp_dir.path().join("foo.a");
        fs::write(&non_lib_path, "dummy").unwrap();
        let metadata = fs::metadata(&non_lib_path).unwrap();
        assert!(!is_binary_target("foo.a", &non_lib_path, &metadata).unwrap());
    }

    #[test]
    fn test_is_binary_target_shared_lib() {
        let temp_dir = TempDir::new().unwrap();
        let so_path = temp_dir.path().join("libfoo.so");
        fs::write(&so_path, "dummy").unwrap();
        let metadata = fs::metadata(&so_path).unwrap();
        assert!(is_binary_target("libfoo.so", &so_path, &metadata).unwrap());
        
        // Test versioned shared libraries
        let versioned_so_path = temp_dir.path().join("libfoo.so.1");
        fs::write(&versioned_so_path, "dummy").unwrap();
        let metadata = fs::metadata(&versioned_so_path).unwrap();
        assert!(is_binary_target("libfoo.so.1", &versioned_so_path, &metadata).unwrap());
        
        let multi_versioned_so_path = temp_dir.path().join("libfoo.so.1.2.3");
        fs::write(&multi_versioned_so_path, "dummy").unwrap();
        let metadata = fs::metadata(&multi_versioned_so_path).unwrap();
        assert!(is_binary_target("libfoo.so.1.2.3", &multi_versioned_so_path, &metadata).unwrap());
        
        // Test that non-numeric versions are excluded (e.g., .so.old, .so.backup)
        let backup_so_path = temp_dir.path().join("libfoo.so.old");
        fs::write(&backup_so_path, "dummy").unwrap();
        let metadata = fs::metadata(&backup_so_path).unwrap();
        assert!(!is_binary_target("libfoo.so.old", &backup_so_path, &metadata).unwrap());
        
        let backup2_so_path = temp_dir.path().join("libfoo.so.backup");
        fs::write(&backup2_so_path, "dummy").unwrap();
        let metadata = fs::metadata(&backup2_so_path).unwrap();
        assert!(!is_binary_target("libfoo.so.backup", &backup2_so_path, &metadata).unwrap());
    }

    #[test]
    fn test_is_binary_target_object_file() {
        let temp_dir = TempDir::new().unwrap();
        let obj_path = temp_dir.path().join("foo.o");
        fs::write(&obj_path, "dummy").unwrap();
        let metadata = fs::metadata(&obj_path).unwrap();
        assert!(!is_binary_target("foo.o", &obj_path, &metadata).unwrap());
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

    #[test]
    #[cfg(unix)]
    fn test_scan_for_binaries_comprehensive() {
        use std::os::unix::fs::PermissionsExt;
        
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create nested directory structure
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("lib")).unwrap();
        fs::create_dir_all(root.join("build")).unwrap();
        fs::create_dir_all(root.join(".c2rust")).unwrap(); // Should be skipped
        fs::create_dir_all(root.join("target")).unwrap(); // Should be skipped

        // Create static library
        fs::write(root.join("lib/libmath.a"), "library").unwrap();

        // Create shared library
        fs::write(root.join("lib/libfoo.so"), "shared").unwrap();

        // Create versioned shared library
        fs::write(root.join("lib/libbar.so.1"), "versioned").unwrap();

        // Create executable binary (with execute bit)
        let exe_path = root.join("bin/myapp");
        fs::write(&exe_path, "binary").unwrap();
        let mut perms = fs::metadata(&exe_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&exe_path, perms).unwrap();

        // Create executable script with shebang (should be excluded)
        let script_path = root.join("bin/run-tests");
        fs::write(&script_path, "#!/bin/bash\necho test").unwrap();
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();

        // Create object file (should be excluded)
        fs::write(root.join("build/foo.o"), "object").unwrap();

        // Create source file (should be excluded)
        fs::write(root.join("build/main.c"), "source").unwrap();

        // Create binary in skipped directory (should be excluded)
        let skipped_exe = root.join(".c2rust/test");
        fs::write(&skipped_exe, "binary").unwrap();
        let mut perms = fs::metadata(&skipped_exe).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&skipped_exe, perms).unwrap();

        // Create symlink (should be skipped)
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("bin"), root.join("link_to_bin")).unwrap();

        // Scan for binaries
        let binaries = scan_for_binaries(root).unwrap();

        // Should find static lib, shared libs, and executable (but not script)
        assert!(binaries.contains(&"lib/libmath.a".to_string()), "Should find static library");
        assert!(binaries.contains(&"lib/libfoo.so".to_string()), "Should find shared library");
        assert!(binaries.contains(&"lib/libbar.so.1".to_string()), "Should find versioned shared library");
        assert!(binaries.contains(&"bin/myapp".to_string()), "Should find executable");

        // Should NOT find script, object file, source file, or files in skipped dirs
        assert!(!binaries.iter().any(|b| b.contains("run-tests")), "Should exclude script with shebang");
        assert!(!binaries.iter().any(|b| b.contains(".o")), "Should exclude object files");
        assert!(!binaries.iter().any(|b| b.contains(".c")), "Should exclude source files");
        assert!(!binaries.iter().any(|b| b.contains(".c2rust")), "Should exclude files in .c2rust");

        // Check that list is sorted
        let mut sorted = binaries.clone();
        sorted.sort();
        assert_eq!(binaries, sorted, "Binaries should be sorted");
    }

    #[test]
    #[cfg(unix)]
    fn test_scan_handles_duplicate_names() {
        use std::os::unix::fs::PermissionsExt;
        
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create two directories with binaries of the same name
        fs::create_dir_all(root.join("build")).unwrap();
        fs::create_dir_all(root.join("bin")).unwrap();

        // Create executable files with the same name in different directories
        let build_foo = root.join("build/foo");
        fs::write(&build_foo, "build version").unwrap();
        let mut perms = fs::metadata(&build_foo).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&build_foo, perms).unwrap();

        let bin_foo = root.join("bin/foo");
        fs::write(&bin_foo, "bin version").unwrap();
        let mut perms = fs::metadata(&bin_foo).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&bin_foo, perms).unwrap();

        // Both should be found with their relative paths
        let binaries = scan_for_binaries(root).unwrap();
        
        assert!(binaries.contains(&"build/foo".to_string()) || binaries.contains(&"bin/foo".to_string()),
                "Should find binaries with relative paths");
        
        // Both paths should be present (no deduplication by basename)
        let foo_count = binaries.iter().filter(|b| b.ends_with("foo")).count();
        assert_eq!(foo_count, 2, "Should have 2 entries for 'foo' in different directories");
    }

    #[test]
    fn test_process_targets_list_authoritative() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create directory structure
        fs::create_dir_all(root.join(".c2rust/default/c")).unwrap();
        
        // Create a static library
        fs::write(root.join("libfoo.a"), "library").unwrap();

        // Write an initial targets.list with a stale entry
        let targets_list_path = root.join(".c2rust/default/c/targets.list");
        fs::write(&targets_list_path, "libfoo.a\nold_binary\n").unwrap();

        // Process targets list
        process_targets_list(root, "default").unwrap();

        // Read the result
        let content = fs::read_to_string(&targets_list_path).unwrap();
        
        // Should only contain libfoo.a, old_binary should be removed
        assert!(content.contains("libfoo.a"), "Should contain current binary");
        assert!(!content.contains("old_binary"), "Should not contain stale entry");
    }
}
