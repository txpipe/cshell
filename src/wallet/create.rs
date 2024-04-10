use clap::Parser;
use miette::{bail, IntoDiagnostic};
use pallas::{
    crypto::key::ed25519::SecretKey,
    ledger::{
        addresses::{Network, ShelleyAddress, ShelleyDelegationPart, ShelleyPaymentPart},
        traverse::ComputeHash,
    },
    wallet::wrapper,
};
use rand::rngs::OsRng;
use tracing::{info, instrument};

use crate::{
    utils::{Config, ConfigName, OutputFormatter},
    wallet,
};

use crate::wallet::config::Wallet;

#[derive(Parser, Clone)]
pub struct Args {
    /// name to identify the wallet
    /// (leave blank to enter in interactive mode)
    pub name: Option<String>,

    /// spending password used to encrypt the private keys
    /// (leave blank to enter in interactive mode)
    password: Option<String>,

    /// name of the chain to attach the wallet
    #[arg(env = "CSHELL_DEFAULT_UTXORPC_CONFIG")]
    pub utxorpc_config: Option<String>,
}

struct UserSelections {
    name: ConfigName,
    password: String,
    utxorpc_config: String,
}

#[instrument("create", skip_all)]
pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let selections = gather_inputs(args, ctx).await?;

    // Make keys
    let priv_key = SecretKey::new(OsRng);
    let pkh = priv_key.public_key().compute_hash();
    let encrypted_priv_key =
        wrapper::encrypt_private_key(OsRng, priv_key.into(), &selections.password);
    let key_data = wallet::config::Keys {
        public_key_hash: hex::encode(pkh),
        private_encrypted: hex::encode(encrypted_priv_key),
    };

    let mainnet_address = ShelleyAddress::new(
        Network::Mainnet,
        ShelleyPaymentPart::key_hash(pkh.into()),
        ShelleyDelegationPart::Null, // TODO: Do we need a delegation part?
    );

    let testnet_address = ShelleyAddress::new(
        Network::Testnet,
        ShelleyPaymentPart::key_hash(pkh.into()),
        ShelleyDelegationPart::Null, // TODO: Do we need a delegation part?
    );

    let addresses = wallet::config::Addresses {
        mainnet: mainnet_address.to_bech32().into_diagnostic()?,
        testnet: testnet_address.to_bech32().into_diagnostic()?,
    };

    // TODO: Update SQLite with the new wallet info.
    // let db = wallet::dal::WalletDB::open(&args.name, &wallet_path)
    // .await
    // .into_diagnostic()?;
    // db.migrate_up().await.into_diagnostic()?;

    // Save wallet config
    let wallet = wallet::config::Wallet::new(
        selections.name,
        key_data,
        addresses,
        ConfigName::new(selections.utxorpc_config)?,
    )?;
    wallet.save(&ctx.dirs, false).await?;

    // Log, print, and finish
    info!(wallet = wallet.name().raw, "created");
    println!("Wallet created:");
    wallet.output(&ctx.output_format);
    Ok(())
}

async fn gather_inputs(args: Args, ctx: &crate::Context) -> miette::Result<UserSelections> {
    let raw_name = match args.name {
        Some(name) => name,
        None => inquire::Text::new("Name of the wallet:")
            .prompt()
            .into_diagnostic()?,
    };
    let name = ConfigName::new(raw_name)?;

    if let Some(conflict) = Wallet::find_match(&ctx.dirs, &name).await? {
        bail!(
            r#"Wallet with the same or conflicting name "{}" already exists. Both normalize to "{}"."#,
            &conflict.raw,
            &name.normalized()
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

    let utxorpc_config = match args.utxorpc_config {
        Some(cfg) => cfg,
        None => inquire::Text::new(
            "Name of the UTxO RPC Config to use with this wallet. \
            Note that this determines the chain this wallet will use.",
        )
        .prompt()
        .into_diagnostic()?,
    };

    Ok(UserSelections {
        name,
        password,
        utxorpc_config,
    })
}
