use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use anyhow::{bail, Result};
use clap::Parser;
use tracing::instrument;

use crate::{output::OutputFormatter, provider::types::Provider, utils::Name};

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

    /// Whether to set as default provider.
    #[arg(long)]
    is_default: Option<bool>,

    /// Whether it is mainnet or testnet.
    #[arg(long)]
    network_kind: Option<NetworkKind>,

    // UTxORPC url
    #[arg(long)]
    utxorpc_url: Option<String>,

    /// JSON encoded UTxORPC headers
    #[arg(long)]
    utxorpc_headers: Option<String>,

    // TRP url
    #[arg(long)]
    trp_url: Option<String>,

    /// JSON encoded TRP headers
    #[arg(long)]
    trp_headers: Option<String>,
}

#[instrument("create", skip_all)]
pub async fn run(args: Args, ctx: &mut crate::Context) -> anyhow::Result<()> {
    let raw_name = match args.name {
        Some(name) => name,
        None => inquire::Text::new("Name of the provider:")
            .prompt()
            .map_err(anyhow::Error::msg)?,
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

    let newtork_kind = match args.network_kind {
        Some(network_kind) => network_kind,
        None => match inquire::Select::new("Network kind:", vec!["mainnet", "testnet"])
            .prompt()
            .map_err(anyhow::Error::msg)?
        {
            "mainnet" => NetworkKind::Mainnet,
            "testnet" => NetworkKind::Testnet,
            _ => bail!("Invalid network kind"),
        },
    };
    let is_testnet = newtork_kind == NetworkKind::Testnet;

    let url = match args.utxorpc_url {
        Some(url) => url,
        None => inquire::Text::new("URL:")
            .prompt()
            .map_err(anyhow::Error::msg)?,
    };
    let headers: HashMap<String, String> = match args.utxorpc_headers {
        Some(aux) => serde_json::from_str(&aux).map_err(anyhow::Error::msg)?,
        None => inquire::Text::new(
            "Add request headers? Example: 'dmtr-api-key:dmtr_jdndajs,other:other-value'",
        )
        .prompt()
        .map_err(anyhow::Error::msg)?
        .split(",")
        .flat_map(|keyval| {
            if keyval.is_empty() {
                return None;
            }
            let mut parts = keyval.split(":");
            let key = match parts.next() {
                Some(s) => s,
                None => return Some(Err(anyhow::Error::msg("Invalid header"))),
            };
            let val = match parts.next() {
                Some(s) => s,
                None => return Some(Err(anyhow::Error::msg("Invalid header"))),
            };
            Some(Ok((key.to_string(), val.to_string())))
        })
        .collect::<Result<_, anyhow::Error>>()?,
    };

    let trp_url = match args.trp_url {
        Some(url) => Some(url),
        None => {
            let response = inquire::Text::new("TRP URL (leave empty for undefined):")
                .prompt()
                .map_err(anyhow::Error::msg)?;
            if response.is_empty() {
                None
            } else {
                Some(response)
            }
        }
    };
    let mut trp_headers = None;
    if trp_url.is_some() {
        let aux: HashMap<String, String> = match args.trp_headers {
            Some(inner) => serde_json::from_str(&inner).map_err(anyhow::Error::msg)?,

            None => inquire::Text::new(
                "Add request headers? Example: 'dmtr-api-key:dmtr_jdndajs,other:other-value'",
            )
            .prompt()
            .map_err(anyhow::Error::msg)?
            .split(",")
            .flat_map(|keyval| {
                if keyval.is_empty() {
                    return None;
                }
                let mut parts = keyval.split(":");
                let key = match parts.next() {
                    Some(s) => s,
                    None => return Some(Err(anyhow::Error::msg("Invalid header"))),
                };
                let val = match parts.next() {
                    Some(s) => s,
                    None => return Some(Err(anyhow::Error::msg("Invalid header"))),
                };
                Some(Ok((key.to_string(), val.to_string())))
            })
            .collect::<Result<_, anyhow::Error>>()?,
        };

        if !aux.is_empty() {
            trp_headers = Some(aux);
        }
    }

    let provider = Provider {
        name,
        is_default: Some(ctx.store.providers().is_empty()),
        is_testnet: Some(is_testnet),
        url,
        headers: if headers.is_empty() {
            None
        } else {
            Some(headers)
        },
        trp_url,
        trp_headers,
    };

    ctx.store.add_provider(&provider)?;

    // Log, print, and finish
    provider.output(&ctx.output_format);
    Ok(())
}
