use clap::{Parser, Subcommand};
use tracing::instrument;

pub mod config;
mod create;
mod delete;
mod edit;
mod info;
mod list;

#[derive(Parser)]
pub struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new UTxO RPC configuration
    Create(create::Args),
    /// Get info about a UTxO configuration
    Info(info::Args),
    /// List UTxO RPC configurations
    List,
    /// Update an existing UTxO RPC configuration
    Edit(edit::Args),
    /// Delete a UTxO RPC configuration
    Delete(delete::Args),
}

#[instrument("utxorpc", skip_all)]
pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    match args.command {
        Commands::Create(args) => create::run(args, ctx).await,
        Commands::Info(args) => info::run(args, ctx).await,
        Commands::List => list::run(ctx).await,
        Commands::Edit(args) => edit::run(args, ctx).await,
        Commands::Delete(args) => delete::run(args, ctx).await,
    }
}
