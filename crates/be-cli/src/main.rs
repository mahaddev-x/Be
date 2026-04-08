mod commands;
mod setup;
mod tui;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "be",
    version = "1.0.0",
    about = "Be — local-first parallel AI task runner",
    long_about = "Run hundreds of independent AI tasks in true parallel.\nEach task unit (Bee) is stateless, single-purpose, and returns validated JSON."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive first-run setup wizard
    Setup,

    /// Manage bee definitions
    #[command(subcommand)]
    Bee(commands::bee::BeeCommands),

    /// Run a swarm of bees
    Run(commands::run::RunArgs),

    /// Show results of a completed job
    Results(commands::results::ResultsArgs),

    /// List and manage jobs
    #[command(subcommand)]
    Jobs(commands::jobs::JobsCommands),

    /// Manage configuration
    #[command(subcommand)]
    Config(commands::config::ConfigCommands),

    /// Start MCP server on stdio
    Mcp,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Setup => setup::run().await,
        Commands::Bee(cmd) => commands::bee::run(cmd).await,
        Commands::Run(args) => commands::run::run(args).await,
        Commands::Results(args) => commands::results::run(args).await,
        Commands::Jobs(cmd) => commands::jobs::run(cmd).await,
        Commands::Config(cmd) => commands::config::run(cmd).await,
        Commands::Mcp => commands::mcp::run().await,
    }
}
