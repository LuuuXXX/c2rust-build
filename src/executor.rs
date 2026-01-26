use crate::error::{Error, Result};
use std::process::Command;

/// Execute a command in the specified directory
pub fn execute_command(dir: &str, command: &[String]) -> Result<()> {
    if command.is_empty() {
        return Err(Error::CommandExecutionFailed(
            "No command provided".to_string(),
        ));
    }

    let program = &command[0];
    let args = &command[1..];

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
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(Error::CommandExecutionFailed(format!(
            "Command '{}' failed with exit code {}\nstdout: {}\nstderr: {}",
            command.join(" "),
            output.status.code().unwrap_or(-1),
            stdout,
            stderr
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
            
            // Verify the error message contains stdout and stderr labels
            assert!(msg.contains("stdout:"), "Error should contain stdout label: {}", msg);
            assert!(msg.contains("stderr:"), "Error should contain stderr label: {}", msg);
            
            // Verify the command name is in the error
            assert!(msg.contains("false"), "Error should contain command name: {}", msg);
        } else {
            panic!("Expected CommandExecutionFailed error");
        }
    }
}
