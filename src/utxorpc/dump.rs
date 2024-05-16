use clap::Parser;
use miette::{bail, IntoDiagnostic};
use utxorpc::{spec::sync::BlockRef, Cardano, CardanoSyncClient, ClientBuilder, HistoryPage};

use crate::{
    utils::{Config, ConfigName, OutputFormatter},
    utxorpc::config::Utxorpc,
};

#[derive(Parser)]
pub struct Args {
    /// Name of the UTxO RPC config to use
    utxorpc_config: String,
    /// Dump from this index
    #[arg(requires = "hash")]
    index: Option<u64>,
    /// Dump from this hash
    #[arg(requires = "index")]
    hash: Option<String>,
    /// Number of blocks to fetch in each page
    #[arg(short, long, default_value = "5")]
    page_size: u32,
}

pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let name = ConfigName::new(args.utxorpc_config)?;
    let utxo_cfg = Utxorpc::load(&ctx.dirs, &name).await?;

    let start = match (args.index, args.hash) {
        (Some(index), Some(hash)) => Some(BlockRef {
            index,
            hash: hex::decode(&hash).into_diagnostic()?.into(),
        }),
        _ => None,
    };

    match utxo_cfg {
        None => bail!(r#"No UTxO config named "{}" exists."#, name.raw),
        Some(cfg) => print_paginated_history(ctx, &cfg, start, args.page_size).await,
    }
}

pub async fn print_paginated_history(
    ctx: &crate::Context,
    utxo_cfg: &Utxorpc,
    mut start: Option<BlockRef>,
    page_size: u32,
) -> miette::Result<()> {
    let mut client = build_client(utxo_cfg).await?;

    loop {
        // Get and print page
        let page = dump_history_page(&mut client, start.clone(), page_size).await?;
        for block in page.items {
            block.output(&ctx.output_format);
        }

        if !inquire::Confirm::new("Get next page?")
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

pub async fn build_client(utxo_cfg: &Utxorpc) -> miette::Result<CardanoSyncClient> {
    let mut client = ClientBuilder::new().uri(&utxo_cfg.url).into_diagnostic()?;

    for (header, value) in utxo_cfg.headers.iter() {
        client = client.metadata(header, value).into_diagnostic()?;
    }

    Ok(client.build::<CardanoSyncClient>().await)
}

pub async fn dump_history_page(
    client: &mut CardanoSyncClient,
    start: Option<BlockRef>,
    limit: u32,
) -> miette::Result<HistoryPage<Cardano>> {
    client
        .dump_history(start.clone(), limit)
        .await
        .into_diagnostic()
}
