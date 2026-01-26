use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command};

/// Represents a compilation database entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileEntry {
    pub directory: String,
    pub file: String,
    pub arguments: Option<Vec<String>>,
    pub command: Option<String>,
}

impl CompileEntry {
    /// Get the compiler arguments as a vector
    pub fn get_arguments(&self) -> Vec<String> {
        if let Some(ref args) = self.arguments {
            args.clone()
        } else if let Some(ref cmd) = self.command {
            // Simple parsing - split by whitespace
            shell_words::split(cmd).unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    /// Extract the compiler name from arguments
    pub fn get_compiler(&self) -> String {
        let args = self.get_arguments();
        if !args.is_empty() {
            // Get the base name of the compiler
            PathBuf::from(&args[0])
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string()
        } else {
            "unknown".to_string()
        }
    }

    /// Check if this is a gcc or clang compiler
    pub fn is_c_compiler(&self) -> bool {
        let compiler = self.get_compiler();
        compiler.contains("gcc") || compiler.contains("clang") || compiler.contains("cc")
    }

    /// Get the C file path as PathBuf
    pub fn get_file_path(&self) -> PathBuf {
        PathBuf::from(&self.file)
    }

    /// Get the directory as PathBuf
    pub fn get_directory(&self) -> PathBuf {
        PathBuf::from(&self.directory)
    }
}

/// Track build process by creating a compilation database
pub fn track_build(build_dir: &Path, command: &[String]) -> Result<Vec<CompileEntry>> {
    // Track compilation using custom compiler wrappers
    track_with_wrapper(build_dir, command)?;
    
    // Parse the compilation database
    let compile_db_path = build_dir.join("compile_commands.json");
    parse_compile_commands(&compile_db_path)
}

fn track_with_wrapper(
    build_dir: &Path,
    command: &[String],
) -> Result<()> {
    // Create a wrapper script that logs compiler invocations
    // Use timestamp and random suffix to avoid PID collisions
    let temp_dir = std::env::temp_dir().join(format!(
        "c2rust-build-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    ));
    fs::create_dir_all(&temp_dir)?;
    
    let log_file = temp_dir.join("compile_commands.log");
    
    // Create wrapper scripts for gcc and clang
    create_compiler_wrapper(&temp_dir, "gcc", &log_file)?;
    create_compiler_wrapper(&temp_dir, "clang", &log_file)?;
    create_compiler_wrapper(&temp_dir, "cc", &log_file)?;
    
    // Execute build with wrappers in PATH
    let program = &command[0];
    let args = &command[1..];
    
    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", temp_dir.display(), original_path);
    
    let output = Command::new(program)
        .args(args)
        .current_dir(build_dir)
        .env("PATH", &new_path)
        .output()
        .map_err(|e| {
            Error::CommandExecutionFailed(format!("Failed to execute build command: {}", e))
        })?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let _ = fs::remove_dir_all(&temp_dir);
        return Err(Error::CommandExecutionFailed(format!(
            "Build command failed with exit code {}\nstdout: {}\nstderr: {}",
            output.status.code().unwrap_or(-1),
            stdout,
            stderr
        )));
    }
    
    // Convert log to compile_commands.json
    convert_log_to_json(&log_file, &build_dir.join("compile_commands.json"))?;
    
    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
    
    Ok(())
}

fn create_compiler_wrapper(temp_dir: &Path, compiler: &str, log_file: &Path) -> Result<()> {
    // Find the real compiler
    let real_compiler = find_real_compiler(compiler, temp_dir);
    
    let wrapper_path = temp_dir.join(compiler);
    let log_path = log_file.display().to_string();
    
    let wrapper_content = format!(
        r#"#!/bin/bash
# Log this compilation
echo "DIR:$(pwd)" >> "{}"
echo "CMD:{} $@" >> "{}"
echo "---" >> "{}"
# Execute the real compiler
exec {} "$@"
"#,
        log_path, real_compiler, log_path, log_path, real_compiler
    );
    
    fs::write(&wrapper_path, wrapper_content)?;
    
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&wrapper_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&wrapper_path, perms)?;
    }
    
    Ok(())
}

fn find_real_compiler(compiler: &str, exclude_dir: &Path) -> String {
    // PATH separator is : on Unix and ; on Windows
    let path_sep = if cfg!(windows) { ';' } else { ':' };
    
    // Find the real compiler in PATH, excluding our wrapper directory
    if let Ok(path_var) = std::env::var("PATH") {
        let exclude_str = exclude_dir.to_string_lossy();
        for path in path_var.split(path_sep) {
            if path == exclude_str {
                continue;
            }
            let candidate = PathBuf::from(path).join(compiler);
            if candidate.exists() && candidate.is_file() {
                return candidate.display().to_string();
            }
        }
    }
    // Fallback: try common locations
    let common_paths = if cfg!(windows) {
        vec!["C:\\msys64\\usr\\bin", "C:\\MinGW\\bin"]
    } else {
        vec!["/usr/bin", "/usr/local/bin", "/opt/homebrew/bin"]
    };
    
    for base in common_paths {
        let candidate = PathBuf::from(base).join(compiler);
        if candidate.exists() && candidate.is_file() {
            return candidate.display().to_string();
        }
    }
    
    // Last resort: just use the compiler name and hope it's in PATH
    compiler.to_string()
}

fn convert_log_to_json(log_file: &Path, output_file: &Path) -> Result<()> {
    if !log_file.exists() {
        // No compilations tracked, create empty database
        fs::write(output_file, "[]")?;
        return Ok(());
    }
    
    let file = fs::File::open(log_file)?;
    let reader = BufReader::new(file);
    
    let mut entries = Vec::new();
    let mut current_dir = String::new();
    let mut current_cmd = String::new();
    
    for line in reader.lines() {
        let line = line?;
        if line.starts_with("DIR:") {
            current_dir = line[4..].to_string();
        } else if line.starts_with("CMD:") {
            current_cmd = line[4..].to_string();
        } else if line == "---" && !current_dir.is_empty() && !current_cmd.is_empty() {
            // Extract C file from command
            if let Some(c_file) = extract_c_file_from_command(&current_cmd) {
                entries.push(CompileEntry {
                    directory: current_dir.clone(),
                    file: c_file,
                    arguments: None,
                    command: Some(current_cmd.clone()),
                });
            }
            current_dir.clear();
            current_cmd.clear();
        }
    }
    
    let json = serde_json::to_string_pretty(&entries)?;
    fs::write(output_file, json)?;
    
    Ok(())
}

fn extract_c_file_from_command(command: &str) -> Option<String> {
    // Simple extraction of .c file from command string
    for part in command.split_whitespace() {
        if part.ends_with(".c") && !part.starts_with('-') {
            return Some(part.to_string());
        }
    }
    None
}

fn parse_compile_commands(path: &Path) -> Result<Vec<CompileEntry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    
    let content = fs::read_to_string(path)?;
    let entries: Vec<CompileEntry> = serde_json::from_str(&content)
        .map_err(|e| Error::IoError(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Failed to parse compile_commands.json: {}", e)
        )))?;
    
    // Filter to only C files compiled with gcc/clang
    Ok(entries
        .into_iter()
        .filter(|e| e.is_c_compiler() && e.file.ends_with(".c"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_c_file_from_command() {
        let cmd = "gcc -c test.c -o test.o";
        assert_eq!(
            extract_c_file_from_command(cmd),
            Some("test.c".to_string())
        );
    }

    #[test]
    fn test_extract_c_file_from_command_none() {
        let cmd = "gcc -c test.cpp -o test.o";
        assert_eq!(extract_c_file_from_command(cmd), None);
    }
}
