use clap::{Parser, Subcommand};
use tracing::instrument;

mod balance;
mod create;
mod delete;
mod edit;
mod import;
mod info;
mod list;
mod restore;
pub mod types;

#[derive(Parser)]
pub struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new wallet. Leave arguments blank for interactive mode
    Create(create::Args),
    /// Restore wallet using BIP39 Mnemonic. Leave arguments blank for interactive mode
    Restore(restore::Args),
    /// Edit an existing wallet
    Edit(edit::Args),
    /// Import a wallet
    Import(import::Args),
    /// Show wallet info
    Info(info::Args),
    /// List available wallets
    List,
    /// Delete a wallet. Caution!! This cannot be undone.
    Delete(delete::Args),
    /// show wallet balance
    Balance(balance::Args),
}

#[instrument("wallet", skip_all)]
pub async fn run(args: Args, ctx: &mut crate::Context) -> miette::Result<()> {
    match args.command {
        Commands::Create(args) => create::run(args, ctx).await,
        Commands::Restore(args) => restore::run(args, ctx).await,
        Commands::Edit(args) => edit::run(args, ctx).await,
        Commands::Import(args) => import::run(args, ctx).await,
        Commands::Info(args) => info::run(args, ctx).await,
        Commands::List => list::run(ctx).await,
        Commands::Delete(args) => delete::run(args, ctx).await,
        Commands::Balance(args) => balance::run(args, ctx).await,
    }
}
