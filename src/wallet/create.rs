use clap::Parser;
use miette::{bail, IntoDiagnostic};
use tracing::instrument;

use crate::{output::OutputFormatter, utils::Name};

use super::types::Wallet;

#[derive(Parser, Clone)]
pub struct Args {
    /// name to identify the wallet
    /// (leave blank to enter in interactive mode)
    pub name: Option<String>,

    /// spending password used to encrypt the private keys
    /// (leave blank to enter in interactive mode)
    password: Option<String>,
}

#[instrument("create", skip_all)]
pub async fn run(args: Args, ctx: &mut crate::Context) -> miette::Result<()> {
    let raw_name = match args.name {
        Some(name) => name,
        None => inquire::Text::new("Name of the wallet:")
            .prompt()
            .into_diagnostic()?,
    };
    let name = Name::try_from(raw_name)?;

    if ctx.store.wallets().iter().any(|wallet| wallet.name == name) {
        bail!(
            "Wallet with the same or conflicting name '{}' already exists.",
            name
        )
    }

    let password = match args.password {
        Some(password) => password,
        None => inquire::Password::new("Password:")
            .with_help_message("The spending password of your wallet")
            .with_display_mode(inquire::PasswordDisplayMode::Masked)
            .prompt()
            .into_diagnostic()?,
    };

    let wallet = Wallet::try_from(&name, &password, ctx.store.default_wallet().is_none())?;

    ctx.store.add_wallet(&wallet)?;

    // Log, print, and finish
    println!("Wallet created.");
    wallet.output(&ctx.output_format);
    Ok(())
}
