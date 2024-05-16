use clap::Parser;
use miette::{Context, IntoDiagnostic};
use num_bigint::BigUint;

use crate::utils::Config;

use super::{
    config::Wallet,
    dal::{types::TxoInfo, WalletDB},
};

#[derive(Parser)]
pub struct Args {
    /// Name of the wallet to show the balance of
    #[arg(env = "CSHELL_WALLET")]
    wallet: String,
}

pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let wallet = Wallet::load_from_raw_name_or_bail(&ctx.dirs, args.wallet).await?;

    let wallet_db = super::dal::WalletDB::open(&wallet.name, &wallet.dir_path(&ctx.dirs))
        .await
        .into_diagnostic()
        .context("Opening wallet for displaying utxos")?;

    let balance = get_balance(&wallet_db).await?;
    println!("{balance}");
    Ok(())
}

pub async fn get_balance(wallet_db: &WalletDB) -> miette::Result<BigUint> {
    let mut balance = BigUint::ZERO;

    let mut paginator = wallet_db.paginate_utxos(sea_orm::Order::Asc, Some(100));

    while let Some(page) = paginator.fetch_and_next().await.into_diagnostic()? {
        page.into_iter()
            .map(|model| model.into())
            .for_each(|utxo: TxoInfo| balance += utxo.coin);
    }

    Ok(balance)
}
