use clap::{command, Parser, Subcommand};
use miette::{Context, IntoDiagnostic};
use utxorpc::spec::sync::BlockRef;

use crate::utils::{Config, OutputFormatter};

use super::{
    config::Wallet,
    dal::{
        types::{self, TransactionInfo},
        WalletDB,
    },
};

#[derive(Parser)]
pub struct Args {
    #[command(subcommand)]
    command: Commands,

    /// Name of the wallet to show history for
    #[arg(env = "CSHELL_WALLET")]
    wallet: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Show blocks the wallet has been involved in
    Blocks,
    /// Show transactions the wallet has been involved in
    #[command(alias = "txs")]
    Transactions,
    /// Show UTxOs
    Utxos,
}

pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let wallet = Wallet::load_from_raw_name_or_bail(&ctx.dirs, args.wallet).await?;

    let wallet_db = super::dal::WalletDB::open(&wallet.name, &wallet.dir_path(&ctx.dirs))
        .await
        .into_diagnostic()
        .context("Opening wallet for displaying utxos")?;

    match args.command {
        Commands::Utxos => utxos(&wallet_db, ctx).await,
        Commands::Transactions => transactions(&wallet_db, ctx).await,
        Commands::Blocks => blocks(&wallet_db, ctx).await,
    }
}

pub async fn blocks(wallet_db: &WalletDB, ctx: &crate::Context) -> miette::Result<()> {
    let mut paginator = wallet_db
        .paginate_block_history(sea_orm::Order::Asc, None)
        .await;

    let num_pages = paginator.num_pages().await.into_diagnostic()?;

    while let Some(page) = paginator.fetch_and_next().await.into_diagnostic()? {
        let blocks: Vec<BlockRef> = page.into_iter().map(types::block_ref_from_model).collect();
        blocks.output(&ctx.output_format);

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

pub async fn transactions(wallet_db: &WalletDB, ctx: &crate::Context) -> miette::Result<()> {
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

pub async fn utxos(wallet_db: &WalletDB, ctx: &crate::Context) -> miette::Result<()> {
    let utxos = wallet_db
        .fetch_all_utxos(sea_orm::Order::Asc)
        .await
        .into_diagnostic()
        .context("Fetching utxos from DB")?;

    utxos.output(&ctx.output_format);
    Ok(())
}
