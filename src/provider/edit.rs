use std::collections::HashMap;

use anyhow::{bail, Context, Result};
use clap::Parser;
use inquire::list_option::ListOption;

use crate::{
    output::OutputFormatter,
    provider::types::Provider,
    utils::{show_is_current, Name},
};

#[derive(clap::ValueEnum, Clone, PartialEq)]
enum NetworkKind {
    Mainnet,
    Testnet,
}

#[derive(Parser, Clone)]
pub struct Args {
    /// Name to identify the provider.
    name: Option<String>,

    /// Name to identify the provider.
    #[arg(long)]
    new_name: Option<String>,

    /// Whether to set as default provider.
    #[arg(long)]
    is_default: Option<bool>,

    /// Whether it is mainnet or testnet.
    network_kind: Option<NetworkKind>,
}

// #[instrument("create", skip_all)]
pub async fn run(args: Args, ctx: &mut crate::Context) -> anyhow::Result<()> {
    let provider = match args.name {
        Some(name) => ctx.store.find_provider(&name),
        None => ctx.store.default_provider(),
    }
    .context("Provider not found")?;

    let new_name = match args.new_name {
        None => {
            let new_name = inquire::Text::new("New name: ")
                .with_default(&provider.name())
                .prompt()
                .map_err(anyhow::Error::msg)?;
            Name::try_from(new_name)?
        }
        Some(new_name) => Name::try_from(new_name)?,
    };

    let new_is_default = match args.is_default {
        Some(x) => x,
        None => match inquire::Select::new(
            "Set as default?",
            vec![
                ListOption::new(0, show_is_current("yes", provider.is_default()).as_str()),
                ListOption::new(1, show_is_current("no", !provider.is_default()).as_str()),
            ],
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

    let new_newtork_kind = match args.network_kind {
        Some(network_kind) => network_kind,
        None => match inquire::Select::new(
            "Network kind:",
            vec![
                ListOption::new(
                    0,
                    show_is_current("mainnet", !provider.is_testnet()).as_str(),
                ),
                ListOption::new(
                    1,
                    show_is_current("testnet", provider.is_testnet()).as_str(),
                ),
            ],
        )
        .prompt()
        .map_err(anyhow::Error::msg)?
        .index
        {
            0 => NetworkKind::Mainnet,
            1 => NetworkKind::Testnet,
            _ => bail!("Invalid network kind"),
        },
    };
    let new_is_testnet = new_newtork_kind == NetworkKind::Testnet;

    let new_url = inquire::Text::new("URL:")
        .with_default(&provider.url)
        .prompt()
        .map_err(anyhow::Error::msg)?;
    let current_headers = provider
        .headers
        .clone()
        .map(|headers| {
            headers
                .into_iter()
                .map(|(key, value)| format!("{key}:{value}"))
                .collect::<Vec<String>>()
                .join(",")
        })
        .unwrap_or("".to_string());

    println!("current headers: {current_headers}");
    let new_headers: HashMap<String, String> = inquire::Text::new(
        "Add request headers? Example: 'dmtr-api-key:dmtr_jdndajs,other:other-value'",
    )
    .with_default(&current_headers)
    .prompt()
    .map_err(anyhow::Error::msg)?
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
    .collect::<Result<_, anyhow::Error>>()?;

    let new_trp_url = inquire::Text::new("TRP URL:")
        .with_default(&provider.trp_url.clone().unwrap_or("".to_string()))
        .prompt()
        .map_err(anyhow::Error::msg)?;

    let current_trp_headers = provider
        .trp_headers
        .clone()
        .map(|headers| {
            headers
                .into_iter()
                .map(|(key, value)| format!("{key}:{value}"))
                .collect::<Vec<String>>()
                .join(",")
        })
        .unwrap_or("".to_string());
    let new_trp_headers: HashMap<String, String> = inquire::Text::new(
        "Add TRP request headers? Example: 'dmtr-api-key:dmtr_jdndajs,other:other-value'",
    )
    .with_default(&current_trp_headers)
    .prompt()
    .map_err(anyhow::Error::msg)?
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
    .collect::<Result<_, anyhow::Error>>()?;

    let new_provider = Provider {
        name: new_name,
        is_default: Some(new_is_default),
        is_testnet: Some(new_is_testnet),
        url: new_url,
        headers: if new_headers.is_empty() {
            None
        } else {
            Some(new_headers)
        },
        trp_url: if new_trp_url.is_empty() {
            None
        } else {
            Some(new_trp_url)
        },
        trp_headers: if new_trp_headers.is_empty() {
            None
        } else {
            Some(new_trp_headers)
        },
    };

    ctx.store.remove_provider(provider.clone())?;
    ctx.store.add_provider(&new_provider)?;

    // Log, print, and finish
    new_provider.output(&ctx.output_format);
    Ok(())
}
