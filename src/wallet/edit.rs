use chrono::Local;
use clap::Parser;
use inquire::list_option::ListOption;
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

    /// New name for wallet.
    #[arg(long)]
    new_name: Option<String>,

    /// Whether to set as default wallet.
    #[arg(long)]
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

    let new_name = match args.new_name {
        Some(new_name) => Name::try_from(new_name)?,
        None => {
            let new_name = inquire::Text::new("New name: ")
                .with_default(&wallet.name)
                .prompt()
                .into_diagnostic()?;
            Name::try_from(new_name)?
        }
    };

    let new_is_default = match args.is_default {
        Some(x) => x,
        None => match inquire::Select::new(
            "Set as default?",
            vec![
                ListOption::new(0, show_is_current("yes", wallet.is_default).as_str()),
                ListOption::new(1, show_is_current("no", wallet.is_default).as_str()),
            ],
        )
        .prompt()
        .into_diagnostic()?
        .index
        {
            0 => true,
            1 => false,
            _ => bail!("invalid response"),
        },
    };

    let new_wallet = Wallet {
        created: wallet.created,
        private_key: wallet.private_key.clone(),
        name: new_name,
        modified: Local::now(),
        public_key: wallet.public_key.clone(),
        is_default: new_is_default,
        is_unsafe: wallet.is_unsafe,
    };

    ctx.store.remove_wallet(wallet.clone())?;
    ctx.store.add_wallet(&new_wallet)?;

    // Log, print, and finish
    new_wallet.output(&ctx.output_format);
    Ok(())
}
