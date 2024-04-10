use tracing::instrument;

use crate::utils::{Config, OutputFormatter};

use super::config::Utxorpc;

#[instrument("list", skip_all)]
pub async fn run(ctx: &crate::Context) -> miette::Result<()> {
    let cfgs = Utxorpc::get_existing(&ctx.dirs).await?;
    cfgs.output(&ctx.output_format);
    Ok(())
}
