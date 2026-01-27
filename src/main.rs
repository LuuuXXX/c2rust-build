mod config_helper;
mod error;
mod preprocessor;
mod tracker;

use clap::{Args, Parser, Subcommand};
use error::Result;
use std::path::PathBuf;

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
    /// Directory to execute build command (required)
    #[arg(long = "build.dir", required = true)]
    build_dir: String,

    /// Optional feature name (default: "default")
    #[arg(long)]
    feature: Option<String>,

    /// Build command to execute (required, can be multiple arguments)
    #[arg(long = "build.cmd", required = true, num_args = 1..)]
    build_cmd: Vec<String>,
}

fn run(args: CommandArgs) -> Result<()> {
    // 1. Check if c2rust-config exists
    config_helper::check_c2rust_config_exists()?;

    // 2. Get feature name (default to "default")
    let feature = args.feature.as_deref().unwrap_or("default");

    // 3. Get required parameters from command line
    let dir = &args.build_dir;
    let command = args.build_cmd;
    let build_dir = PathBuf::from(dir);

    // 4. Determine the project root directory
    let project_root = if build_dir.is_absolute() {
        if build_dir.file_name().and_then(|n| n.to_str()) == Some("build") {
            build_dir.parent().unwrap_or(&build_dir).to_path_buf()
        } else {
            build_dir.clone()
        }
    } else {
        std::env::current_dir()?
    };

    println!("=== c2rust-build ===");
    println!("Build directory: {}", build_dir.display());
    println!("Project root: {}", project_root.display());
    println!("Feature: {}", feature);
    println!("Command: {}", command.join(" "));
    println!();

    // 5. Track the build process to capture compiler invocations
    println!("Tracking build process...");
    let (compile_entries, compilers) = tracker::track_build(&build_dir, &command, &project_root)?;
    println!("Tracked {} compilation(s)", compile_entries.len());

    if compile_entries.is_empty() {
        println!("Warning: No C file compilations were tracked.");
        println!("Make sure your build command actually compiles C files.");
    } else {
        // 6. Preprocess the tracked C files
        println!("\nPreprocessing C files...");
        let preprocessed_files = preprocessor::preprocess_files(
            &compile_entries,
            feature,
            &project_root,
        )?;
        println!("Preprocessed {} file(s)", preprocessed_files.len());
    }

    // 7. Save configuration using c2rust-config
    let command_str = command.join(" ");
    config_helper::save_config(dir, &command_str, Some(feature))?;
    
    // 8. Save detected compilers to c2rust-config globally
    if !compilers.is_empty() {
        println!("\nSaving detected compilers...");
        config_helper::save_compilers(&compilers)?;
    }

    println!("\n✓ Build tracking and preprocessing completed successfully!");
    println!("✓ Configuration saved.");
    Ok(())
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
