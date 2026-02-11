mod config_helper;
mod error;
mod file_selector;
mod git_helper;
mod target_selector;
mod targets_processor;
mod tracker;

use clap::{Args, Parser, Subcommand};
use error::Result;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "c2rust-build")]
#[command(about = "C project build execution tool for c2rust")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute build command and save configuration
    Build(CommandArgs),
}

#[derive(Args)]
struct CommandArgs {
    /// Optional feature name (default: "default")
    #[arg(long)]
    feature: Option<String>,

    /// Skip interactive file selection and select all files
    #[arg(long)]
    no_interactive: bool,

    /// Build command to execute - use after '--' separator
    /// Example: c2rust-build build -- make CFLAGS="-O2" target
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        required = true,
        value_name = "BUILD_CMD"
    )]
    build_cmd: Vec<String>,
}

/// Clean the feature directory before build to ensure a clean working environment
/// Removes and recreates the .c2rust/<feature> directory
fn clean_feature_directory(project_root: &Path, feature: &str) -> Result<()> {
    let feature_dir = project_root.join(".c2rust").join(feature);
    
    // Check if the directory exists
    if feature_dir.exists() {
        println!("Cleaning feature directory: {}", feature_dir.display());
        
        // Try to remove the directory
        match fs::remove_dir_all(&feature_dir) {
            Ok(_) => {
                println!("Successfully removed old feature directory");
            }
            Err(e) => {
                // Log the error but don't fail the build
                eprintln!(
                    "Warning: Failed to remove feature directory {}: {}",
                    feature_dir.display(),
                    e
                );
                eprintln!("Continuing with build process...");
            }
        }
    }
    
    // Create the feature directory
    println!("Creating clean feature directory: {}", feature_dir.display());
    fs::create_dir_all(&feature_dir).map_err(|e| {
        error::Error::CommandExecutionFailed(format!(
            "Failed to create feature directory {}: {}",
            feature_dir.display(),
            e
        ))
    })?;
    
    println!("Feature directory ready");
    println!();
    
    Ok(())
}

/// Count preprocessed files recursively in directory
fn count_preprocessed_files(dir: &Path) -> Result<usize> {
    let mut count = 0;

    if !dir.exists() {
        return Ok(0);
    }

    fn visit_dir(dir: &Path, count: &mut usize) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;

            // Skip symbolic links to avoid infinite recursion or double-counting
            if file_type.is_symlink() {
                continue;
            }

            if file_type.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "c2rust" || ext == "i" {
                        *count += 1;
                    }
                }
            } else if file_type.is_dir() {
                visit_dir(&path, count)?;
            }
        }
        Ok(())
    }

    visit_dir(dir, &mut count)?;
    Ok(count)
}

