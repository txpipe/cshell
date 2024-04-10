use clap::Parser;
use miette::bail;
use tracing::{info, instrument};

use crate::{
    utils::{Config, ConfigName},
    utxorpc::config::Utxorpc,
    wallet::config::Wallet,
};

#[derive(Parser)]
pub struct Args {
    /// Name of the wallet that will have its history updated
    wallet: String,
}

#[instrument("update", skip_all, fields(wallet=args.wallet))]
pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let wallet_name = ConfigName::new(args.wallet)?;
    let wallet = match Wallet::load(&ctx.dirs, &wallet_name).await? {
        Some(wallet) => wallet,
        None => bail!(r#"No wallet named "{}" exists."#, &wallet_name.raw),
    };

    info!(
        wallet = &wallet.name.raw,
        utxorpc_config = &wallet.utxorpc_config.raw,
        "updating"
    );

    let utxo_cfg = Utxorpc::load(&ctx.dirs, &wallet.utxorpc_config).await?;
    let _utxo_cfg = match utxo_cfg {
        None => bail!(
            "The UTxO configuration for this wallet does not exist: {}",
            &wallet.utxorpc_config.raw
        ),
        Some(utxo_cfg) => utxo_cfg,
    };

    // TODO
    unimplemented!();
}
