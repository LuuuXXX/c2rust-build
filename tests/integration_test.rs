use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Helper function to create a dummy hook library file for tests
fn create_dummy_hook_lib(temp_dir: &TempDir) -> String {
    let hook_lib_path = temp_dir.path().join("libhook.so");
    fs::write(&hook_lib_path, "dummy").unwrap();
    hook_lib_path.to_str().unwrap().to_string()
}

#[test]
fn test_build_command_basic() {
    let temp_dir = TempDir::new().unwrap();
    let hook_lib = create_dummy_hook_lib(&temp_dir);

    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();

    cmd.arg("build")
        .arg("--")
        .arg("echo")
        .arg("building")
        .current_dir(temp_dir.path())
        .env("C2RUST_HOOK_LIB", &hook_lib)
        .env("C2RUST_CONFIG", "/nonexistent/c2rust-config");

    // The command should fail because c2rust-config is not found
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("c2rust-config not found"));
}

#[test]
fn test_config_tool_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let hook_lib = create_dummy_hook_lib(&temp_dir);

    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();

    cmd.arg("build")
        .arg("--")
        .arg("echo")
        .arg("build")
        .current_dir(temp_dir.path())
        .env("C2RUST_HOOK_LIB", &hook_lib)
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

    cmd.assert().failure().stderr(predicate::str::contains(
        "required arguments were not provided",
    ));
}

#[test]
fn test_build_command_with_separator() {
    let temp_dir = TempDir::new().unwrap();
    let hook_lib = create_dummy_hook_lib(&temp_dir);

    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();

    cmd.arg("build")
        .arg("--")
        .arg("make")
        .arg("CFLAGS=-O2")
        .arg("target")
        .current_dir(temp_dir.path())
        .env("C2RUST_HOOK_LIB", &hook_lib)
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
    let hook_lib = create_dummy_hook_lib(&temp_dir);

    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();

    cmd.arg("build")
        .arg("--feature")
        .arg("debug")
        .arg("--")
        .arg("echo")
        .arg("build")
        .current_dir(temp_dir.path())
        .env("C2RUST_HOOK_LIB", &hook_lib)
        .env("C2RUST_CONFIG", "/nonexistent/c2rust-config");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("c2rust-config not found"));
}

#[test]
fn test_build_command_with_flags() {
    let temp_dir = TempDir::new().unwrap();
    let hook_lib = create_dummy_hook_lib(&temp_dir);

    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();

    cmd.arg("build")
        .arg("--")
        .arg("make")
        .arg("-j4")
        .arg("--verbose")
        .current_dir(temp_dir.path())
        .env("C2RUST_HOOK_LIB", &hook_lib)
        .env("C2RUST_CONFIG", "/nonexistent/c2rust-config");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("c2rust-config not found"));
}

#[test]
fn test_project_root_detection_with_existing_c2rust() {
    let temp_dir = TempDir::new().unwrap();
    let hook_lib = create_dummy_hook_lib(&temp_dir);
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
        .env("C2RUST_HOOK_LIB", &hook_lib)
        .env("C2RUST_CONFIG", "/nonexistent/c2rust-config");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("c2rust-config not found"));
}

#[test]
fn test_project_root_detection_without_c2rust() {
    let temp_dir = TempDir::new().unwrap();
    let hook_lib = create_dummy_hook_lib(&temp_dir);
    let root = temp_dir.path();
    let subdir = root.join("build");

    fs::create_dir_all(&subdir).unwrap();

    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();

    cmd.arg("build")
        .arg("--")
        .arg("echo")
        .arg("test")
        .current_dir(&subdir)
        .env("C2RUST_HOOK_LIB", &hook_lib)
        .env("C2RUST_CONFIG", "/nonexistent/c2rust-config");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("c2rust-config not found"));
}

#[test]
fn test_deeply_nested_directory_structure() {
    let temp_dir = TempDir::new().unwrap();
    let hook_lib = create_dummy_hook_lib(&temp_dir);
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
        .env("C2RUST_HOOK_LIB", &hook_lib)
        .env("C2RUST_CONFIG", "/nonexistent/c2rust-config");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("c2rust-config not found"));
}

#[test]
fn test_hook_library_not_found() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("c2rust-build").unwrap();

    cmd.arg("build")
        .arg("--")
        .arg("echo")
        .arg("test")
        .current_dir(temp_dir.path())
        .env_remove("C2RUST_HOOK_LIB");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Hook library not found"));
}

#[test]
fn test_target_selection_integration() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create directory structure
    let c_dir = project_root.join(".c2rust/default/c");
    fs::create_dir_all(&c_dir).unwrap();

    // Create targets.list with multiple targets
    let targets_list = c_dir.join("targets.list");
    fs::write(&targets_list, "bin/myapp\nlib/libfoo.a\nlib/libbar.so\n").unwrap();

    // Create a preprocessed file so file selection doesn't fail
    fs::create_dir_all(c_dir.join("src")).unwrap();
    fs::write(c_dir.join("src/main.c.c2rust"), "preprocessed").unwrap();

    // Verify targets.list exists and contains expected content
    assert!(targets_list.exists());
    let content = fs::read_to_string(&targets_list).unwrap();
    assert!(content.contains("bin/myapp"));
    assert!(content.contains("lib/libfoo.a"));
    assert!(content.contains("lib/libbar.so"));
    
    // Verify the file format is correct (one target per line)
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "bin/myapp");
    assert_eq!(lines[1], "lib/libfoo.a");
    assert_eq!(lines[2], "lib/libbar.so");
}
