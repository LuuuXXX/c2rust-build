use crate::error::{Error, Result};
use std::collections::HashMap;
use std::process::Command;

/// Grouped compilation result: (options, files)
pub type CompilationGroup = (String, Vec<String>);

/// Execute a command in the specified directory
/// 
/// This function executes the command with inherited stdout/stderr,
/// allowing users to see build progress in real-time. Note that on failure,
/// the error message will not include command output since it streams directly
/// to the terminal.
#[allow(dead_code)]
pub fn execute_command(dir: &str, command: &[String]) -> Result<()> {
    if command.is_empty() {
        return Err(Error::CommandExecutionFailed(
            "No command provided".to_string(),
        ));
    }

    let program = &command[0];
    let args = &command[1..];

    // Use spawn with inherited stdio for real-time output
    let status = Command::new(program)
        .args(args)
        .current_dir(dir)
        .status()
        .map_err(|e| {
            Error::CommandExecutionFailed(format!(
                "Failed to execute command '{}': {}",
                command.join(" "),
                e
            ))
        })?;

    if !status.success() {
        return Err(Error::CommandExecutionFailed(format!(
            "Command '{}' failed with exit code {}",
            command.join(" "),
            status.code().unwrap_or(-1)
        )));
    }

    Ok(())
}

/// Parse a single compiler command line and extract options and source files
fn parse_compiler_line(line: &str, compilers: &[String]) -> Option<CompilationGroup> {
    // Parse the command line
    let parts = match shell_words::split(line) {
        Ok(parts) => parts,
        Err(_) => return None,
    };

    if parts.is_empty() {
        return None;
    }

    // Extract the compiler name from the first part (may be absolute path)
    let compiler_name = std::path::Path::new(&parts[0])
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&parts[0]);

    // Check if this is a compiler command
    if !compilers.iter().any(|c| c == compiler_name) {
        return None;
    }

    let mut options = Vec::new();
    let mut files = Vec::new();
    let mut skip_next = false;

    for (_i, arg) in parts.iter().enumerate().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }

        // Skip -o and its argument
        if arg == "-o" {
            skip_next = true;
            continue;
        }

        // Collect options (starting with -)
        if arg.starts_with('-') {
            options.push(arg.clone());
        } else {
            // Check if this is a source file
            let ext = std::path::Path::new(arg)
                .extension()
                .and_then(|e| e.to_str());
            if let Some(extension) = ext {
                if ["c", "cpp", "cc", "cxx", "C"].contains(&extension) {
                    files.push(arg.clone());
                }
            }
        }
    }

    if files.is_empty() {
        return None;
    }

    // Sort options for consistent grouping
    options.sort();
    let options_str = options.join(" ");

    Some((options_str, files))
}

/// Parse build output and group compilation units
fn parse_and_group(output: &str, compilers: &[String]) -> Vec<CompilationGroup> {
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();

    for line in output.lines() {
        if let Some((options, files)) = parse_compiler_line(line, compilers) {
            groups
                .entry(options)
                .or_default()
                .extend(files);
        }
    }

    // Convert to sorted vector and deduplicate files
    let mut result: Vec<CompilationGroup> = groups
        .into_iter()
        .map(|(options, mut files)| {
            files.sort();
            files.dedup();
            (options, files)
        })
        .collect();

    // Sort by options for consistent output
    result.sort_by(|a, b| a.0.cmp(&b.0));

    result
}

/// Execute build command and parse compilation units
/// Returns grouped compilation results: Vec<(options, files)>
pub fn execute_and_parse(
    dir: &str,
    command: &[String],
    compilers: &[String],
) -> Result<Vec<CompilationGroup>> {
    if command.is_empty() {
        return Err(Error::CommandExecutionFailed(
            "No command provided".to_string(),
        ));
    }

    let program = &command[0];
    let args = &command[1..];

    // Execute and capture output
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| {
            Error::CommandExecutionFailed(format!(
                "Failed to execute command '{}': {}",
                command.join(" "),
                e
            ))
        })?;

    if !output.status.success() {
        return Err(Error::CommandExecutionFailed(format!(
            "Command '{}' failed with exit code {}",
            command.join(" "),
            output.status.code().unwrap_or(-1)
        )));
    }

    // Combine stdout and stderr for parsing
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined_output = format!("{}\n{}", stdout, stderr);

    // Print output for user visibility
    print!("{}", stdout);
    eprint!("{}", stderr);

    // Parse and group compilation commands
    let groups = parse_and_group(&combined_output, compilers);

    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_command_empty() {
        let result = execute_command(".", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_command_basic() {
        // Test with a simple command that should succeed
        let result = execute_command(".", &["echo".to_string(), "test".to_string()]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_compiler_line() {
        let line = "gcc -I../include -DDEBUG=1 -o main.o main.c utils.c";
        let compilers = vec!["gcc".to_string()];
        let result = parse_compiler_line(line, &compilers).unwrap();

        assert!(result.0.contains("-I../include"));
        assert!(result.0.contains("-DDEBUG=1"));
        assert_eq!(result.1, vec!["main.c", "utils.c"]);
    }

    #[test]
    fn test_parse_compiler_line_absolute_path() {
        let line = "/usr/bin/gcc -I../include -o main.o main.c";
        let compilers = vec!["gcc".to_string()];
        let result = parse_compiler_line(line, &compilers).unwrap();

        assert!(result.0.contains("-I../include"));
        assert_eq!(result.1, vec!["main.c"]);
    }

    #[test]
    fn test_parse_compiler_line_not_compiler() {
        let line = "echo building project";
        let compilers = vec!["gcc".to_string()];
        let result = parse_compiler_line(line, &compilers);

        assert!(result.is_none());
    }

    #[test]
    fn test_parse_compiler_line_no_sources() {
        let line = "gcc -I../include -o main.o";
        let compilers = vec!["gcc".to_string()];
        let result = parse_compiler_line(line, &compilers);

        assert!(result.is_none());
    }

    #[test]
    fn test_parse_and_group() {
        let output = r#"
            gcc -I../include -DDEBUG=1 -o debug.o main.c debug.c
            gcc -I../include -DDEBUG=1 -o common.o common.c
            gcc -I../include -o release.o release.c
        "#;

        let compilers = vec!["gcc".to_string()];
        let groups = parse_and_group(output, &compilers);

        assert_eq!(groups.len(), 2);
        
        // Find the group with DEBUG flag
        let debug_group = groups.iter().find(|(opts, _)| opts.contains("-DDEBUG=1")).unwrap();
        assert_eq!(debug_group.1.len(), 3);
        assert!(debug_group.1.contains(&"main.c".to_string()));
        assert!(debug_group.1.contains(&"debug.c".to_string()));
        assert!(debug_group.1.contains(&"common.c".to_string()));

        // Find the group without DEBUG flag
        let release_group = groups.iter().find(|(opts, _)| !opts.contains("-DDEBUG=1")).unwrap();
        assert_eq!(release_group.1.len(), 1);
        assert!(release_group.1.contains(&"release.c".to_string()));
    }

    #[test]
    fn test_parse_and_group_dedup() {
        let output = r#"
            gcc -I../include -o a.o main.c
            gcc -I../include -o b.o main.c
        "#;

        let compilers = vec!["gcc".to_string()];
        let groups = parse_and_group(output, &compilers);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].1.len(), 1);
        assert_eq!(groups[0].1[0], "main.c");
    }
}
