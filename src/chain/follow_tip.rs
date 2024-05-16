use clap::Parser;
use miette::{bail, Context, IntoDiagnostic};
use std::time::Duration;
use tokio::task::JoinHandle;
use utxorpc::{spec::sync::BlockRef, Cardano, CardanoSyncClient, ClientBuilder, LiveTip, TipEvent};

use crate::{
    utils::{Config, ConfigName, OutputFormatter},
    utxorpc::config::Utxorpc,
};

#[derive(Parser)]
pub struct Args {
    /// Name of the UTxO RPC config
    utxorpc_config: String,
    /// Slot of the block to use as an intersect
    slot: u64,
    /// Hash of the block to use as an intersect
    hash: String,
    /// Show only the actual tip
    #[arg(short, long)]
    tip_only: bool,
}

pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let name = ConfigName::new(args.utxorpc_config)?;
    let utxo_cfg = Utxorpc::load(&ctx.dirs, &name).await?;

    let intersect_ref = BlockRef {
        index: args.slot,
        hash: args.hash.into(),
    };

    match utxo_cfg {
        None => bail!(r#"No UTxO config named "{}" exists."#, name.raw),
        Some(cfg) => {
            if args.tip_only {
                print_current_tip(ctx, cfg, vec![intersect_ref]).await
            } else {
                print_follow_tip(ctx, cfg, vec![intersect_ref]).await
            }
        }
    }
}

async fn print_follow_tip(
    ctx: &crate::Context,
    utxo_cfg: Utxorpc,
    intersect_refs: Vec<BlockRef>,
) -> miette::Result<()> {
    let mut tip = follow_tip(utxo_cfg, intersect_refs).await?;

    while let Ok(event) = tip.event().await {
        match event {
            TipEvent::Apply(block) => {
                println!("--------Apply Block--------");
                block.output(&ctx.output_format);
            }
            TipEvent::Undo(block) => {
                println!("UNDO:\n{}", block.header.unwrap().slot)
            }
            TipEvent::Reset(point) => println!("RESET: {}", point.index),
        }
    }

    Ok(())
}

pub async fn follow_tip(
    utxo_cfg: Utxorpc,
    intersect_refs: Vec<BlockRef>,
) -> miette::Result<LiveTip<Cardano>> {
    let mut client = ClientBuilder::new()
        .uri(utxo_cfg.url)
        .into_diagnostic()
        .context("Making new ClientBuilder to follow tip")?;

    for (header, value) in utxo_cfg.headers {
        client = client
            .metadata(header, value)
            .into_diagnostic()
            .context("Adding metadata to client while getting tip")?;
    }

    let mut client = client.build::<CardanoSyncClient>().await;

    client
        .follow_tip(intersect_refs)
        .await
        .into_diagnostic()
        .context("Getting live tip from u5c")
}

async fn print_current_tip(
    ctx: &crate::Context,
    utxo_cfg: Utxorpc,
    intersect_refs: Vec<BlockRef>,
) -> miette::Result<()> {
    let tip = get_current_tip(utxo_cfg, intersect_refs).await?;
    match tip {
        Some(tip) => vec![tip].output(&ctx.output_format),
        None => bail!("An error occured."),
    }
    Ok(())
}

// This has not been tested as there is an issue with the u5c port on Demeter!
pub async fn get_current_tip(
    utxo_cfg: Utxorpc,
    intersect_refs: Vec<BlockRef>,
) -> miette::Result<Option<BlockRef>> {
    let mut live_tip = follow_tip(utxo_cfg, intersect_refs).await?;
    let (tx, rx) = std::sync::mpsc::channel::<BlockRef>();

    let handle: JoinHandle<miette::Result<()>> = tokio::spawn(async move {
        loop {
            match live_tip
                .event()
                .await
                .into_diagnostic()
                .context("u5c error while getting chain tip")?
            {
                TipEvent::Apply(block) => {
                    tx.send(BlockRef {
                        index: block.header.as_ref().unwrap().slot, // TODO
                        hash: block.header.unwrap().hash,
                    })
                    .into_diagnostic()
                    .context("Sending new tip to listener task")?;
                }
                TipEvent::Reset(block_ref) => tx
                    .send(block_ref)
                    .into_diagnostic()
                    .context("Sending reset message to listener task while getting chain tip")?,
                TipEvent::Undo(_) => {}
            }
        }
    });

    let mut tip = None;
    while let Ok(block_ref) = rx.recv_timeout(Duration::from_secs(3)) {
        tip = Some(block_ref);
    }
    handle.abort();
    Ok(tip)
}
