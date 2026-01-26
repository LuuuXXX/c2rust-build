use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_build_command_basic() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path().to_str().unwrap();

    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    cmd.arg("build")
        .arg("--dir")
        .arg(dir_path)
        .arg("--")
        .arg("echo")
        .arg("building");

    // The command should fail because c2rust-config is not installed
    // We verify it fails with the expected error message
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("c2rust-config not found"));
}

#[test]
fn test_build_with_feature() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path().to_str().unwrap();

    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    cmd.arg("build")
        .arg("--feature")
        .arg("debug")
        .arg("--dir")
        .arg(dir_path)
        .arg("--")
        .arg("echo")
        .arg("build");

    // Should fail with c2rust-config not found error
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("c2rust-config not found"));
}

#[test]
fn test_missing_dir_argument() {
    // Create a fresh temp directory to isolate config state
    let temp_dir = TempDir::new().unwrap();
    
    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    // Point C2RUST_CONFIG to a nonexistent path to ensure deterministic behavior
    // This will cause the tool to fail with "c2rust-config not found" error
    cmd.arg("build")
        .arg("--")
        .arg("echo")
        .arg("build")
        .current_dir(temp_dir.path())  // Run in isolated directory
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
    
    cmd.arg("build")
        .arg("--dir")
        .arg(dir_path);

    cmd.assert()
        .failure();
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
        .stdout(predicate::str::contains("--dir"))
        .stdout(predicate::str::contains("--feature"));
}

#[test]
fn test_nonexistent_directory() {
    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    cmd.arg("build")
        .arg("--dir")
        .arg("/nonexistent/directory/path")
        .arg("--")
        .arg("echo")
        .arg("test");

    // Should fail with c2rust-config not found (checked before directory access)
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("c2rust-config not found"));
}
