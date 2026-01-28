use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fs;
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

/// Get the hook library path from environment variable
fn get_hook_library_path() -> Result<PathBuf> {
    std::env::var("C2RUST_HOOK_LIB")
        .map(PathBuf::from)
        .map_err(|_| Error::HookLibraryNotFound)
}

/// Track build process by creating a compilation database
/// Returns the compile entries and a list of detected compilers
pub fn track_build(build_dir: &Path, command: &[String], project_root: &Path) -> Result<(Vec<CompileEntry>, Vec<String>)> {
    // Get hook library path
    let hook_lib = get_hook_library_path()?;
    
    // Verify hook library exists
    if !hook_lib.exists() {
        return Err(Error::HookLibraryNotFound);
    }
    
    // Execute build with LD_PRELOAD hook
    let compilers = execute_with_hook(build_dir, command, project_root, &hook_lib)?;
    
    // Parse the compilation database from .c2rust directory
    let compile_db_path = project_root.join(".c2rust").join("compile_commands.json");
    let entries = parse_compile_commands(&compile_db_path)?;
    
    Ok((entries, compilers))
}

/// Execute build command with LD_PRELOAD hook
fn execute_with_hook(
    build_dir: &Path,
    command: &[String],
    project_root: &Path,
    hook_lib: &Path,
) -> Result<Vec<String>> {
    // Ensure .c2rust directory exists
    let c2rust_dir = project_root.join(".c2rust");
    fs::create_dir_all(&c2rust_dir)?;
    
    let output_file = c2rust_dir.join("compile_output.txt");
    
    // Remove old output file if it exists
    if output_file.exists() {
        fs::remove_file(&output_file)?;
    }
    
    let program = &command[0];
    let args = &command[1..];
    
    // Get absolute path for project root
    let abs_project_root = project_root.canonicalize()
        .map_err(|e| Error::IoError(e))?;
    
    // Display command execution details
    println!("Executing command: {} {}", program, args.join(" "));
    println!("In directory: {}", build_dir.display());
    println!();
    
    // Spawn the command with LD_PRELOAD
    let mut child = Command::new(program)
        .args(args)
        .current_dir(build_dir)
        .env("LD_PRELOAD", hook_lib)
        .env("C2RUST_ROOT", abs_project_root.to_str().unwrap())
        .env("C2RUST_OUTPUT_FILE", output_file.to_str().unwrap())
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
        return Err(Error::CommandExecutionFailed(format!(
            "Build command failed with exit code {}",
            status.code().unwrap_or(-1)
        )));
    }
    
    // Parse hook output and generate compile_commands.json
    let (entries, compilers) = parse_hook_output(&output_file)?;
    
    // Write compile_commands.json
    let final_compile_db = c2rust_dir.join("compile_commands.json");
    let json = serde_json::to_string_pretty(&entries)?;
    fs::write(&final_compile_db, json)?;
    
    Ok(compilers)
}

/// Parse hook output file and extract compilation entries
fn parse_hook_output(output_file: &Path) -> Result<(Vec<CompileEntry>, Vec<String>)> {
    if !output_file.exists() {
        return Ok((Vec::new(), Vec::new()));
    }
    
    let content = fs::read_to_string(output_file)?;
    let mut entries = Vec::new();
    let mut compilers = std::collections::HashSet::new();
    
    // Parse entries separated by ---ENTRY---
    for entry_str in content.split("---ENTRY---") {
        let entry_str = entry_str.trim();
        if entry_str.is_empty() {
            continue;
        }
        
        let lines: Vec<&str> = entry_str.lines().collect();
        if lines.len() < 3 {
            continue;
        }
        
        let compile_options = lines[0].trim();
        let file_path = lines[1].trim();
        let directory = lines[2].trim();
        
        if file_path.is_empty() || directory.is_empty() {
            continue;
        }
        
        // Build the command string
        let command = if compile_options.is_empty() {
            format!("gcc -c {}", file_path)
        } else {
            format!("gcc {} -c {}", compile_options, file_path)
        };
        
        entries.push(CompileEntry {
            directory: directory.to_string(),
            file: file_path.to_string(),
            arguments: None,
            command: Some(command),
        });
        
        // Track gcc as the compiler (we'll use clang for preprocessing)
        compilers.insert("gcc".to_string());
    }
    
    Ok((entries, compilers.into_iter().collect()))
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
    
    // Filter to only C files
    Ok(entries
        .into_iter()
        .filter(|e| e.file.ends_with(".c"))
        .collect())
}
