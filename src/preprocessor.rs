use crate::error::{Error, Result};
use crate::tracker::CompileEntry;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Represents a preprocessed file with its metadata.
///
/// The `original_path` and `preprocessed_path` fields are currently
/// written but not always read by the rest of the pipeline. They are
/// intentionally retained (and marked with `#[allow(dead_code)]`) so
/// that future features such as incremental rebuilds, preprocessing
/// caching, and richer diagnostics can access the full mapping
/// between original and preprocessed sources without changing this
/// type's public API.
#[derive(Debug, Clone)]
pub struct PreprocessedFile {
    /// Absolute path to the original source file before preprocessing.
    ///
    /// This may not be used by all current callers, but is preserved
    /// for future tooling that needs to relate diagnostics or cache
    /// entries back to the original source location.
    #[allow(dead_code)]
    original_path: PathBuf,
    /// Path to the generated preprocessed file on disk.
    ///
    /// Retained for potential future features (e.g., reusing
    /// preprocessed output across runs or debugging the preprocessor
    /// stage) even if it is not read in the current code.
    #[allow(dead_code)]
    preprocessed_path: PathBuf,
}

/// Get the clang path from environment variable or use default
fn get_clang_path() -> String {
    std::env::var("C2RUST_CLANG").unwrap_or_else(|_| "clang".to_string())
}

/// Verify that clang is available
pub fn verify_clang() -> Result<()> {
    let clang_path = get_clang_path();
    Command::new(&clang_path)
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|_| ())
        .ok_or(Error::ClangNotFound)
}

/// Preprocess C files using clang's -E flag
pub fn preprocess_files(
    entries: &[CompileEntry],
    feature: &str,
    project_root: &Path,
) -> Result<Vec<PreprocessedFile>> {
    let mut preprocessed = Vec::new();
    
    for entry in entries {
        let result = preprocess_file(entry, feature, project_root)?;
        preprocessed.push(result);
    }
    
    Ok(preprocessed)
}

/// Preprocess a single C file
fn preprocess_file(
    entry: &CompileEntry,
    feature: &str,
    project_root: &Path,
) -> Result<PreprocessedFile> {
    let file_path = entry.get_file_path();
    let full_file_path = if file_path.is_absolute() {
        file_path.clone()
    } else {
        entry.get_directory().join(&file_path)
    };
    
    let output_base = project_root.join(".c2rust").join(feature);
    
    let relative_path: PathBuf = if file_path.is_absolute() {
        // For absolute paths, try to make them relative to the project root
        file_path.strip_prefix(project_root)
            .ok()
            .map(|p| p.to_path_buf())
            .or_else(|| {
                // If not under project root, strip leading / or drive letter
                let stripped: Option<PathBuf> = file_path.strip_prefix("/").ok().map(|p: &Path| p.to_path_buf());
                
                #[cfg(windows)]
                let stripped = if stripped.is_none() {
                    // Windows: strip drive letter prefix (e.g., C:\)
                    if let Some(path_str) = file_path.to_str() {
                        // Check for Windows drive letter pattern: X:\
                        if path_str.len() > 3 
                            && path_str.chars().nth(1) == Some(':')
                            && (path_str.chars().nth(2) == Some('\\') || path_str.chars().nth(2) == Some('/'))
                        {
                            Some(PathBuf::from(&path_str[3..]))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    stripped
                };
                
                stripped
            })
            .or_else(|| {
                // If we can't strip the prefix, just use the file name
                file_path.file_name().map(PathBuf::from)
            })
            .unwrap_or_else(|| file_path.clone())
    } else {
        file_path.clone()
    };
    
    let mut output_path = output_base.join(&relative_path);
    if let Some(file_name) = output_path.file_name() {
        let new_file_name = format!("{}.c2rust", file_name.to_string_lossy());
        output_path.set_file_name(new_file_name);
    }
    
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    run_preprocessor(entry, &full_file_path, &output_path)?;
    
    Ok(PreprocessedFile {
        original_path: full_file_path,
        preprocessed_path: output_path,
    })
}

/// Run the preprocessor on a file using clang
fn run_preprocessor(
    entry: &CompileEntry,
    input_file: &Path,
    output_file: &Path,
) -> Result<()> {
    let args = entry.get_arguments();
    
    let mut preprocess_args = vec!["-E".to_string()];
    let mut skip_next = false;
    
    for arg in args.iter().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }
        
        if arg == "-c" {
            continue;
        }
        if arg == "-o" {
            skip_next = true;
            continue;
        }
        
        if arg.starts_with("-I") || 
           arg.starts_with("-D") || 
           arg.starts_with("-U") ||
           arg.starts_with("-std") ||
           arg.starts_with("-include") ||
           arg == "-I" || arg == "-D" || arg == "-U" || arg == "-include" {
            preprocess_args.push(arg.clone());
            if (arg == "-I" || arg == "-D" || arg == "-U" || arg == "-include") && skip_next == false {
                skip_next = true;
            }
        }
    }
    
    preprocess_args.push(input_file.display().to_string());
    preprocess_args.push("-o".to_string());
    preprocess_args.push(output_file.display().to_string());
    
    let clang_path = get_clang_path();
    
    let output = Command::new(&clang_path)
        .args(&preprocess_args)
        .current_dir(&entry.get_directory())
        .output()
        .map_err(|e| {
            Error::CommandExecutionFailed(format!(
                "Failed to run clang preprocessor for {}: {}",
                input_file.display(),
                e
            ))
        })?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::CommandExecutionFailed(format!(
            "Clang preprocessor failed for {}:\n{}",
            input_file.display(),
            stderr
        )));
    }
    
    Ok(())
}
