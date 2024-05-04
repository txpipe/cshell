use clap::Parser;
use miette::{Context, IntoDiagnostic};

use crate::utils::{Config, OutputFormatter};

use super::{
    config::Wallet,
    dal::{types::TxoInfo, WalletDB},
};

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

    let utxos = utxos_for_wallet(&wallet_db).await?;

    utxos.output(&ctx.output_format);
    Ok(())
}

pub async fn utxos_for_wallet(wallet_db: &WalletDB) -> miette::Result<Vec<TxoInfo>> {
    wallet_db
        .fetch_all_utxos(sea_orm::Order::Asc)
        .await
        .into_diagnostic()
        .context("Fetching utxos from DB")
}
