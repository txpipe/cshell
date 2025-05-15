use clap::{command, Parser, Subcommand};
use tracing::instrument;

mod block;

#[derive(Parser)]
pub struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// fetch block
    Block(block::Args),
}

#[instrument("search", skip_all)]
pub async fn run(args: Args, ctx: &mut crate::Context) -> miette::Result<()> {
    match args.command {
        Commands::Block(args) => block::run(args, ctx).await,
    }
}
