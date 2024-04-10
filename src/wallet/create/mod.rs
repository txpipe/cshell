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

use crate::wallet;
use crate::wallet::config;

#[derive(Parser, Clone)]
pub struct Args {
    /// name to identify the wallet
    /// (leave blank to enter in interactive mode)
    pub name: Option<String>,

    /// spending password used to encrypt the private keys
    /// (leave blank to enter in interactive mode)
    password: Option<String>,

    /// name of the chain to attach the wallet
    // TODO
    #[arg(short, long, env = "CSHELL_DEFAULT_CHAIN")]
    pub chain: Option<String>,
}

struct UserSelections {
    name: String,
    password: String,
    chain: Option<String>,
}

#[instrument("create", skip_all)]
pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let selections = gather_inputs(args)?;

    let wallet_slug = slug::slugify(&selections.name);
    let wallet_path = config::Wallet::wallet_dir(&ctx.dirs.root_dir, &wallet_slug);

    if wallet_path.exists() {
        bail!("Wallet {} already exists.", wallet_slug)
    }

    // TODO
    // if selections.chain.is_some()
    //     && !chain::config::Chain::dir(&ctx.dirs.root_dir, selections.chain.as_ref().unwrap()).exists()
    // {
    //     bail!("chain doesn't exist")
    // }

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
        ShelleyDelegationPart::Null, // TODO ?
    );

    let testnet_address = ShelleyAddress::new(
        Network::Testnet,
        ShelleyPaymentPart::key_hash(pkh.into()),
        ShelleyDelegationPart::Null,
    );

    let addresses = wallet::config::Addresses {
        mainnet: mainnet_address.to_bech32().into_diagnostic()?, // TODO: Should be mainnet, pre-prod, and preview?
        testnet: testnet_address.to_bech32().into_diagnostic()?,
    };

    // TODO
    // let db = wallet::dal::WalletDB::open(&args.name, &wallet_path)
    // .await
    // .into_diagnostic()?;
    // db.migrate_up().await.into_diagnostic()?;

    // Save wallet config
    let wallet = wallet::config::Wallet::new(
        String::from(&wallet_slug),
        key_data,
        addresses,
        selections.chain,
    );
    wallet.save_config(&ctx.dirs.root_dir)?;

    info!(wallet = wallet.name, "created");
    println!(
        "Wallet created at {}",
        wallet::config::Wallet::wallet_dir(&ctx.dirs.root_dir, &wallet_slug).display()
    );
    Ok(())
}

fn gather_inputs(args: Args) -> miette::Result<UserSelections> {
    let name = match args.name {
        Some(name) => name,
        None => inquire::Text::new("Name of the wallet:")
            .prompt()
            .into_diagnostic()?,
    };

    let password = match args.password {
        Some(password) => password,
        None => inquire::Password::new("password:")
            .with_help_message("the spending password of your wallet")
            .with_display_mode(inquire::PasswordDisplayMode::Masked)
            .prompt()
            .into_diagnostic()?,
    };

    Ok(UserSelections {
        name,
        password,
        chain: args.chain,
    })
}
