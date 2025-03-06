use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use clap::Parser;
use miette::{bail, Context, IntoDiagnostic};
use tracing::instrument;

use crate::{output::OutputFormatter, utils::Name};

use super::{types::Provider, utxorpc::UTxORPCProvider};

#[derive(clap::ValueEnum, Clone)]
enum ProviderKind {
    Utxorpc,
}

#[derive(clap::ValueEnum, Clone, PartialEq)]
enum NetworkKind {
    Mainnet,
    Testnet,
}

#[derive(Serialize, Deserialize)]
struct UTxORPCParameters {
    url: String,
    headers: HashMap<String, String>,
}

#[derive(Parser, Clone)]
pub struct Args {
    /// Name to identify the provider.
    #[arg(long)]
    name: Option<String>,

    /// Provider kind.
    #[arg(long)]
    kind: Option<ProviderKind>,

    /// Whether to set as default provider.
    #[arg(long)]
    is_default: Option<bool>,

    /// Whether it is mainnet or testnet.
    #[arg(long)]
    network_kind: Option<NetworkKind>,

    /// JSON encoded parameters particular to the provider type.
    #[arg(long)]
    parameters: Option<String>,
}

#[instrument("create", skip_all)]
pub async fn run(args: Args, ctx: &mut crate::Context) -> miette::Result<()> {
    let raw_name = match args.name {
        Some(name) => name,
        None => inquire::Text::new("Name of the provider:")
            .prompt()
            .into_diagnostic()?,
    };
    let name = Name::try_from(raw_name)?;

    if ctx
        .store
        .providers()
        .iter()
        .any(|provider| *provider.name() == *name)
    {
        bail!(
            "Provider with the same or conflicting name '{}' already exists.",
            name
        )
    }

    let kind = match args.kind {
        Some(kind) => kind,
        None => match inquire::Select::new("Kind of provider:", vec!["UTxORPC"])
            .prompt()
            .into_diagnostic()?
        {
            "UTxORPC" => ProviderKind::Utxorpc,
            _ => bail!("Invalid kind."),
        },
    };

    let newtork_kind = match args.network_kind {
        Some(network_kind) => network_kind,
        None => match inquire::Select::new("Network kind:", vec!["mainnet", "testnet"])
            .prompt()
            .into_diagnostic()?
        {
            "mainnet" => NetworkKind::Mainnet,
            "testnet" => NetworkKind::Testnet,
            _ => bail!("Invalid network kind"),
        },
    };
    let is_testnet = newtork_kind == NetworkKind::Testnet;

    // Provider specific inquires.
    let provider = match kind {
        ProviderKind::Utxorpc => {
            let (url, headers) = match args.parameters {
                Some(parameters) => {
                    let parameters: UTxORPCParameters = serde_json::from_str(&parameters)
                        .into_diagnostic()
                        .context("Invalid parameters")?;
                    (parameters.url, parameters.headers)
                }
                None => {
                    let url = inquire::Text::new("URL:").prompt().into_diagnostic()?;
                    let headers: HashMap<String, String> = inquire::Text::new(
                "Add request headers? Example: 'dmtr-api-key:dmtr_jdndajs,other:other-value'",
            )
            .prompt()
            .into_diagnostic()?
            .split(",")
            .flat_map(|keyval| {
                if keyval.is_empty() {
                    return None
                }
                let mut parts = keyval.split(":");
                let key = match parts.next() {
                    Some(s) => s,
                    None => return Some(Err(miette::Error::msg("Invalid header"))),
                };
                let val = match parts.next() {
                    Some(s) => s,
                    None => return  Some(Err(miette::Error::msg("Invalid header"))),
                };
                Some( Ok((key.to_string(), val.to_string())) )
            })
            .collect::<Result<_, miette::Error>>()?;
                    (url, headers)
                }
            };
            Provider::UTxORPC(UTxORPCProvider {
                name,
                is_default: Some(ctx.store.providers().is_empty()),
                is_testnet: Some(is_testnet),
                url,
                headers: if headers.is_empty() {
                    None
                } else {
                    Some(headers)
                },
            })
        }
    };

    ctx.store.add_provider(&provider)?;

    // Log, print, and finish
    provider.output(&ctx.output_format);
    Ok(())
}
