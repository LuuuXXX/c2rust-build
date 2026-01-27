mod config_helper;
mod error;
mod executor;
mod interaction;
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
    #[arg(long, required = true)]
    dir: String,

    /// Optional feature name (default: "default")
    #[arg(long)]
    feature: Option<String>,

    /// Build command to execute (e.g., "make") (required)
    #[arg(last = true, required = true)]
    command: Vec<String>,
}

fn run(args: CommandArgs) -> Result<()> {
    // 1. Check if c2rust-config exists
    config_helper::check_c2rust_config_exists()?;

    // 2. Get feature name (default to "default")
    let feature = args.feature.as_deref().unwrap_or("default");

    // 3. Get required parameters from command line
    let dir = &args.dir;
    let command = args.command;
    
    // Validate that command is not empty
    if command.is_empty() {
        return Err(error::Error::MissingParameter(
            "Build command is required. Provide command arguments after --".to_string(),
        ));
    }
    
    let build_dir = PathBuf::from(dir);

    println!("=== c2rust-build ===");
    println!("Build directory: {}", build_dir.display());
    println!("Feature: {}", feature);
    println!("Command: {}", command.join(" "));
    println!();

    // 4. Track the build process to capture compiler invocations
    println!("Tracking build process...");
    // Use the build directory as the project root so all artifacts share the same .c2rust directory
    let compile_entries = tracker::track_build(&build_dir, &command, &build_dir)?;
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

        // 6. Group files by module
        let modules = preprocessor::group_by_module(&preprocessed_files);

        // 7. User interaction for module selection
        // Check if running in interactive environment (TTY available)
        let selected_modules = if atty::is(atty::Stream::Stdin) {
            // Interactive mode: let user select
            interaction::select_modules(&modules)?
        } else {
            // Non-interactive mode (CI/CD): select all modules
            println!("\nNon-interactive environment detected, keeping all modules.");
            modules.keys().cloned().collect()
        };

        // Delete unselected modules
        let unselected_modules: Vec<_> = modules
            .keys()
            .filter(|k| !selected_modules.contains(k))
            .collect();

        if !unselected_modules.is_empty() {
            println!("\nRemoving {} unselected module(s)...", unselected_modules.len());
            for module_name in unselected_modules {
                if let Some(files) = modules.get(module_name) {
                    preprocessor::delete_module_files(files)?;
                    println!("  Removed: {}", module_name);
                }
            }
        }
    }

    // 8. Save configuration using c2rust-config for backward compatibility
    let command_str = command.join(" ");
    config_helper::save_config(dir, &command_str, Some(feature))?;

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
