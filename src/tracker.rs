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
                    eprintln!("Warning: failed to parse command string '{}': {}", cmd, e);
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
pub fn track_build(
    build_dir: &Path,
    command: &[String],
    project_root: &Path,
) -> Result<(Vec<CompileEntry>, Vec<String>)> {
    let hook_lib = get_hook_library_path()?;

    if !hook_lib.exists() {
        return Err(Error::HookLibraryNotFound);
    }

    let compilers = execute_with_hook(build_dir, command, project_root, &hook_lib)?;

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
    let c2rust_dir = project_root.join(".c2rust");
    fs::create_dir_all(&c2rust_dir)?;

    let output_file = c2rust_dir.join("compile_output.txt");

    if output_file.exists() {
        fs::remove_file(&output_file)?;
    }

    let program = &command[0];
    let args = &command[1..];

    let abs_project_root = project_root.canonicalize()?;

    println!("Executing command: {} {}", program, args.join(" "));
    println!("In directory: {}", build_dir.display());
    println!();

    let mut child = Command::new(program)
        .args(args)
        .current_dir(build_dir)
        .env("LD_PRELOAD", hook_lib)
        .env("C2RUST_ROOT", &abs_project_root)
        .env("C2RUST_OUTPUT_FILE", &output_file)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| {
            Error::CommandExecutionFailed(format!("Failed to execute build command: {}", e))
        })?;

    let status = child.wait().map_err(|e| {
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

    let (entries, compilers) = parse_hook_output(&output_file)?;

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

    for entry_str in content.split("---ENTRY---") {
        let entry_str = entry_str.trim();
        if entry_str.is_empty() {
            continue;
        }

        let lines: Vec<&str> = entry_str.lines().collect();

        let (compile_options, file_path, directory) = if lines.len() == 2 {
            ("", lines[0].trim(), lines[1].trim())
        } else if lines.len() >= 3 {
            (lines[0].trim(), lines[1].trim(), lines[2].trim())
        } else {
            continue;
        };

        if file_path.is_empty() || directory.is_empty() {
            continue;
        }

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

        compilers.insert("gcc".to_string());
    }

    Ok((entries, compilers.into_iter().collect()))
}

fn parse_compile_commands(path: &Path) -> Result<Vec<CompileEntry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)?;
    let entries: Vec<CompileEntry> = serde_json::from_str(&content).map_err(|e| {
        Error::Json(format!("Failed to parse compile_commands.json: {}", e))
    })?;

    // Filter to only C files
    Ok(entries
        .into_iter()
        .filter(|e| e.file.ends_with(".c"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_hook_output_with_flags() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "---ENTRY---").unwrap();
        writeln!(temp_file, "-I./include -DDEBUG").unwrap();
        writeln!(temp_file, "/path/to/file.c").unwrap();
        writeln!(temp_file, "/working/dir").unwrap();

        let (entries, compilers) = parse_hook_output(temp_file.path()).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].file, "/path/to/file.c");
        assert_eq!(entries[0].directory, "/working/dir");
        assert!(entries[0]
            .command
            .as_ref()
            .unwrap()
            .contains("-I./include -DDEBUG"));
        assert_eq!(compilers.len(), 1);
        assert!(compilers.contains(&"gcc".to_string()));
    }

    #[test]
    fn test_parse_hook_output_without_flags() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "---ENTRY---").unwrap();
        writeln!(temp_file, "/path/to/file.c").unwrap();
        writeln!(temp_file, "/working/dir").unwrap();

        let (entries, compilers) = parse_hook_output(temp_file.path()).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].file, "/path/to/file.c");
        assert_eq!(entries[0].directory, "/working/dir");
        assert_eq!(
            entries[0].command.as_ref().unwrap(),
            "gcc -c /path/to/file.c"
        );
        assert_eq!(compilers.len(), 1);
    }

    #[test]
    fn test_parse_hook_output_multiple_entries() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "---ENTRY---").unwrap();
        writeln!(temp_file, "-I./include").unwrap();
        writeln!(temp_file, "/path/to/file1.c").unwrap();
        writeln!(temp_file, "/working/dir1").unwrap();
        writeln!(temp_file, "---ENTRY---").unwrap();
        writeln!(temp_file, "/path/to/file2.c").unwrap();
        writeln!(temp_file, "/working/dir2").unwrap();

        let (entries, _) = parse_hook_output(temp_file.path()).unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].file, "/path/to/file1.c");
        assert_eq!(entries[1].file, "/path/to/file2.c");
    }

    #[test]
    fn test_parse_hook_output_malformed_lines() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "---ENTRY---").unwrap();
        writeln!(temp_file, "only_one_line").unwrap();
        writeln!(temp_file, "---ENTRY---").unwrap();
        writeln!(temp_file, "-I./include").unwrap();
        writeln!(temp_file, "/valid/file.c").unwrap();
        writeln!(temp_file, "/valid/dir").unwrap();

        let (entries, _) = parse_hook_output(temp_file.path()).unwrap();

        // Should skip malformed entry and parse valid one
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].file, "/valid/file.c");
    }

    #[test]
    fn test_parse_hook_output_empty_file() {
        let temp_file = NamedTempFile::new().unwrap();

        let (entries, compilers) = parse_hook_output(temp_file.path()).unwrap();

        assert_eq!(entries.len(), 0);
        assert_eq!(compilers.len(), 0);
    }

    #[test]
    fn test_parse_hook_output_nonexistent_file() {
        let (entries, compilers) = parse_hook_output(Path::new("/nonexistent/file.txt")).unwrap();

        assert_eq!(entries.len(), 0);
        assert_eq!(compilers.len(), 0);
    }
}
