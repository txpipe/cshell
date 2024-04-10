use clap::{Parser, Subcommand};
use tracing::instrument;

pub mod dump;
pub mod follow_tip;
pub mod get_block;
pub mod utils;

#[derive(Parser)]
pub struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Dump chain history
    DumpHistory(dump::Args),
    /// Get a specific block
    GetBlock(get_block::Args),
    /// Follow the chain's tip from a list of possible intersections
    FollowTip(follow_tip::Args),
}

#[instrument("chain", skip_all)]
pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    match args.command {
        Commands::DumpHistory(args) => dump::run(args, &ctx).await,
        Commands::GetBlock(args) => get_block::run(args, &ctx).await,
        Commands::FollowTip(args) => follow_tip::run(args, &ctx).await,
    }
}
