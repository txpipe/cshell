use clap::Parser;
use miette::{Context, IntoDiagnostic};

use crate::utils::{Config, OutputFormatter};

use super::{config::Wallet, dal::types::TransactionInfo};

#[derive(Parser)]
pub struct Args {
    /// Name of the wallet to query
    wallet: String,
}

pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let wallet = Wallet::load_from_raw_name_or_bail(&ctx.dirs, args.wallet).await?;

    let wallet_db = super::dal::WalletDB::open(&wallet.name, &wallet.dir_path(&ctx.dirs))
        .await
        .into_diagnostic()
        .context("Opening wallet for displaying utxos")?;

    let mut paginator = wallet_db.paginate_tx_history(sea_orm::Order::Asc, None);
    let num_pages = paginator.num_pages().await.into_diagnostic()?;

    while let Some(page) = paginator.fetch_and_next().await.into_diagnostic()? {
        let tx_infos: Vec<TransactionInfo> = page.into_iter().map(|model| model.into()).collect();
        tx_infos.output(&ctx.output_format);

        if paginator.cur_page() >= num_pages || {
            !inquire::Confirm::new("Get next page?")
                .with_default(true)
                .prompt()
                .into_diagnostic()?
        } {
            break;
        }
    }
    Ok(())
}
