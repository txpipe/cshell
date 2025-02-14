use clap::Parser;
use miette::Result;
use tracing::instrument;

#[derive(Parser)]
pub struct Args {
    /// Name of the wallet to update. If undefined will use default.
    name: String,
}

#[instrument(skip_all, name = "edit")]
pub async fn run(_args: Args, _ctx: &crate::Context) -> Result<()> {
    todo!()
}
