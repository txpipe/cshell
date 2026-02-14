use anyhow::bail;
use clap::Parser;
use comfy_table::Table;
use serde_json::json;
use utxorpc::spec::query::{any_utxo_data::ParsedState, AnyUtxoData};

use crate::output::{OutputFormat, OutputFormatter};

#[derive(Parser)]
pub struct Args {
    /// Name of the wallet to show the UTxOs of. If undefined, the default wallet is used.
    name: Option<String>,

    /// Name of the provider to use. If undefined, the default provider is used.
    provider: Option<String>,
}

pub async fn run(args: Args, ctx: &crate::Context) -> anyhow::Result<()> {
    let wallet = match args.name {
        Some(name) => ctx.store.find_wallet(&name),
        None => ctx.store.default_wallet(),
    };

    let provider = match args.provider {
        Some(name) => ctx.store.find_provider(&name),
        None => ctx.store.default_provider(),
    };

    match (wallet, provider) {
        (Some(wallet), Some(provider)) => {
            let address = wallet.address(provider.is_testnet());
            let utxos = provider.get_wallet_utxos(&address).await?;
            let output = WalletUtxoOutput::new(utxos);

            let format = if ctx.output_format_overridden {
                ctx.output_format.clone()
            } else {
                OutputFormat::Json
            };

            output.output(&format);

            Ok(())
        }
        (None, Some(_)) => bail!("Wallet not found."),
        (Some(_), None) => bail!("Provider not found."),
        (None, None) => bail!("Wallet and provider not found."),
    }
}

struct WalletUtxoOutput {
    utxos: Vec<AnyUtxoData>,
}

impl WalletUtxoOutput {
    fn new(utxos: Vec<AnyUtxoData>) -> Self {
        Self { utxos }
    }
}

impl OutputFormatter for WalletUtxoOutput {
    fn to_table(&self) {
        let mut table = Table::new();

        table.set_header(vec!["Tx Hash", "Index", "Lovelace", "Assets", "Datum Hash"]);

        for utxo in &self.utxos {
            let (tx_hash, index) = utxo
                .txo_ref
                .as_ref()
                .map(|reference| (hex::encode(&reference.hash), reference.index.to_string()))
                .unwrap_or_else(|| ("-".to_string(), "-".to_string()));

            let (coin, asset_count, datum_hash) = match &utxo.parsed_state {
                Some(ParsedState::Cardano(output)) => {
                    let asset_count: usize = output
                        .assets
                        .iter()
                        .map(|multiasset| multiasset.assets.len())
                        .sum();

                    let datum_hash = output
                        .datum
                        .as_ref()
                        .map(|datum| hex::encode(&datum.hash))
                        .unwrap_or_else(|| "-".to_string());

                    (
                        crate::utils::format_bigint_opt(&output.coin),
                        asset_count.to_string(),
                        datum_hash,
                    )
                }
                None => ("-".to_string(), "0".to_string(), "-".to_string()),
            };

            table.add_row(vec![tx_hash, index, coin, asset_count, datum_hash]);
        }

        println!("{table}");
    }

    fn to_json(&self) {
        let payload = json!({ "utxos": self.utxos });
        println!("{}", serde_json::to_string_pretty(&payload).unwrap());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_without_arguments() {
        let args = Args::parse_from(["wallet-utxos"]);

        assert!(args.name.is_none());
        assert!(args.provider.is_none());
    }

    #[test]
    fn parses_with_wallet_and_provider() {
        let args = Args::parse_from(["wallet-utxos", "alice", "mainnet"]);

        assert_eq!(args.name.as_deref(), Some("alice"));
        assert_eq!(args.provider.as_deref(), Some("mainnet"));
    }
}
