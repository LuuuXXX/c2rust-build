use crate::error::{Error, Result};
use std::process::Command;

/// Execute a command in the specified directory
/// 
/// This function executes the command with inherited stdout/stderr,
/// allowing users to see build progress in real-time. Note that on failure,
/// the error message will not include command output since it streams directly
/// to the terminal.
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
}
