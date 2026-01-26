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
    // 1. Check if c2rust-config exists
    config_helper::check_c2rust_config_exists()?;

    // 2. Execute the build command
    executor::execute_command(&args.dir, &args.command)?;

    // 3. Save configuration using c2rust-config
    let command_str = args.command.join(" ");
    config_helper::save_config(&args.dir, &command_str, args.feature.as_deref())?;

    println!("Build command executed successfully and configuration saved.");
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
