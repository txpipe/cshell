use clap::Parser;
use inquire::MultiSelect;
use miette::{bail, Context, IntoDiagnostic};
use serde_json::json;
use tracing::instrument;

use crate::output::OutputFormat;

#[derive(Parser, Clone)]
pub struct Args {
    /// Transaction cbor
    cbor: String,

    /// Allow sign using unsafe wallets
    #[arg(long, help = "Allow unsafe wallet signatures")]
    r#unsafe: bool,

    #[arg(long, help = "Wallets that will sign the transaction")]
    signer: Vec<String>,
}

#[instrument("sign", skip_all)]
pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let mut cbor = hex::decode(args.cbor)
        .into_diagnostic()
        .context("invalid cbor")?;

    let signers = if args.signer.is_empty() {
        let wallet_names: Vec<String> = ctx
            .store
            .wallets()
            .iter()
            .map(|wallet| wallet.name.to_string())
            .collect();

        MultiSelect::new(
            "What wallet should be used to sign the transaction?",
            wallet_names,
        )
        .prompt()
        .unwrap_or_default()
    } else {
        args.signer.clone()
    };

    let wallets = signers
        .iter()
        .map(|signer| {
            let wallet = ctx
                .store
                .wallets()
                .iter()
                .find(|wallet| wallet.name.to_string().eq(signer));

            let Some(wallet) = wallet else {
                bail!("invalid signer wallet '{signer}'")
            };

            if wallet.is_unsafe && !args.r#unsafe {
                bail!(
                    "wallet '{signer}' is unsafe, use the param --unsafe to allow unsafe signatures"
                )
            }

            Ok(wallet)
        })
        .collect::<Result<Vec<_>, _>>()?;

    for wallet in wallets {
        let password = match wallet.is_unsafe {
            true => None,
            false => Some(
                inquire::Password::new("Password:")
                    .with_help_message(&format!(
                        "The spending password for '{}' wallet:",
                        wallet.name
                    ))
                    .with_display_mode(inquire::PasswordDisplayMode::Masked)
                    .prompt()
                    .into_diagnostic()?,
            ),
        };

        cbor = wallet.sign(cbor, &password)?;
    }

    match ctx.output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "cbor": hex::encode(&cbor),
                }))
                .unwrap()
            );
        }
        OutputFormat::Table => println!("{}", hex::encode(&cbor)),
    }

    Ok(())
}
