use crate::error::{Error, Result};
use crate::tracker::CompileEntry;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Represents a preprocessed file with its metadata
#[derive(Debug, Clone)]
pub struct PreprocessedFile {
    #[allow(dead_code)]
    pub original_path: PathBuf,
    #[allow(dead_code)]
    pub preprocessed_path: PathBuf,
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
    let relative_path: PathBuf = if file_path.is_absolute() {
        // For absolute paths, try to make them relative to the project root
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
    
    // Run preprocessor
    run_preprocessor(entry, &full_file_path, &output_path)?;
    
    Ok(PreprocessedFile {
        original_path: full_file_path,
        preprocessed_path: output_path,
    })
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
    
    // Build preprocessor command: filter out -c and -o arguments
    let compiler = &args[0];
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
        preprocess_args.push(arg.clone());
    }
    
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

#[cfg(test)]
mod tests {
    // Removed module-related tests as they are no longer needed
}
