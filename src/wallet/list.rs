use tracing::instrument;

use crate::utils::{Config, OutputFormatter};

use super::config::Wallet;

#[instrument("list", skip_all)]
pub async fn run(ctx: &crate::Context) -> miette::Result<()> {
    let wallets = Wallet::get_all_existing(&ctx.dirs).await?;
    wallets.output(&ctx.output_format);
    Ok(())
}
