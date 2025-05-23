use clap::{Parser, Subcommand};
use tracing::instrument;

mod new;
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
    /// Start a new full transaction, resolve, sign and submit
    New(new::Args),

    /// Resolve a transaction
    Resolve(resolve::Args),

    /// Sign a transaction cbor
    Sign(sign::Args),

    /// Submit a transaction cbor
    Submit(submit::Args),
}

#[instrument("transaction", skip_all)]
pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    match args.command {
        Commands::New(args) => new::run(args, ctx).await,
        Commands::Resolve(args) => resolve::run(args, ctx).await,
        Commands::Sign(args) => sign::run(args, ctx).await,
        Commands::Submit(args) => submit::run(args, ctx).await,
    }
}
