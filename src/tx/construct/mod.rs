use clap::{Parser, Subcommand};
use tracing::instrument;

mod wizard;
mod common;
mod add_input;
mod add_output;
mod build;

#[derive(Parser)]
pub struct Args {
    #[command(subcommand)] 
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a tx3 transaction using a wizard
    Wizard(wizard::Args),

    /// Add input to transaction
    #[command(name = "add-input")]
    AddInput(add_input::Args),

    /// Add output to transaction
    #[command(name = "add-output")]
    AddOutput(add_output::Args),

    /// Build the transaction
    Build(build::Args),
}

#[instrument("construct", skip_all)]
pub async fn run(args: Args, ctx: &crate::Context) -> anyhow::Result<()> {
    match args.command {
        Commands::Wizard(args) => wizard::run(args, ctx).await,
        Commands::AddInput(args) => add_input::run(args, ctx).await,
        Commands::AddOutput(args) => add_output::run(args, ctx).await,
        Commands::Build(args) => build::run(args, ctx).await,
    }
}
