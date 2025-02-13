use clap::{Parser, Subcommand};
use tracing::instrument;

mod create;
mod delete;
mod edit;
mod info;
mod list;
mod test;
pub mod types;
pub mod utxorpc;

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
    /// Delete a wallet. Caution!! This cannot be undone.
    Delete(delete::Args),
    /// Try connection.
    Test(test::Args),
}

#[instrument("wallet", skip_all)]
pub async fn run(args: Args, ctx: &mut crate::Context) -> miette::Result<()> {
    ctx.with_tracing();

    match args.command {
        Commands::Create(args) => create::run(args, ctx).await,
        Commands::Edit(args) => edit::run(args, ctx).await,
        Commands::Info(args) => info::run(args, ctx).await,
        Commands::List => list::run(ctx).await,
        Commands::Delete(args) => delete::run(args, ctx).await,
        Commands::Test(args) => test::run(args, ctx).await,
    }
}
