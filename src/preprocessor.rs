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
        file_path
            .strip_prefix(project_root)
            .ok()
            .map(|p| p.to_path_buf())
            .or_else(|| {
                // If not under project root, strip leading / or drive letter
                let stripped: Option<PathBuf> = file_path
                    .strip_prefix("/")
                    .ok()
                    .map(|p: &Path| p.to_path_buf());

                #[cfg(windows)]
                let stripped = if stripped.is_none() {
                    // Windows: strip drive letter prefix (e.g., C:\)
                    if let Some(path_str) = file_path.to_str() {
                        // Check for Windows drive letter pattern: X:\
                        if path_str.len() > 3
                            && path_str.chars().nth(1) == Some(':')
                            && (path_str.chars().nth(2) == Some('\\')
                                || path_str.chars().nth(2) == Some('/'))
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
    // Replace the .c extension with .c2rust
    output_path = output_path.with_extension("c2rust");

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    run_preprocessor(entry, &full_file_path, &output_path)?;

    Ok(PreprocessedFile {
        original_path: full_file_path,
        preprocessed_path: output_path,
    })
}

/// Build preprocessor arguments from compiler arguments
///
/// This function extracts the relevant compiler flags needed for preprocessing
/// and handles both combined-form (e.g., `-Iinclude/`) and split-form (e.g., `-I include/`)
/// arguments correctly.
fn build_preprocess_args(
    compiler_args: &[String],
    input_file: &Path,
    output_file: &Path,
) -> Vec<String> {
    let mut preprocess_args = vec!["-E".to_string()];
    let mut args_iter = compiler_args.iter().skip(1);

    while let Some(arg) = args_iter.next() {
        if arg == "-c" {
            continue;
        }
        if arg == "-o" {
            // Skip the output file argument
            args_iter.next();
            continue;
        }

        // Check for split-form flags first (exact match)
        if arg == "-I" || arg == "-D" || arg == "-U" || arg == "-include" {
            // Split form (e.g., -I include/)
            preprocess_args.push(arg.clone());
            // Also consume and push the next argument (the value)
            if let Some(value) = args_iter.next() {
                preprocess_args.push(value.clone());
            }
        } else if arg.starts_with("-I")
            || arg.starts_with("-D")
            || arg.starts_with("-U")
            || arg.starts_with("-std")
            || arg.starts_with("-include")
        {
            // Combined form (e.g., -Iinclude/)
            preprocess_args.push(arg.clone());
        }
    }

    preprocess_args.push(input_file.display().to_string());
    preprocess_args.push("-o".to_string());
    preprocess_args.push(output_file.display().to_string());

    preprocess_args
}

/// Run the preprocessor on a file using clang
fn run_preprocessor(entry: &CompileEntry, input_file: &Path, output_file: &Path) -> Result<()> {
    let args = entry.get_arguments();
    let preprocess_args = build_preprocess_args(&args, input_file, output_file);

    let clang_path = get_clang_path();

    let output = Command::new(&clang_path)
        .args(&preprocess_args)
        .current_dir(entry.get_directory())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_build_preprocess_args_combined_form() {
        // Test combined-form flags like -Iinclude/
        let args = vec![
            "gcc".to_string(),
            "-c".to_string(),
            "-Iinclude/".to_string(),
            "-DDEBUG".to_string(),
            "-Uold_macro".to_string(),
            "file.c".to_string(),
        ];
        let input = PathBuf::from("input.c");
        let output = PathBuf::from("output.i");

        let result = build_preprocess_args(&args, &input, &output);

        assert_eq!(result[0], "-E");
        assert!(result.contains(&"-Iinclude/".to_string()));
        assert!(result.contains(&"-DDEBUG".to_string()));
        assert!(result.contains(&"-Uold_macro".to_string()));
        assert!(!result.contains(&"-c".to_string()));
        assert_eq!(result[result.len() - 3], "input.c");
        assert_eq!(result[result.len() - 2], "-o");
        assert_eq!(result[result.len() - 1], "output.i");
    }

    #[test]
    fn test_build_preprocess_args_split_form() {
        // Test split-form flags like -I include/
        let args = vec![
            "gcc".to_string(),
            "-I".to_string(),
            "include/".to_string(),
            "-D".to_string(),
            "DEBUG".to_string(),
            "-U".to_string(),
            "old_macro".to_string(),
            "file.c".to_string(),
        ];
        let input = PathBuf::from("input.c");
        let output = PathBuf::from("output.i");

        let result = build_preprocess_args(&args, &input, &output);

        assert_eq!(result[0], "-E");
        // Check that split-form flags include both the flag and value
        let i_index = result.iter().position(|x| x == "-I").unwrap();
        assert_eq!(result[i_index + 1], "include/");

        let d_index = result.iter().position(|x| x == "-D").unwrap();
        assert_eq!(result[d_index + 1], "DEBUG");

        let u_index = result.iter().position(|x| x == "-U").unwrap();
        assert_eq!(result[u_index + 1], "old_macro");

        assert_eq!(result[result.len() - 3], "input.c");
        assert_eq!(result[result.len() - 2], "-o");
        assert_eq!(result[result.len() - 1], "output.i");
    }

    #[test]
    fn test_build_preprocess_args_include_flag() {
        // Test -include flag in both forms
        let args_combined = vec![
            "gcc".to_string(),
            "-includeheader.h".to_string(),
            "file.c".to_string(),
        ];
        let args_split = vec![
            "gcc".to_string(),
            "-include".to_string(),
            "header.h".to_string(),
            "file.c".to_string(),
        ];
        let input = PathBuf::from("input.c");
        let output = PathBuf::from("output.i");

        let result_combined = build_preprocess_args(&args_combined, &input, &output);
        assert!(result_combined.contains(&"-includeheader.h".to_string()));

        let result_split = build_preprocess_args(&args_split, &input, &output);
        let include_index = result_split.iter().position(|x| x == "-include").unwrap();
        assert_eq!(result_split[include_index + 1], "header.h");
    }

    #[test]
    fn test_build_preprocess_args_output_flag_skipped() {
        // Test that -o and its value are skipped
        let args = vec![
            "gcc".to_string(),
            "-c".to_string(),
            "-o".to_string(),
            "original_output.o".to_string(),
            "-Iinclude/".to_string(),
            "file.c".to_string(),
        ];
        let input = PathBuf::from("input.c");
        let output = PathBuf::from("output.i");

        let result = build_preprocess_args(&args, &input, &output);

        // Should not contain the original -o or its value (except our new -o at the end)
        assert!(!result.contains(&"original_output.o".to_string()));
        assert!(result.contains(&"-Iinclude/".to_string()));
        assert_eq!(result[result.len() - 2], "-o");
        assert_eq!(result[result.len() - 1], "output.i");
    }

    #[test]
    fn test_build_preprocess_args_std_flag() {
        // Test that -std flags are preserved
        let args = vec![
            "gcc".to_string(),
            "-std=c11".to_string(),
            "file.c".to_string(),
        ];
        let input = PathBuf::from("input.c");
        let output = PathBuf::from("output.i");

        let result = build_preprocess_args(&args, &input, &output);

        assert!(result.contains(&"-std=c11".to_string()));
    }

    #[test]
    fn test_build_preprocess_args_mixed_forms() {
        // Test a mix of combined and split forms
        let args = vec![
            "gcc".to_string(),
            "-Iinclude/".to_string(), // combined
            "-D".to_string(),         // split
            "DEBUG".to_string(),
            "-Uold".to_string(),    // combined
            "-include".to_string(), // split
            "header.h".to_string(),
            "file.c".to_string(),
        ];
        let input = PathBuf::from("input.c");
        let output = PathBuf::from("output.i");

        let result = build_preprocess_args(&args, &input, &output);

        assert!(result.contains(&"-Iinclude/".to_string()));
        assert!(result.contains(&"-Uold".to_string()));

        let d_index = result.iter().position(|x| x == "-D").unwrap();
        assert_eq!(result[d_index + 1], "DEBUG");

        let include_index = result.iter().position(|x| x == "-include").unwrap();
        assert_eq!(result[include_index + 1], "header.h");
    }
}
