use clap::Parser;
use miette::{bail, IntoDiagnostic};
use utxorpc::{
    spec::sync::{BlockRef, FetchBlockRequest, FollowTipRequest},
    CardanoSyncClient, ClientBuilder, TipEvent,
};

use crate::{
    utils::{Config, ConfigName},
    utxorpc::config::Utxorpc,
};

use super::utils;

#[derive(Parser)]
pub struct Args {
    utxorpc_config: String,
    index: u64,
    hash: String,
}

pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let name = ConfigName::new(args.utxorpc_config)?;
    let utxo_cfg = Utxorpc::load(&ctx.dirs, &name).await?;

    let intersect_ref = utils::block_ref(args.index, args.hash);

    match utxo_cfg {
        None => bail!(r#"No UTxO config named "{}" exists."#, name.raw),
        Some(cfg) => follow_tip(cfg, intersect_ref).await,
    }
}

pub async fn follow_tip(utxo_cfg: Utxorpc, intersect_ref: BlockRef) -> miette::Result<()> {
    let mut client = ClientBuilder::new().uri(utxo_cfg.url).into_diagnostic()?;

    for (header, value) in utxo_cfg.headers {
        client = client.metadata(header, value).into_diagnostic()?;
    }

    let mut client = client.build::<CardanoSyncClient>().await;

    let mut tip = client
        .follow_tip(vec![intersect_ref])
        .await
        .into_diagnostic()?;

    while let Ok(event) = tip.event().await {
        match event {
            TipEvent::Apply(block) => println!(
                "APPLY:\n{}",
                serde_json::to_string_pretty(&block).into_diagnostic()?
            ),
            TipEvent::Undo(block) => {
                println!("UNDO:\n{}", block.header.unwrap().slot)
            }
            TipEvent::Reset(point) => println!("RESET: {}", point.index),
        }
    }
    Ok(())
}
