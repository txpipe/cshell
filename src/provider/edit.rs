use std::{collections::HashMap, fmt::Display};

use clap::Parser;
use miette::{bail, IntoDiagnostic};

use crate::{output::OutputFormatter, utils::Name};

use super::{types::Provider, utxorpc::UTxORPCProvider};

fn show_is_current(option: impl Display, is_current: bool) -> String {
    if is_current {
        format!("{} (current)", option)
    } else {
        format!("{}", option)
    }
}

#[derive(clap::ValueEnum, Clone, Copy)]
enum ProviderKind {
    Utxorpc,
}

impl Display for ProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ProviderKind::Utxorpc => "utxorpc",
            }
        )
    }
}

#[derive(clap::ValueEnum, Clone, PartialEq)]
enum NetworkKind {
    Mainnet,
    Testnet,
}

#[derive(Parser, Clone)]
pub struct Args {
    /// Name to identify the provider.
    name: Option<String>,

    /// Provider kind.
    kind: Option<ProviderKind>,

    /// Whether to set as default provider.
    is_default: Option<bool>,

    /// Whether it is mainnet or testnet.
    network_kind: Option<NetworkKind>,
}

// #[instrument("create", skip_all)]
pub async fn run(args: Args, ctx: &mut crate::Context) -> miette::Result<()> {
    let provider = match args.name {
        Some(name) => ctx.store.find_provider(&name),
        None => ctx.store.default_provider(),
    };

    let Some(provider) = provider else {
        bail!("Provider not found.")
    };

    let new_name = inquire::Text::new("New name: ")
        .with_default(&provider.name())
        .prompt()
        .into_diagnostic()?;
    let new_name = Name::try_from(new_name)?;

    let new_kind = match args.kind {
        Some(kind) => kind,
        None => match inquire::Select::new(
            "Kind of provider:",
            vec![show_is_current(
                ProviderKind::Utxorpc,
                ProviderKind::Utxorpc.to_string().to_lowercase() == provider.kind(),
            )
            .as_str()],
        )
        .prompt()
        .into_diagnostic()?
        {
            "utxorpc" => ProviderKind::Utxorpc,
            "utxorpc (current)" => ProviderKind::Utxorpc,
            _ => bail!("Invalid kind."),
        },
    };

    let new_is_default = match args.is_default {
        Some(x) => x,
        None => match inquire::Select::new(
            "Set as default?",
            vec![
                show_is_current("yes", provider.is_default()).as_str(),
                show_is_current("no", !provider.is_default()).as_str(),
            ],
        )
        .prompt()
        .into_diagnostic()?
        {
            "yes" => true,
            "yes (current)" => true,
            "no" => false,
            "no (current)" => false,
            _ => bail!("invalid response"),
        },
    };

    let new_newtork_kind = match args.network_kind {
        Some(network_kind) => network_kind,
        None => match inquire::Select::new(
            "Network kind:",
            vec![
                show_is_current("mainnet", !provider.is_testnet()).as_str(),
                show_is_current("testnet", provider.is_testnet()).as_str(),
            ],
        )
        .prompt()
        .into_diagnostic()?
        {
            "mainnet" => NetworkKind::Mainnet,
            "mainnet (current)" => NetworkKind::Mainnet,
            "testnet" => NetworkKind::Testnet,
            "testnet (current)" => NetworkKind::Testnet,
            _ => bail!("Invalid network kind"),
        },
    };
    let new_is_testnet = new_newtork_kind == NetworkKind::Testnet;

    // Provider specific inquires.
    let new_provider = match new_kind {
        ProviderKind::Utxorpc => {
            let new_url = inquire::Text::new("URL:")
                .with_default(provider.parameters().unwrap()["url"].as_str().unwrap())
                .prompt()
                .into_diagnostic()?;
            let current_headers = provider.parameters().unwrap()["headers"]
                .as_object()
                .map(|headers| {
                    headers
                        .into_iter()
                        .map(|(key, value)| format!("{key}:{value}"))
                        .collect::<Vec<String>>()
                        .join(",")
                })
                .unwrap_or("".to_string());
            let new_headers: HashMap<String, String> = inquire::Text::new(
                "Add request headers? Example: 'dmtr-api-key:dmtr_jdndajs,other:other-value'",
            )
            .with_default(&current_headers)
            .prompt()
            .into_diagnostic()?
            .split(",")
            .map(|keyval| {
                let mut parts = keyval.split(":");
                let key = match parts.next() {
                    Some(s) => s,
                    None => bail!("Invalid header."),
                };
                let val = match parts.next() {
                    Some(s) => s,
                    None => bail!("Invalid header."),
                };
                Ok((key.to_string(), val.to_string()))
            })
            .collect::<Result<_, miette::Error>>()?;

            Provider::UTxORPC(UTxORPCProvider {
                name: new_name,
                is_default: Some(new_is_default),
                is_testnet: Some(new_is_testnet),
                url: new_url,
                headers: if new_headers.is_empty() {
                    None
                } else {
                    Some(new_headers)
                },
            })
        }
    };

    ctx.store.remove_provider(provider.clone())?;
    ctx.store.add_provider(&new_provider)?;

    // Log, print, and finish
    println!("Provider edited.");
    new_provider.output(&ctx.output_format);
    Ok(())
}
