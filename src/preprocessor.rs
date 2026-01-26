use crate::error::{Error, Result};
use crate::tracker::CompileEntry;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Represents a preprocessed file with its metadata
#[derive(Debug, Clone)]
pub struct PreprocessedFile {
    pub original_path: PathBuf,
    pub preprocessed_path: PathBuf,
    pub module_name: String,
}

/// Preprocess C files using compiler's -E flag
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
    // Get the file path
    let file_path = entry.get_file_path();
    let full_file_path = if file_path.is_absolute() {
        file_path.clone()
    } else {
        entry.get_directory().join(&file_path)
    };
    
    // Determine the output path: .c2rust/<feature>/c/<original_structure>
    let output_base = project_root.join(".c2rust").join(feature).join("c");
    
    // Preserve the original directory structure
    let relative_path = if file_path.is_absolute() {
        // For absolute paths, try to make them relative to the project root
        // or just use the file name hierarchy starting from the last known parent
        let stripped = file_path.strip_prefix("/").ok();
        
        #[cfg(windows)]
        let stripped = stripped.or_else(|| {
            // Windows: try to strip drive letter prefix like C:\
            if let Some(path_str) = file_path.to_str() {
                if path_str.len() > 2 && path_str.chars().nth(1) == Some(':') {
                    return Some(PathBuf::from(&path_str[3..]));
                }
            }
            None
        });
        
        stripped
            .map(|p| p.to_path_buf())
            .or_else(|| {
                // If we can't strip the prefix, just use the file name
                file_path.file_name().map(PathBuf::from)
            })
            .unwrap_or_else(|| file_path.clone())
    } else {
        file_path.clone()
    };
    
    let output_path = output_base.join(&relative_path);
    
    // Create output directory
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    // Extract module name from path
    let module_name = extract_module_name(&relative_path);
    
    // Run preprocessor
    run_preprocessor(entry, &full_file_path, &output_path)?;
    
    Ok(PreprocessedFile {
        original_path: full_file_path,
        preprocessed_path: output_path,
        module_name,
    })
}

/// Extract module name from file path
fn extract_module_name(path: &Path) -> String {
    // Get the first component of the path as module name
    // e.g., src/module/file.c -> "src"
    // or just use the directory name before the filename
    if let Some(parent) = path.parent() {
        if parent.components().count() > 0 {
            parent
                .components()
                .next()
                .and_then(|c| c.as_os_str().to_str())
                .unwrap_or("default")
                .to_string()
        } else {
            "default".to_string()
        }
    } else {
        "default".to_string()
    }
}

/// Run the preprocessor on a file
fn run_preprocessor(
    entry: &CompileEntry,
    input_file: &Path,
    output_file: &Path,
) -> Result<()> {
    let args = entry.get_arguments();
    
    if args.is_empty() {
        return Err(Error::CommandExecutionFailed(
            "No compiler arguments found".to_string(),
        ));
    }
    
    // Build preprocessor command
    let compiler = &args[0];
    
    // Filter out incompatible flags and add -E
    let mut preprocess_args = Vec::new();
    preprocess_args.push("-E".to_string());
    
    let mut skip_next = false;
    for (_i, arg) in args.iter().enumerate().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }
        
        // Skip output-related flags
        if arg == "-o" || arg == "-c" {
            skip_next = true;
            continue;
        }
        
        // Skip the input file itself (we'll add it explicitly)
        if arg.ends_with(".c") {
            continue;
        }
        
        // Keep include paths, defines, and other preprocessor flags
        if arg.starts_with("-I") || arg.starts_with("-D") || arg.starts_with("-U") {
            preprocess_args.push(arg.clone());
        }
    }
    
    // Add input file
    preprocess_args.push(input_file.display().to_string());
    
    // Add output file
    preprocess_args.push("-o".to_string());
    preprocess_args.push(output_file.display().to_string());
    
    // Execute preprocessor
    let output = Command::new(compiler)
        .args(&preprocess_args)
        .current_dir(&entry.get_directory())
        .output()
        .map_err(|e| {
            Error::CommandExecutionFailed(format!(
                "Failed to run preprocessor for {}: {}",
                input_file.display(),
                e
            ))
        })?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::CommandExecutionFailed(format!(
            "Preprocessor failed for {}:\n{}",
            input_file.display(),
            stderr
        )));
    }
    
    Ok(())
}

/// Group preprocessed files by module
pub fn group_by_module(files: &[PreprocessedFile]) -> HashMap<String, Vec<PreprocessedFile>> {
    let mut groups: HashMap<String, Vec<PreprocessedFile>> = HashMap::new();
    
    for file in files {
        groups
            .entry(file.module_name.clone())
            .or_insert_with(Vec::new)
            .push(file.clone());
    }
    
    groups
}

/// Delete preprocessed files for a module
pub fn delete_module_files(files: &[PreprocessedFile]) -> Result<()> {
    for file in files {
        if file.preprocessed_path.exists() {
            fs::remove_file(&file.preprocessed_path)?;
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_module_name() {
        let path = PathBuf::from("src/module/file.c");
        assert_eq!(extract_module_name(&path), "src");
    }

    #[test]
    fn test_extract_module_name_simple() {
        let path = PathBuf::from("file.c");
        assert_eq!(extract_module_name(&path), "default");
    }
}
