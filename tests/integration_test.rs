use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_build_command_basic() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path().to_str().unwrap();

    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    cmd.arg("build")
        .arg("--build.dir")
        .arg(dir_path)
        .arg("--build.cmd")
        .arg("echo")
        .arg("building")
        .current_dir(temp_dir.path())
        .env("C2RUST_CONFIG", "/nonexistent/c2rust-config");

    // The command should fail because c2rust-config is not found
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("c2rust-config not found"));
}

#[test]
fn test_config_tool_not_found() {
    // Create a fresh temp directory to isolate config state
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path().to_str().unwrap();
    
    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    // Point C2RUST_CONFIG to a nonexistent path to ensure deterministic behavior
    // This will cause the tool to fail with "c2rust-config not found" error
    cmd.arg("build")
        .arg("--build.dir")
        .arg(dir_path)
        .arg("--build.cmd")
        .arg("echo")
        .arg("build")
        .current_dir(temp_dir.path())
        .env("C2RUST_CONFIG", "/nonexistent/c2rust-config");

    // With C2RUST_CONFIG pointing to nonexistent path, should fail with tool-not-found error
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("c2rust-config not found"));
}

#[test]
fn test_missing_command_argument() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path().to_str().unwrap();

    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    // Missing --build.cmd argument
    cmd.arg("build")
        .arg("--build.dir")
        .arg(dir_path)
        .current_dir(temp_dir.path())
        .env("C2RUST_CONFIG", "/nonexistent/c2rust-config");

    // Should fail with error about missing --build.cmd
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("required arguments were not provided"));
}

#[test]
fn test_missing_dir_argument() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    // Missing --build.dir argument (only command provided)
    cmd.arg("build")
        .arg("--build.cmd")
        .arg("echo")
        .arg("test")
        .current_dir(temp_dir.path())
        .env("C2RUST_CONFIG", "/nonexistent/c2rust-config");

    // Should fail with error about missing --build.dir
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("required arguments were not provided"));
}

#[test]
fn test_help_output() {
    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("c2rust-build"))
        .stdout(predicate::str::contains("C project build execution tool"));
}

#[test]
fn test_build_subcommand_help() {
    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    cmd.arg("build").arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Execute build command"))
        .stdout(predicate::str::contains("--build.dir"))
        .stdout(predicate::str::contains("--build.cmd"));
}

#[test]
fn test_nonexistent_directory() {
    let temp_dir = TempDir::new().unwrap();
    
    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    cmd.arg("build")
        .arg("--build.dir")
        .arg("/nonexistent/directory/path")
        .arg("--build.cmd")
        .arg("echo")
        .arg("test")
        .current_dir(temp_dir.path())
        .env("C2RUST_CONFIG", "/nonexistent/c2rust-config");

    // Should fail with c2rust-config not found (checked before directory access)
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("c2rust-config not found"));
}
