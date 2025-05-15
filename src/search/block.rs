use clap::Parser;
use miette::Result;
use tracing::instrument;

#[derive(Parser)]
pub struct Args {
    /// block hash
    hash: String,
}

#[instrument(skip_all, name = "block")]
pub async fn run(_args: Args, _ctx: &mut crate::Context) -> Result<()> {
    Ok(())
}
