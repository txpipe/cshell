use clap::{Parser, Subcommand};
use tracing::instrument;

mod balance;
pub mod config;
mod create;
mod dal;
mod delete;
mod edit;
mod history;
mod info;
mod list;
mod sync;

#[derive(Parser)]
pub struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new wallet. Leave arguments blank for interactive mode
    Create(create::Args),
    /// Edit an existing wallet
    Edit(edit::Args),
    /// Show wallet info
    Info(info::Args),
    /// List available wallets
    List,
    /// Update wallet state from the chain
    Sync(sync::Args),
    /// Delete a wallet. Caution!! This cannot be undone.
    Delete(delete::Args),
    /// Show info about wallet history
    History(history::Args),
    /// show wallet balance
    Balance(balance::Args),
}

#[instrument("wallet", skip_all)]
pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    ctx.with_tracing();

    match args.command {
        Commands::Create(args) => create::run(args, ctx).await,
        Commands::Edit(args) => edit::run(args, ctx).await,
        Commands::Info(args) => info::run(args, ctx).await,
        Commands::List => list::run(ctx).await,
        Commands::Sync(args) => sync::run(args, ctx).await,
        Commands::Delete(args) => delete::run(args, ctx).await,
        Commands::History(args) => history::run(args, ctx).await,
        Commands::Balance(args) => balance::run(args, ctx).await,
    }
}