fn run(args: CommandArgs) -> Result<()> {
    // Verify hook library is set and exists before proceeding
    tracker::verify_hook_library()?;
    config_helper::check_c2rust_config_exists()?;

    let feature = args.feature.as_deref().unwrap_or("default");
    let command = args.build_cmd;

    let current_dir = std::env::current_dir().map_err(|e| {
        error::Error::CommandExecutionFailed(format!("Failed to get current directory: {}", e))
    })?;

    let project_root = find_project_root(&current_dir)?;

    // Calculate build directory relative to project root, falling back to "." if needed
    let build_dir_relative = current_dir.strip_prefix(&project_root)
        .map(|p| {
            if p.as_os_str().is_empty() {
                ".".to_string()
            } else {
                p.display().to_string()
            }
        })
        .unwrap_or_else(|_| {
            eprintln!("Warning: current directory is not under project root, using '.' as build directory");
            ".".to_string()
        });

    println!("=== c2rust-build ===");
    println!("Project root: {}", project_root.display());
    println!("Build directory (relative): {}", build_dir_relative);
    println!("Feature: {}", feature);
    println!("Command: {}", command.join(" "));
    println!();

    // Clean the feature directory before build to ensure a clean working environment
    clean_feature_directory(&project_root, feature)?;

    println!("Tracking build process...");
    let compilers = tracker::track_build(&current_dir, &command, &project_root, feature)?;

    // Check for preprocessed files instead of compile_entries
    let c_dir = project_root.join(".c2rust").join(feature).join("c");
    let preprocessed_count = count_preprocessed_files(&c_dir)?;

    println!("Generated {} preprocessed file(s)", preprocessed_count);

    if preprocessed_count == 0 {
        println!("Warning: No C file compilations were tracked.");
        println!("Make sure your build command actually compiles C files.");
    }

    println!("\nNote: Files and targets are generated directly by libhook.so");
    println!("Files are located at: .c2rust/{}/c/", feature);

    // Target artifact selection step (do this first)
    let selected_target =
        target_selector::process_and_select_target(&project_root, feature, args.no_interactive)?;

    // File selection step (select files that participate in building the target)
    if preprocessed_count > 0 {
        file_selector::process_and_select_files(
            &c_dir,
            feature,
            &project_root,
            args.no_interactive,
            selected_target.as_deref(),
        )?;
    }

    let command_str = command.join(" ");
    config_helper::save_config(
        &build_dir_relative,
        &command_str,
        Some(feature),
        &project_root,
    )?;

    // Save selected target if one was chosen
    if let Some(target) = selected_target {
        config_helper::save_target(&target, Some(feature), &project_root)?;
    }

    if !compilers.is_empty() {
        println!("\nSaving detected compilers...");
        config_helper::save_compilers(&compilers, &project_root)?;
    }

    // Auto-commit changes in .c2rust directory if any
    git_helper::auto_commit_if_modified(&project_root)?;

    println!("\n✓ Build tracking completed successfully!");
    println!("✓ Configuration saved.");
    println!("\nOutput structure:");
    println!("  .c2rust/");
    println!("    └── {}/", feature);
    println!("        ├── c/");
    println!("        │   ├── targets.list        # List of discovered binary targets");
    println!("        │   └── <path>/");
    println!("        │       └── *.c2rust (or *.i)");
    println!("        └── selected_files.json");
    Ok(())
}

