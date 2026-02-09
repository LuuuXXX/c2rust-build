use crate::error::{Error, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Get the hook library path from environment variable
pub fn get_hook_library_path() -> Result<PathBuf> {
    std::env::var("C2RUST_HOOK_LIB")
        .map(PathBuf::from)
        .map_err(|_| Error::HookLibraryNotFound)
}

/// Verify that hook library exists and is accessible
pub fn verify_hook_library() -> Result<()> {
    let hook_lib = get_hook_library_path()?;

    if !hook_lib.exists() {
        return Err(Error::HookLibraryNotFound);
    }

    Ok(())
}

/// Track build process by executing with hook library
/// Returns a list of detected compilers
pub fn track_build(
    build_dir: &Path,
    command: &[String],
    project_root: &Path,
    feature: &str,
) -> Result<Vec<String>> {
    let hook_lib = get_hook_library_path()?;
    let compilers = execute_with_hook(build_dir, command, project_root, feature, &hook_lib)?;
    Ok(compilers)
}

/// Execute build command with LD_PRELOAD hook
fn execute_with_hook(
    build_dir: &Path,
    command: &[String],
    project_root: &Path,
    feature: &str,
    hook_lib: &Path,
) -> Result<Vec<String>> {
    let c2rust_dir = project_root.join(".c2rust");
    fs::create_dir_all(&c2rust_dir)?;

    // Create feature-specific directory for preprocessing output
    let feature_dir = c2rust_dir.join(feature);
    fs::create_dir_all(&feature_dir)?;

    // Create c directory for preprocessed files and targets.list
    let c_dir = feature_dir.join("c");
    fs::create_dir_all(&c_dir)?;

    // Clear targets.list file at the start of each build to avoid duplicates
    let targets_list = c_dir.join("targets.list");
    if targets_list.exists() {
        fs::remove_file(&targets_list)?;
    }

    let program = &command[0];
    let args = &command[1..];

    let abs_project_root = project_root.canonicalize()?;
    let abs_feature_dir = feature_dir.canonicalize()?;

    println!("Executing command: {} {}", program, args.join(" "));
    println!("In directory: {}", build_dir.display());
    println!();
    println!("With environment variables:");
    println!("  LD_PRELOAD={}", hook_lib.display());
    println!("  C2RUST_PROJECT_ROOT={}", abs_project_root.display());
    println!("  C2RUST_FEATURE_ROOT={}", abs_feature_dir.display());
    println!();
    println!("Full command:");
    println!(
        "  LD_PRELOAD={} C2RUST_PROJECT_ROOT={} C2RUST_FEATURE_ROOT={} {} {}",
        hook_lib.display(),
        abs_project_root.display(),
        abs_feature_dir.display(),
        program,
        args.join(" ")
    );
    println!();

    let mut child = Command::new(program)
        .args(args)
        .current_dir(build_dir)
        .env("LD_PRELOAD", hook_lib)
        .env("C2RUST_PROJECT_ROOT", &abs_project_root)
        .env("C2RUST_FEATURE_ROOT", &abs_feature_dir)
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

    // Note: Compiler detection has been removed in this version.
    // The build command typically invokes build tools (make, cmake, ninja)
    // rather than compilers directly, making detection from the command unreliable.
    // The original implementation parsed hook output to capture actual compiler
    // invocations, but that required maintaining compile_output.txt which this
    // PR removes. If compiler detection is needed in the future, consider
    // implementing it in the hook library to write compiler info directly.
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial_test::serial]
    fn test_get_hook_library_path_not_set() {
        // Clear the environment variable
        std::env::remove_var("C2RUST_HOOK_LIB");

        let result = get_hook_library_path();
        assert!(result.is_err());
    }

    #[test]
    #[serial_test::serial]
    fn test_get_hook_library_path_set() {
        let test_path = "/tmp/test_libhook.so";
        std::env::set_var("C2RUST_HOOK_LIB", test_path);

        let result = get_hook_library_path();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().to_str().unwrap(), test_path);

        std::env::remove_var("C2RUST_HOOK_LIB");
    }

    #[test]
    #[serial_test::serial]
    fn test_verify_hook_library_not_set() {
        // Clear the environment variable
        std::env::remove_var("C2RUST_HOOK_LIB");

        let result = verify_hook_library();
        assert!(result.is_err());
    }

    #[test]
    #[serial_test::serial]
    fn test_verify_hook_library_nonexistent() {
        // Set to a path that doesn't exist
        std::env::set_var("C2RUST_HOOK_LIB", "/nonexistent/path/libhook.so");

        let result = verify_hook_library();
        assert!(result.is_err());

        std::env::remove_var("C2RUST_HOOK_LIB");
    }
}
