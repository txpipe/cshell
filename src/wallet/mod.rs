use clap::{Parser, Subcommand};
use tracing::instrument;

mod balance;
mod block_history;
pub mod config;
mod create;
mod dal;
mod delete;
mod info;
mod list;
mod transactions;
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
    /// Show blocks this wallet has been involved in
    BlockHistory(block_history::Args),
    /// Show transactions this wallet has been involved in
    #[command(alias = "txs")]
    TxHistory(transactions::Args),
    /// List current utxos of a wallet
    Utxos(utxos::Args),
    /// show wallet balance
    Balance(balance::Args),
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
        Commands::BlockHistory(args) => block_history::run(args, ctx).await,
        Commands::TxHistory(args) => transactions::run(args, ctx).await,
        Commands::Utxos(args) => utxos::run(args, ctx).await,
        Commands::Balance(args) => balance::run(args, ctx).await,
    }
}
