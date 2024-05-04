use clap::{Parser, Subcommand};
use tracing::instrument;

pub mod config;
mod create;
mod dal;
mod delete;
mod info;
mod list;
mod update;
mod utxos;

#[derive(Parser)]
pub struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new wallet. Leave arguments blank for interactive mode
    Create(create::Args),
    /// Show wallet info
    Info(info::Args),
    /// List available wallets
    List,
    /// Update wallet state from the chain
    Update(update::Args),
    /// Delete a wallet. Caution!! This cannot be undone.
    Delete(delete::Args),
    // /// show wallet history
    // History(history::Args),
    /// List current utxos of a wallet
    Utxos(utxos::Args),
    // /// select current utxos of a wallet
    // Select(select::Args),
    // /// show wallet balance
    // Balance(balance::Args),
}

#[instrument("wallet", skip_all)]
pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    match args.command {
        Commands::Create(args) => create::run(args, ctx).await,
        Commands::Info(args) => info::run(args, ctx).await,
        // Commands::Address(args) => address::run(args, ctx).await,
        Commands::List => list::run(ctx).await,
        Commands::Update(args) => {
            ctx.with_tracing();
            update::run(args, ctx).await
        }
        Commands::Delete(args) => delete::run(args, ctx).await,
        // Commands::History(args) => history::run(args).await,
        Commands::Utxos(args) => utxos::run(args, ctx).await,
        // Commands::Select(args) => select::run(args, ctx).await,
        // Commands::Balance(args) => balance::run(args, ctx).await,
    }
}
