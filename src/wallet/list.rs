use crate::output::OutputFormatter;
use tracing::instrument;

#[instrument("list", skip_all)]
pub async fn run(ctx: &crate::Context) -> miette::Result<()> {
    ctx.store.wallets().output(&ctx.output_format);
    Ok(())
}
