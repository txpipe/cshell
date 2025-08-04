use anyhow::bail;
use clap::Parser;
use tracing::instrument;

use crate::{output::OutputFormatter, utils::Name};

use super::types::Wallet;

#[derive(Parser, Clone)]
pub struct Args {
    /// name to identify the wallet
    /// (leave blank to enter in interactive mode)
    #[arg(long)]
    pub name: Option<String>,

    /// spending password used to encrypt the private keys
    /// (leave blank to enter in interactive mode)
    #[arg(long)]
    password: Option<String>,

    /// disable password requirement (not recommended)
    #[arg(long)]
    r#unsafe: bool,
}

#[instrument("create", skip_all)]
pub async fn run(args: Args, ctx: &mut crate::Context) -> anyhow::Result<()> {
    let raw_name = match args.name {
        Some(name) => name,
        None => inquire::Text::new("Name of the wallet:").prompt()?,
    };
    let name = Name::try_from(raw_name)?;

    if ctx.store.wallets().iter().any(|wallet| wallet.name == name) {
        bail!(
            "Wallet with the same or conflicting name '{}' already exists.",
            name
        )
    }

    let password = match args.r#unsafe {
        true => String::new(),
        false => match args.password {
            Some(password) => password,
            None => inquire::Password::new("Password:")
                .with_help_message("The spending password of your wallet")
                .with_display_mode(inquire::PasswordDisplayMode::Masked)
                .prompt()?,
        },
    };

    let new_wallet = Wallet::try_from(
        &name,
        &password,
        ctx.store.default_wallet().is_none(),
        args.r#unsafe,
    )?;

    ctx.store.add_wallet(&new_wallet.1)?;

    // Log, print, and finish
    new_wallet.output(&ctx.output_format);
    Ok(())
}
