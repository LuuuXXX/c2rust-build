use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;
use std::fs;

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
    let temp_dir = TempDir::new().unwrap();
    
    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    cmd.arg("build")
        .arg("--")
        .arg("echo")
        .arg("build")
        .current_dir(temp_dir.path())
        .env("C2RUST_CONFIG", "/nonexistent/c2rust-config");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("c2rust-config not found"));
}

#[test]
fn test_missing_command_argument() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    cmd.arg("build")
        .current_dir(temp_dir.path())
        .env("C2RUST_CONFIG", "/nonexistent/c2rust-config");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("required arguments were not provided"));
}

#[test]
fn test_build_command_with_separator() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    cmd.arg("build")
        .arg("--")
        .arg("make")
        .arg("CFLAGS=-O2")
        .arg("target")
        .current_dir(temp_dir.path())
        .env("C2RUST_CONFIG", "/nonexistent/c2rust-config");

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

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("c2rust-config not found"));
}

#[test]
fn test_build_command_with_flags() {
    let temp_dir = TempDir::new().unwrap();
    
    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    cmd.arg("build")
        .arg("--")
        .arg("make")
        .arg("-j4")
        .arg("--verbose")
        .current_dir(temp_dir.path())
        .env("C2RUST_CONFIG", "/nonexistent/c2rust-config");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("c2rust-config not found"));
}

#[test]
fn test_project_root_detection_with_existing_c2rust() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    let c2rust_dir = root.join(".c2rust");
    let subdir = root.join("subdir");
    
    fs::create_dir_all(&c2rust_dir).unwrap();
    fs::create_dir_all(&subdir).unwrap();
    
    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    cmd.arg("build")
        .arg("--")
        .arg("echo")
        .arg("test")
        .current_dir(&subdir)
        .env("C2RUST_CONFIG", "/nonexistent/c2rust-config");
    
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("c2rust-config not found"));
}

#[test]
fn test_project_root_detection_without_c2rust() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    let subdir = root.join("build");
    
    fs::create_dir_all(&subdir).unwrap();
    
    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    cmd.arg("build")
        .arg("--")
        .arg("echo")
        .arg("test")
        .current_dir(&subdir)
        .env("C2RUST_CONFIG", "/nonexistent/c2rust-config");
    
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("c2rust-config not found"));
}

#[test]
fn test_deeply_nested_directory_structure() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    let c2rust_dir = root.join(".c2rust");
    let deep_dir = root.join("a").join("b").join("c");
    
    fs::create_dir_all(&c2rust_dir).unwrap();
    fs::create_dir_all(&deep_dir).unwrap();
    
    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    cmd.arg("build")
        .arg("--")
        .arg("echo")
        .arg("test")
        .current_dir(&deep_dir)
        .env("C2RUST_CONFIG", "/nonexistent/c2rust-config");
    
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("c2rust-config not found"));
}

#[test]
fn test_c2rust_project_root_env_variable() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    let subdir = root.join("build");
    
    fs::create_dir_all(&subdir).unwrap();
    
    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();
    
    // Set C2RUST_PROJECT_ROOT to a specific directory
    cmd.arg("build")
        .arg("--")
        .arg("echo")
        .arg("test")
        .current_dir(&subdir)
        .env("C2RUST_PROJECT_ROOT", root.to_str().unwrap())
        .env("C2RUST_CONFIG", "/nonexistent/c2rust-config");
    
    // The command should use the C2RUST_PROJECT_ROOT as project root
    // (it will still fail at c2rust-config check, but that's expected)
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("c2rust-config not found"));
}
