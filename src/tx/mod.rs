use clap::{Parser, Subcommand};
use tracing::instrument;

mod common;

mod invoke;
mod resolve;
mod sign;
mod submit;

#[derive(Parser)]
pub struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Invoke a tx3 transaction (resolve, sign and submit)
    Invoke(invoke::Args),

    /// Resolve a tx3 transaction
    Resolve(resolve::Args),

    /// Sign a CBOR transaction
    Sign(sign::Args),

    /// Submit a CBOR transaction
    Submit(submit::Args),
}

#[instrument("transaction", skip_all)]
pub async fn run(args: Args, ctx: &crate::Context) -> anyhow::Result<()> {
    match args.command {
        Commands::Invoke(args) => invoke::run(args, ctx).await,
        Commands::Resolve(args) => resolve::run(args, ctx).await,
        Commands::Sign(args) => sign::run(args, ctx).await,
        Commands::Submit(args) => submit::run(args, ctx).await,
    }
}
