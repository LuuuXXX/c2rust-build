mod config_helper;
mod error;
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
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true, value_name = "BUILD_CMD")]
    build_cmd: Vec<String>,
}

fn run(args: CommandArgs) -> Result<()> {
    // 1. Check if c2rust-config exists
    config_helper::check_c2rust_config_exists()?;

    // 2. Verify clang is available
    preprocessor::verify_clang()?;

    // 3. Get feature name (default to "default")
    let feature = args.feature.as_deref().unwrap_or("default");

    // 4. Get build command from arguments
    let command = args.build_cmd;
    
    // 5. Get the current working directory (where the command is executed)
    let current_dir = std::env::current_dir()
        .map_err(|e| error::Error::CommandExecutionFailed(
            format!("Failed to get current directory: {}", e)
        ))?;
    
    // 6. Find the project root (where .c2rust will be created)
    // Start from current directory and search upward for .c2rust or use current as root
    let project_root = find_project_root(&current_dir)?;
    
    // 7. Calculate the build directory relative to project root
    // Note: If current_dir is not a descendant of project_root (which shouldn't happen
    // based on find_project_root logic), we fall back to "." as a safe default.
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
    println!("Clang: {}", std::env::var("C2RUST_CLANG").unwrap_or_else(|_| "clang".to_string()));
    println!();

    // 8. Track the build process to capture compiler invocations
    println!("Tracking build process...");
    let (compile_entries, compilers) = tracker::track_build(&current_dir, &command, &project_root)?;
    println!("Tracked {} compilation(s)", compile_entries.len());

    if compile_entries.is_empty() {
        println!("Warning: No C file compilations were tracked.");
        println!("Make sure your build command actually compiles C files.");
    } else {
        // 9. Preprocess the tracked C files
        println!("\nPreprocessing C files...");
        let preprocessed_files = preprocessor::preprocess_files(
            &compile_entries,
            feature,
            &project_root,
        )?;
        println!("Preprocessed {} file(s)", preprocessed_files.len());
    }

    // 10. Save configuration using c2rust-config
    let command_str = command.join(" ");
    config_helper::save_config(&build_dir_relative, &command_str, Some(feature), &project_root)?;
    
    // 11. Save detected compilers to c2rust-config globally
    if !compilers.is_empty() {
        println!("\nSaving detected compilers...");
        config_helper::save_compilers(&compilers, &project_root)?;
    }

    println!("\n✓ Build tracking and preprocessing completed successfully!");
    println!("✓ Configuration saved.");
    println!("\nOutput structure:");
    println!("  .c2rust/");
    println!("    ├── compile_commands.json");
    println!("    ├── compile_output.txt");
    println!("    └── {}/", feature);
    println!("        └── <path>/");
    println!("            └── *.c.c2rust");
    Ok(())
}

/// Find the project root directory by searching for .c2rust directory
/// or return the current directory as the root.
/// 
/// Note: On first run, if .c2rust doesn't exist, this returns the starting directory.
/// The .c2rust directory will be created at this location during the build process.
/// On subsequent runs, it will find the previously created .c2rust directory.
fn find_project_root(start_dir: &Path) -> Result<PathBuf> {
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
                eprintln!("Warning: Permission denied accessing {}, continuing search", c2rust_dir.display());
            }
            Err(e) => {
                // Other IO errors - warn and continue searching
                eprintln!("Warning: Error accessing {}: {}, continuing search", c2rust_dir.display(), e);
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
