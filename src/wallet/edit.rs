use clap::Parser;
use miette::{bail, IntoDiagnostic, Result};
use tracing::{info, instrument};

use crate::{
    utils::{Config, ConfigName, OutputFormatter},
    wallet::config::Wallet,
};

#[derive(Parser)]
pub struct Args {
    /// Name of the wallet to update
    name: String,
    /// Name of the UTxO RPC config
    #[arg(short, long, alias = "u5c")]
    utxorpc: Option<String>,
}

#[instrument(skip_all, name = "edit")]
pub async fn run(args: Args, ctx: &crate::Context) -> Result<()> {
    let wallet_name = ConfigName::new(args.name.clone())?;
    let wallet = Wallet::load(&ctx.dirs, &wallet_name).await?;
    match wallet {
        None => bail!(r#"No wallet named "{}" exists."#, &args.name),
        Some(mut wallet) => {
            if wallet.name.raw != args.name {
                let should_update = inquire::Confirm::new(&format!(
                    r#"A wallet with matching or conflicting name "{}" exists, do you want to update it? Both names normalize to "{}"."#,
                    &wallet.name.raw,
                    &wallet.name.normalized()
                ))
                .with_default(false)
                .prompt()
                .into_diagnostic()?;

                if !should_update {
                    return Ok(());
                }
            }

            if let Some(u5c) = args.utxorpc {
                wallet.utxorpc_config = ConfigName::new(u5c)?;
                wallet.save(&ctx.dirs, true).await?;
            }

            info!(
                r#"Updated the UTxO RPC config for "{}""#,
                &wallet.name().raw,
            );
            wallet.output(&ctx.output_format);
        }
    }
    Ok(())
}
