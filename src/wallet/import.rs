use anyhow::{bail, Context, Result};
use chrono::Local;
use clap::Parser;
use inquire::list_option::ListOption;
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

#[instrument(skip_all, name = "import")]
pub async fn run(args: Args, ctx: &mut crate::Context) -> Result<()> {
    let name = match args.name {
        Some(name) => Name::try_from(name)?,
        None => {
            let name = inquire::Text::new("Name: ")
                .prompt()
                .map_err(anyhow::Error::msg)?;
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
        .map_err(anyhow::Error::msg)?
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
            .map_err(anyhow::Error::msg)?,
    };
    let public_key = PublicKey::from_str(&public_key).context("invalid public key")?;

    let wallet = Wallet {
        created: Local::now(),
        private_key: None,
        name,
        modified: Local::now(),
        public_key: public_key.as_ref().to_vec(),
        stake_public_key: None,
        is_default: new_is_default,
        is_unsafe: false,
    };

    ctx.store.add_wallet(&wallet)?;

    // Log, print, and finish
    wallet.output(&ctx.output_format);
    Ok(())
}
