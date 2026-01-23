mod config_helper;
mod error;
mod executor;

use clap::{Args, Parser, Subcommand};
use error::Result;

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
    /// Directory to execute build command
    #[arg(long, required = true)]
    dir: String,

    /// Optional feature name
    #[arg(long)]
    feature: Option<String>,

    /// Build command to execute (e.g., "make")
    #[arg(last = true, required = true)]
    command: Vec<String>,
}

fn run(args: CommandArgs) -> Result<()> {
    // 1. Validate the directory exists before doing anything else
    // This provides better error messages for user mistakes
    let dir_path = std::path::Path::new(&args.dir);
    if !dir_path.exists() {
        return Err(error::Error::CommandExecutionFailed(format!(
            "Directory '{}' does not exist",
            args.dir
        )));
    }
    if !dir_path.is_dir() {
        return Err(error::Error::CommandExecutionFailed(format!(
            "'{}' is not a directory",
            args.dir
        )));
    }

    // 2. Check if c2rust-config exists
    config_helper::check_c2rust_config_exists()?;

    // 3. Execute the build command
    executor::execute_command(&args.dir, &args.command)?;

    // 4. Save configuration using c2rust-config
    // Note: Command arguments are joined with spaces. If arguments contain spaces
    // or special characters, they may not be preserved exactly as originally provided.
    let command_str = args.command.join(" ");
    config_helper::save_config(&args.dir, &command_str, args.feature.as_deref())?;

    // 5. Print success message with details
    match args.feature.as_deref() {
        Some(feature) => {
            println!(
                "✓ Build command '{}' executed successfully in '{}'",
                command_str, args.dir
            );
            println!("✓ Configuration saved with feature '{}'", feature);
        }
        None => {
            println!(
                "✓ Build command '{}' executed successfully in '{}'",
                command_str, args.dir
            );
            println!("✓ Configuration saved");
        }
    }
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
