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

    println!("=== c2rust-build ===");
    println!("Build directory: {}", build_dir.display());
    println!("Feature: {}", feature);
    println!("Command: {}", command.join(" "));
    println!();

    // 4. Track the build process to capture compiler invocations
    println!("Tracking build process...");
    // Use the build directory as the project root so all artifacts share the same .c2rust directory
    let (compile_entries, compilers) = tracker::track_build(&build_dir, &command, &build_dir)?;
    println!("Tracked {} compilation(s)", compile_entries.len());

    if compile_entries.is_empty() {
        println!("Warning: No C file compilations were tracked.");
        println!("Make sure your build command actually compiles C files.");
    } else {
        // 5. Preprocess the tracked C files
        println!("\nPreprocessing C files...");
        let preprocessed_files = preprocessor::preprocess_files(
            &compile_entries,
            feature,
            &build_dir,
        )?;
        println!("Preprocessed {} file(s)", preprocessed_files.len());
    }

    // 6. Save configuration using c2rust-config
    let command_str = command.join(" ");
    config_helper::save_config(dir, &command_str, Some(feature))?;
    
    // 7. Save detected compilers to c2rust-config globally
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
