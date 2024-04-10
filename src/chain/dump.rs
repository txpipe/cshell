use clap::Parser;
use miette::{bail, IntoDiagnostic};
use utxorpc::{spec::sync::BlockRef, CardanoSyncClient, ClientBuilder};

use crate::{
    utils::{Config, ConfigName},
    utxorpc::config::Utxorpc,
};

use super::utils;

#[derive(Parser)]
pub struct Args {
    utxorpc_config: String,
    limit: u32,
    #[arg(requires = "hash")]
    index: Option<u64>,
    #[arg(requires = "index")]
    hash: Option<String>,
    #[arg(short, long)]
    pages: u32,
}

pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let name = ConfigName::new(args.utxorpc_config)?;
    let utxo_cfg = Utxorpc::load(&ctx.dirs, &name).await?;

    let start = match (args.index, args.hash) {
        (Some(index), Some(hash)) => Some(utils::block_ref(index, hash)),
        _ => None,
    };

    match utxo_cfg {
        None => bail!(r#"No UTxO config named "{}" exists."#, name.raw),
        Some(cfg) => dump_history(cfg, start, args.limit, args.pages).await,
    }
}

pub async fn dump_history(
    utxo_cfg: Utxorpc,
    mut start: Option<BlockRef>,
    limit: u32,
    pages: u32,
) -> miette::Result<()> {
    let mut client = ClientBuilder::new().uri(utxo_cfg.url).into_diagnostic()?;

    for (header, value) in utxo_cfg.headers {
        client = client.metadata(header, value).into_diagnostic()?;
    }

    let mut client = client.build::<CardanoSyncClient>().await;

    for _ in 0..pages {
        let page = client.dump_history(start, limit).await.into_diagnostic()?;

        for block in page.items {
            println!(
                "{}",
                serde_json::to_string_pretty(&block).into_diagnostic()?
            );
        }

        if !inquire::Confirm::new("Get next page?")
            .with_default(true)
            .prompt()
            .into_diagnostic()?
        {
            break;
        } else {
            match page.next {
                Some(next) => {
                    start = Some(next);
                }
                None => break,
            }
        }
    }

    Ok(())
}
