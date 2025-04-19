use chrono::Local;
use clap::Parser;
use inquire::list_option::ListOption;
use miette::{bail, Context, IntoDiagnostic, Result};
use pallas::crypto::key::ed25519::PublicKey;
use std::str::FromStr;
use tracing::instrument;

use crate::{output::OutputFormatter, utils::Name, wallet::types::Wallet};

#[derive(Parser)]
pub struct Args {
    /// Name of the wallet to update. If undefined will use default.
    name: Option<String>,

    // Public Key
    public_key: Option<String>,

    /// Whether to set as default wallet.
    #[arg(long)]
    is_default: Option<bool>,
}

#[instrument(skip_all, name = "edit")]
pub async fn run(args: Args, ctx: &mut crate::Context) -> Result<()> {
    let name = match args.name {
        Some(name) => Name::try_from(name)?,
        None => {
            let name = inquire::Text::new("Name: ").prompt().into_diagnostic()?;
            Name::try_from(name)?
        }
    };

    if ctx.store.find_wallet(&name).is_some() {
        bail!("Wallet with that name already exists.")
    }

    let new_is_default = match args.is_default {
        Some(x) => x,
        None => match inquire::Select::new(
            "Set as default?",
            vec![ListOption::new(0, "yes"), ListOption::new(1, "no")],
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

    let public_key = match args.public_key {
        Some(public_key) => public_key,
        None => inquire::Text::new("Public key: ")
            .prompt()
            .into_diagnostic()?,
    };
    let public_key = PublicKey::from_str(&public_key)
        .into_diagnostic()
        .context("invalid public key")?;

    let wallet = Wallet {
        created: Local::now(),
        encrypted_private_key: None,
        name,
        modified: Local::now(),
        public_key: public_key.as_ref().to_vec(),
        is_default: new_is_default,
    };

    ctx.store.add_wallet(&wallet)?;

    // Log, print, and finish
    wallet.output(&ctx.output_format);
    Ok(())
}
