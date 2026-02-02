mod config_helper;
mod error;
mod git_helper;
mod preprocessor;
mod tracker;

use clap::{Args, Parser, Subcommand};
use error::Result;
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

fn run(args: CommandArgs) -> Result<()> {
    config_helper::check_c2rust_config_exists()?;
    preprocessor::verify_clang()?;

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
    println!(
        "Clang: {}",
        std::env::var("C2RUST_CLANG").unwrap_or_else(|_| "clang".to_string())
    );
    println!();

    println!("Tracking build process...");
    let (compile_entries, compilers) = tracker::track_build(&current_dir, &command, &project_root)?;
    println!("Tracked {} compilation(s)", compile_entries.len());

    if compile_entries.is_empty() {
        println!("Warning: No C file compilations were tracked.");
        println!("Make sure your build command actually compiles C files.");
    } else {
        println!("\nPreprocessing C files...");
        let preprocessed_files =
            preprocessor::preprocess_files(&compile_entries, feature, &project_root)?;
        println!("Preprocessed {} file(s)", preprocessed_files.len());
    }

    let command_str = command.join(" ");
    config_helper::save_config(
        &build_dir_relative,
        &command_str,
        Some(feature),
        &project_root,
    )?;

    if !compilers.is_empty() {
        println!("\nSaving detected compilers...");
        config_helper::save_compilers(&compilers, &project_root)?;
    }

    // Auto-commit changes in .c2rust directory if any
    git_helper::auto_commit_if_modified(&project_root)?;

    println!("\n✓ Build tracking and preprocessing completed successfully!");
    println!("✓ Configuration saved.");
    println!("\nOutput structure:");
    println!("  .c2rust/");
    println!("    ├── compile_commands.json");
    println!("    ├── compile_output.txt");
    println!("    └── {}/", feature);
    println!("        └── c/");
    println!("            └── <path>/");
    println!("                └── *.c.c2rust");
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
}
