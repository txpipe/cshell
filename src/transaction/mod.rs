use clap::{Parser, Subcommand};
use tracing::instrument;

#[derive(Parser)]
pub struct Args {}

#[derive(Subcommand)]
enum Commands {}

#[instrument("transaction", skip_all)]
pub async fn run(_args: Args, _ctx: &crate::Context) -> miette::Result<()> {
    unimplemented!("Not implemented yet. Sorry!")
}
