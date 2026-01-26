use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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
            // Parse command string properly
            match shell_words::split(cmd) {
                Ok(args) => args,
                Err(e) => {
                    eprintln!(
                        "Warning: failed to parse command string '{}': {}",
                        cmd, e
                    );
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        }
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
pub fn track_build(build_dir: &Path, command: &[String], project_root: &Path) -> Result<Vec<CompileEntry>> {
    // Track compilation using custom compiler wrappers
    track_with_wrapper(build_dir, command, project_root)?;
    
    // Parse the compilation database from .c2rust directory
    let compile_db_path = project_root.join(".c2rust").join("compile_commands.json");
    parse_compile_commands(&compile_db_path)
}

/// Find the real compiler in the system PATH
fn find_real_compiler(compiler_name: &str, path: &std::ffi::OsStr) -> Result<PathBuf> {
    let paths: Vec<PathBuf> = std::env::split_paths(path).collect();
    
    for dir in paths {
        #[cfg(unix)]
        {
            let candidate = dir.join(compiler_name);
            
            // Check if file exists and is executable
            if candidate.exists() {
                if let Ok(metadata) = fs::metadata(&candidate) {
                    use std::os::unix::fs::PermissionsExt;
                    if metadata.permissions().mode() & 0o111 != 0 {
                        return Ok(candidate);
                    }
                }
                // File exists but not executable or can't read metadata - skip it
            }
        }
        
        #[cfg(not(unix))]
        {
            // On Windows, check for both with and without .exe extension
            let candidate_with_exe = dir.join(format!("{}.exe", compiler_name));
            let candidate_without_exe = dir.join(compiler_name);
            
            // Prefer .exe version if it exists
            if candidate_with_exe.exists() {
                return Ok(candidate_with_exe);
            }
            
            // Fall back to without .exe
            if candidate_without_exe.exists() {
                return Ok(candidate_without_exe);
            }
        }
    }
    
    Err(Error::CommandExecutionFailed(format!(
        "Could not find '{}' in PATH. Make sure the compiler is installed.",
        compiler_name
    )))
}


fn track_with_wrapper(
    build_dir: &Path,
    command: &[String],
    project_root: &Path,
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
    
    // Find real compiler paths BEFORE modifying PATH
    let original_path = std::env::var_os("PATH").unwrap_or_default();
    
    // Try to find at least one compiler (gcc, clang, or cc)
    // gcc is most common, so try it first
    let gcc_path = find_real_compiler("gcc", &original_path);
    let clang_path = find_real_compiler("clang", &original_path);
    let cc_path = find_real_compiler("cc", &original_path);
    
    // Ensure at least one compiler is found
    if gcc_path.is_err() && clang_path.is_err() && cc_path.is_err() {
        return Err(Error::CommandExecutionFailed(
            "Could not find any C compiler (gcc, clang, or cc) in PATH. Please install a C compiler.".to_string()
        ));
    }
    
    // Create wrapper scripts for found compilers
    if let Ok(path) = gcc_path {
        create_compiler_wrapper(&temp_dir, "gcc", &log_file, &path)?;
    }
    if let Ok(path) = clang_path {
        create_compiler_wrapper(&temp_dir, "clang", &log_file, &path)?;
    }
    if let Ok(path) = cc_path {
        create_compiler_wrapper(&temp_dir, "cc", &log_file, &path)?;
    }
    
    // Execute build with wrappers in PATH
    let program = &command[0];
    let args = &command[1..];
    
    // Use platform-appropriate PATH manipulation
    let original_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths: Vec<PathBuf> = std::env::split_paths(&original_path).collect();
    paths.insert(0, temp_dir.clone());
    let new_path = std::env::join_paths(paths).map_err(|e| {
        Error::CommandExecutionFailed(format!("Failed to construct PATH: {}", e))
    })?;
    
    // Display command execution details
    println!("Executing command: {} {}", program, args.join(" "));
    println!("In directory: {}", build_dir.display());
    println!();
    
    // Spawn the command with inherited stdout/stderr for real-time output
    let mut child = Command::new(program)
        .args(args)
        .current_dir(build_dir)
        .env("PATH", &new_path)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| {
            Error::CommandExecutionFailed(format!("Failed to execute build command: {}", e))
        })?;
    
    let status = child.wait()
        .map_err(|e| {
            Error::CommandExecutionFailed(format!("Failed to wait for build command: {}", e))
        })?;
    
    println!();
    if let Some(code) = status.code() {
        println!("Exit code: {}", code);
    }
    
    if !status.success() {
        if let Err(e) = fs::remove_dir_all(&temp_dir) {
            eprintln!("Warning: failed to cleanup temporary directory: {}", e);
        }
        return Err(Error::CommandExecutionFailed(format!(
            "Build command failed with exit code {}",
            status.code().unwrap_or(-1)
        )));
    }
    
    // Ensure .c2rust directory exists
    let c2rust_dir = project_root.join(".c2rust");
    fs::create_dir_all(&c2rust_dir)?;
    
    // Convert log to compile_commands.json in .c2rust directory
    convert_log_to_json(&log_file, &c2rust_dir.join("compile_commands.json"))?;
    
    // Cleanup
    if let Err(e) = fs::remove_dir_all(&temp_dir) {
        eprintln!("Warning: failed to cleanup temporary directory: {}", e);
    }
    
    Ok(())
}

fn create_compiler_wrapper(temp_dir: &Path, compiler: &str, log_file: &Path, real_compiler: &Path) -> Result<()> {
    let wrapper_path = temp_dir.join(compiler);
    let log_path = log_file.display().to_string();
    let real_compiler_str = real_compiler.display().to_string();
    
    let wrapper_content = format!(
        r#"#!/bin/bash
# Log this compilation with file locking for parallel builds
{{
  flock 200
  echo "DIR:$(pwd)" >&200
  echo "CMD:{} $@" >&200
  echo "---" >&200
}} 200>>"{}"
# Execute the real compiler using absolute path
exec "{}" "$@"
"#,
        compiler, log_path, real_compiler_str
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
    // Use shell_words to properly parse the command string
    let args = shell_words::split(command).ok()?;
    
    // Look for .c files in the parsed arguments
    for arg in args {
        // Skip arguments that are flags (start with -)
        if arg.starts_with('-') {
            continue;
        }
        // Check if it's a C file
        if arg.ends_with(".c") {
            return Some(arg);
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
    
    // Filter to only C files (wrappers only track gcc/clang/cc)
    Ok(entries
        .into_iter()
        .filter(|e| e.file.ends_with(".c"))
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
