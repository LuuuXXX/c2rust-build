use crate::error::{Error, Result};
use std::process::{Command, Stdio};

/// Execute a command in the specified directory with real-time output
pub fn execute_command(dir: &str, command: &[String]) -> Result<()> {
    if command.is_empty() {
        return Err(Error::CommandExecutionFailed(
            "No command provided".to_string(),
        ));
    }

    let program = &command[0];
    let args = &command[1..];

    // Display command execution details
    println!("Executing command: {} {}", program, args.join(" "));
    println!("In directory: {}", dir);
    println!();

    // Spawn the command with inherited stdout/stderr for real-time output
    let mut child = Command::new(program)
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| {
            Error::CommandExecutionFailed(format!(
                "Failed to execute command '{}': {}",
                command.join(" "),
                e
            ))
        })?;

    let status = child.wait()
        .map_err(|e| {
            Error::CommandExecutionFailed(format!(
                "Failed to wait for command '{}': {}",
                command.join(" "),
                e
            ))
        })?;

    println!();
    if let Some(code) = status.code() {
        println!("Exit code: {}", code);
    }

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

    #[test]
    fn test_execute_command_failure() {
        // Test with a command that will fail (false command always exits with code 1)
        let result = execute_command(".", &["false".to_string()]);
        
        assert!(result.is_err());
        
        if let Err(Error::CommandExecutionFailed(msg)) = result {
            // Verify the error message contains the exit code
            assert!(msg.contains("exit code"), "Error should contain exit code: {}", msg);
            assert!(msg.contains("exit code 1"), "Error should contain exit code 1: {}", msg);
            
            // Verify the command name is in the error
            assert!(msg.contains("false"), "Error should contain command name: {}", msg);
        } else {
            panic!("Expected CommandExecutionFailed error");
        }
    }
}
