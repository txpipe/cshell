use clap::Parser;
use miette::{bail, IntoDiagnostic};
use utxorpc::{spec::sync::BlockRef, Cardano, CardanoSyncClient, ClientBuilder, HistoryPage};

use crate::{
    utils::{Config, ConfigName, OutputFormatter},
    utxorpc::config::Utxorpc,
};

#[derive(Parser)]
pub struct Args {
    utxorpc_config: String,
    limit: u32,
    #[arg(requires = "hash")]
    index: Option<u64>,
    #[arg(requires = "index")]
    hash: Option<String>,
    #[arg(short, long, default_value = "1")]
    pages: u32,
}

pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let name = ConfigName::new(args.utxorpc_config)?;
    let utxo_cfg = Utxorpc::load(&ctx.dirs, &name).await?;

    let start = match (args.index, args.hash) {
        (Some(index), Some(hash)) => Some(BlockRef {
            index,
            hash: hash.into(),
        }),
        _ => None,
    };

    match utxo_cfg {
        None => bail!(r#"No UTxO config named "{}" exists."#, name.raw),
        Some(cfg) => print_paginated_history(ctx, &cfg, start, args.limit, args.pages).await,
    }
}

pub async fn print_paginated_history(
    ctx: &crate::Context,
    utxo_cfg: &Utxorpc,
    mut start: Option<BlockRef>,
    limit: u32,
    pages: u32,
) -> miette::Result<()> {
    for page_idx in 0..pages {
        // Get and print page
        let page = dump_history(utxo_cfg, &start, limit).await?;
        for block in page.items {
            block.output(&ctx.output_format);
        }

        if page_idx > 0
            && !inquire::Confirm::new("Get next page?")
                .with_default(true)
                .prompt()
                .into_diagnostic()?
        {
            break;
        } else {
            match page.next {
                Some(next) => start = Some(next),
                None => println!("Chain tip reached."),
            }
        }
    }

    Ok(())
}

pub async fn dump_history(
    utxo_cfg: &Utxorpc,
    start: &Option<BlockRef>,
    limit: u32,
) -> miette::Result<HistoryPage<Cardano>> {
    let mut client = ClientBuilder::new().uri(&utxo_cfg.url).into_diagnostic()?;

    for (header, value) in utxo_cfg.headers.iter() {
        client = client.metadata(header, value).into_diagnostic()?;
    }

    let mut client = client.build::<CardanoSyncClient>().await;

    let page = client
        .dump_history(start.clone(), limit)
        .await
        .into_diagnostic()?;

    Ok(page)
}