/// Find the project root directory.
/// Searches for .c2rust directory upward from start_dir.
/// If not found, returns the start_dir as root.
///
/// Note: On first run, if .c2rust doesn't exist, this returns the starting directory.
/// The .c2rust directory will be created at this location during the build process.
/// On subsequent runs, it will find the previously created .c2rust directory.
fn find_project_root(start_dir: &Path) -> Result<PathBuf> {
    // Search for .c2rust directory
    let mut current = start_dir.to_path_buf();

    loop {
        let c2rust_dir = current.join(".c2rust");

        // Use metadata() instead of exists() to detect permission/IO errors
        match std::fs::metadata(&c2rust_dir) {
            Ok(metadata) if metadata.is_dir() => {
                return Ok(current);
            }
            Ok(_) => {
                // .c2rust exists but is not a directory - continue searching
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // .c2rust doesn't exist - continue searching
            }
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                // Permission denied - warn and continue searching
                eprintln!(
                    "Warning: Permission denied accessing {}, continuing search",
                    c2rust_dir.display()
                );
            }
            Err(e) => {
                // Other IO errors - warn and continue searching
                eprintln!(
                    "Warning: Error accessing {}: {}, continuing search",
                    c2rust_dir.display(),
                    e
                );
            }
        }

        // Try to go up one directory
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => {
                // Reached filesystem root, use the starting directory
                return Ok(start_dir.to_path_buf());
            }
        }
    }
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Build(args) => run(args),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_project_root_with_c2rust_in_current_dir() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        let c2rust_dir = root.join(".c2rust");

        fs::create_dir_all(&c2rust_dir).unwrap();

        let result = find_project_root(root).unwrap();

        assert_eq!(result, root);
    }

    #[test]
    fn test_find_project_root_with_c2rust_in_parent_dir() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        let c2rust_dir = root.join(".c2rust");
        let subdir = root.join("subdir");

        fs::create_dir_all(&c2rust_dir).unwrap();
        fs::create_dir_all(&subdir).unwrap();

        let result = find_project_root(&subdir).unwrap();

        assert_eq!(result, root);
    }

    #[test]
    fn test_find_project_root_with_deeply_nested_subdirs() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        let c2rust_dir = root.join(".c2rust");
        let deep_dir = root.join("a").join("b").join("c").join("d");

        fs::create_dir_all(&c2rust_dir).unwrap();
        fs::create_dir_all(&deep_dir).unwrap();

        let result = find_project_root(&deep_dir).unwrap();

        assert_eq!(result, root);
    }

    #[test]
    fn test_find_project_root_without_c2rust_fallback_to_start_dir() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        let subdir = root.join("build");

        fs::create_dir_all(&subdir).unwrap();

        let result = find_project_root(&subdir).unwrap();

        // Should fall back to the starting directory when .c2rust is not found
        assert_eq!(result, subdir);
    }

    #[test]
    fn test_find_project_root_c2rust_as_file_not_directory() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        let c2rust_file = root.join(".c2rust");
        let subdir = root.join("subdir");

        // Create .c2rust as a file, not a directory
        fs::write(&c2rust_file, "not a directory").unwrap();
        fs::create_dir_all(&subdir).unwrap();

        let result = find_project_root(&subdir).unwrap();

        // Should continue searching and fall back to start dir since .c2rust is not a directory
        assert_eq!(result, subdir);
    }

    #[test]
    fn test_find_project_root_multiple_c2rust_dirs_finds_closest() {
        let temp_dir = TempDir::new().unwrap();
        let outer_root = temp_dir.path();
        let outer_c2rust = outer_root.join(".c2rust");
        let inner_root = outer_root.join("project");
        let inner_c2rust = inner_root.join(".c2rust");
        let work_dir = inner_root.join("src");

        fs::create_dir_all(&outer_c2rust).unwrap();
        fs::create_dir_all(&inner_c2rust).unwrap();
        fs::create_dir_all(&work_dir).unwrap();

        let result = find_project_root(&work_dir).unwrap();

        // Should find the closest .c2rust directory (inner_root, not outer_root)
        assert_eq!(result, inner_root);
    }

    #[test]
    fn test_count_preprocessed_files_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        fs::create_dir_all(&c_dir).unwrap();

        let count = count_preprocessed_files(&c_dir).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_count_preprocessed_files_nonexistent_directory() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("nonexistent");

        let count = count_preprocessed_files(&c_dir).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_count_preprocessed_files_with_c2rust_files() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        fs::create_dir_all(&c_dir).unwrap();

        // Create .c2rust files
        fs::write(c_dir.join("file1.c2rust"), "content").unwrap();
        fs::write(c_dir.join("file2.c2rust"), "content").unwrap();

        let count = count_preprocessed_files(&c_dir).unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_count_preprocessed_files_with_i_files() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        fs::create_dir_all(&c_dir).unwrap();

        // Create .i files
        fs::write(c_dir.join("file1.i"), "content").unwrap();
        fs::write(c_dir.join("file2.i"), "content").unwrap();

        let count = count_preprocessed_files(&c_dir).unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_count_preprocessed_files_mixed_types() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        fs::create_dir_all(&c_dir).unwrap();

        // Create mix of .c2rust and .i files
        fs::write(c_dir.join("file1.c2rust"), "content").unwrap();
        fs::write(c_dir.join("file2.i"), "content").unwrap();
        // Also add files that should not be counted
        fs::write(c_dir.join("file3.c"), "content").unwrap();
        fs::write(c_dir.join("file4.txt"), "content").unwrap();

        let count = count_preprocessed_files(&c_dir).unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_count_preprocessed_files_nested() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        fs::create_dir_all(c_dir.join("subdir1").join("subdir2")).unwrap();

        // Create files at different levels
        fs::write(c_dir.join("file1.c2rust"), "content").unwrap();
        fs::write(c_dir.join("subdir1").join("file2.c2rust"), "content").unwrap();
        fs::write(
            c_dir.join("subdir1").join("subdir2").join("file3.i"),
            "content",
        )
        .unwrap();

        let count = count_preprocessed_files(&c_dir).unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_count_preprocessed_files_skips_symlinks() {
        let temp_dir = TempDir::new().unwrap();
        let c_dir = temp_dir.path().join("c");
        let real_dir = temp_dir.path().join("real");
        fs::create_dir_all(&c_dir).unwrap();
        fs::create_dir_all(&real_dir).unwrap();

        // Create a real file
        fs::write(c_dir.join("file1.c2rust"), "content").unwrap();
        fs::write(real_dir.join("file2.c2rust"), "content").unwrap();

        // Create a symlink to the real directory (on Unix systems)
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let link_path = c_dir.join("link_to_real");
            symlink(&real_dir, &link_path).unwrap();

            // Should only count file1.c2rust, not file2.c2rust (which is behind a symlink)
            let count = count_preprocessed_files(&c_dir).unwrap();
            assert_eq!(count, 1);
        }

        // On non-Unix systems, just count the one file
        #[cfg(not(unix))]
        {
            let count = count_preprocessed_files(&c_dir).unwrap();
            assert_eq!(count, 1);
        }
    }

    #[test]
    fn test_clean_feature_directory_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        // Clean a non-existent directory - should create it
        let result = clean_feature_directory(project_root, "test_feature");
        assert!(result.is_ok());

        // Verify the directory was created
        let feature_dir = project_root.join(".c2rust").join("test_feature");
        assert!(feature_dir.exists());
        assert!(feature_dir.is_dir());
    }

    #[test]
    fn test_clean_feature_directory_existing_empty() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();
        let feature_dir = project_root.join(".c2rust").join("test_feature");

        // Create an existing empty directory
        fs::create_dir_all(&feature_dir).unwrap();
        assert!(feature_dir.exists());

        // Clean the directory
        let result = clean_feature_directory(project_root, "test_feature");
        assert!(result.is_ok());

        // Verify the directory still exists and is empty
        assert!(feature_dir.exists());
        assert!(feature_dir.is_dir());
    }

    #[test]
    fn test_clean_feature_directory_existing_with_files() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();
        let feature_dir = project_root.join(".c2rust").join("test_feature");
        let c_dir = feature_dir.join("c");

        // Create directory with some files
        fs::create_dir_all(&c_dir).unwrap();
        let test_file = c_dir.join("test.c2rust");
        fs::write(&test_file, "old content").unwrap();
        assert!(test_file.exists());

        // Clean the directory
        let result = clean_feature_directory(project_root, "test_feature");
        assert!(result.is_ok());

        // Verify the directory exists but the old file is gone
        assert!(feature_dir.exists());
        assert!(!test_file.exists());
    }

    #[test]
    fn test_clean_feature_directory_nested_structure() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();
        let feature_dir = project_root.join(".c2rust").join("test_feature");

        // Create a nested structure
        let nested_dir = feature_dir.join("c").join("src").join("subdir");
        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(nested_dir.join("file1.c2rust"), "content1").unwrap();
        fs::write(feature_dir.join("c").join("file2.c2rust"), "content2").unwrap();
        fs::write(feature_dir.join("config.json"), "config").unwrap();

        // Verify files exist
        assert!(nested_dir.join("file1.c2rust").exists());
        assert!(feature_dir.join("c").join("file2.c2rust").exists());
        assert!(feature_dir.join("config.json").exists());

        // Clean the directory
        let result = clean_feature_directory(project_root, "test_feature");
        assert!(result.is_ok());

        // Verify all old files are gone
        assert!(feature_dir.exists());
        assert!(!nested_dir.join("file1.c2rust").exists());
        assert!(!feature_dir.join("c").join("file2.c2rust").exists());
        assert!(!feature_dir.join("config.json").exists());
    }

    #[test]
    fn test_clean_feature_directory_preserves_other_features() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        // Create multiple feature directories
        let feature1_dir = project_root.join(".c2rust").join("feature1");
        let feature2_dir = project_root.join(".c2rust").join("feature2");

        fs::create_dir_all(&feature1_dir).unwrap();
        fs::create_dir_all(&feature2_dir).unwrap();

        let file1 = feature1_dir.join("test1.txt");
        let file2 = feature2_dir.join("test2.txt");

        fs::write(&file1, "content1").unwrap();
        fs::write(&file2, "content2").unwrap();

        // Clean only feature1
        let result = clean_feature_directory(project_root, "feature1");
        assert!(result.is_ok());

        // Verify feature1 exists but file1 is gone
        assert!(feature1_dir.exists());
        assert!(!file1.exists());

        // Verify feature2 and file2 are untouched
        assert!(feature2_dir.exists());
        assert!(file2.exists());
    }
}
