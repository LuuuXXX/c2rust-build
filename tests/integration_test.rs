use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_build_command_basic() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    cmd.arg("build")
        .arg("--")
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
    
    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    // Point C2RUST_CONFIG to a nonexistent path to ensure deterministic behavior
    // This will cause the tool to fail with "c2rust-config not found" error
    cmd.arg("build")
        .arg("--")
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

    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    // Missing build command after --
    cmd.arg("build")
        .current_dir(temp_dir.path())
        .env("C2RUST_CONFIG", "/nonexistent/c2rust-config");

    // Should fail with error about missing build command
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("required arguments were not provided"));
}

#[test]
fn test_build_command_with_separator() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    // Test the new -- separator format
    cmd.arg("build")
        .arg("--")
        .arg("make")
        .arg("CFLAGS=-O2")
        .arg("target")
        .current_dir(temp_dir.path())
        .env("C2RUST_CONFIG", "/nonexistent/c2rust-config");

    // Should fail with c2rust-config not found error
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("c2rust-config not found"));
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
        .stdout(predicate::str::contains("BUILD_CMD"))
        .stdout(predicate::str::contains("--feature"));
}

#[test]
fn test_build_with_feature() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    cmd.arg("build")
        .arg("--feature")
        .arg("debug")
        .arg("--")
        .arg("echo")
        .arg("build")
        .current_dir(temp_dir.path())
        .env("C2RUST_CONFIG", "/nonexistent/c2rust-config");

    // Should fail with c2rust-config not found error
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("c2rust-config not found"));
}

#[test]
fn test_build_command_with_flags() {
    let temp_dir = TempDir::new().unwrap();
    
    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    // Test that flags after -- are treated as build command args
    cmd.arg("build")
        .arg("--")
        .arg("make")
        .arg("-j4")
        .arg("--verbose")
        .current_dir(temp_dir.path())
        .env("C2RUST_CONFIG", "/nonexistent/c2rust-config");

    // Should fail with c2rust-config not found (checked before directory access)
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("c2rust-config not found"));
}
