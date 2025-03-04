use chrono::Local;
use clap::Parser;
use miette::{bail, IntoDiagnostic, Result};
use tracing::instrument;

use crate::{
    output::OutputFormatter,
    utils::{show_is_current, Name},
    wallet::types::Wallet,
};

#[derive(Parser)]
pub struct Args {
    /// Name of the wallet to update. If undefined will use default.
    name: Option<String>,

    /// Whether to set as default wallet.
    is_default: Option<bool>,
}

#[instrument(skip_all, name = "edit")]
pub async fn run(args: Args, ctx: &mut crate::Context) -> Result<()> {
    let wallet = match args.name {
        Some(name) => ctx.store.find_wallet(&name),
        None => ctx.store.default_wallet(),
    };

    let Some(wallet) = wallet else {
        bail!("Wallet not found.")
    };

    let new_name = inquire::Text::new("New name: ")
        .with_default(&wallet.name)
        .prompt()
        .into_diagnostic()?;
    let new_name = Name::try_from(new_name)?;

    let new_is_default = match args.is_default {
        Some(x) => x,
        None => match inquire::Select::new(
            "Set as default?",
            vec![
                show_is_current("yes", wallet.is_default).as_str(),
                show_is_current("no", !wallet.is_default).as_str(),
            ],
        )
        .prompt()
        .into_diagnostic()?
        {
            "yes" => true,
            "yes (current)" => true,
            "no" => false,
            "no (current)" => false,
            _ => bail!("invalid response"),
        },
    };

    let new_wallet = Wallet {
        created: wallet.created,
        encrypted_private_key: wallet.encrypted_private_key.clone(),
        name: new_name,
        modified: Local::now(),
        public_key: wallet.public_key.clone(),
        is_default: new_is_default,
    };

    ctx.store.remove_wallet(wallet.clone())?;
    ctx.store.add_wallet(&new_wallet)?;

    // Log, print, and finish
    println!("Wallet modified.");
    new_wallet.output(&ctx.output_format);
    Ok(())
}
