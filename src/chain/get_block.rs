use clap::Parser;
use miette::{bail, IntoDiagnostic};
use utxorpc::{
    spec::sync::{BlockRef, FetchBlockRequest},
    CardanoSyncClient, ClientBuilder,
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

    let block_ref = utils::block_ref(args.index, args.hash);

    match utxo_cfg {
        None => bail!(r#"No UTxO config named "{}" exists."#, name.raw),
        Some(cfg) => get_block(cfg, block_ref).await,
    }
}

pub async fn get_block(utxo_cfg: Utxorpc, block_ref: BlockRef) -> miette::Result<()> {
    let mut client = ClientBuilder::new().uri(utxo_cfg.url).into_diagnostic()?;

    for (header, value) in utxo_cfg.headers {
        client = client.metadata(header, value).into_diagnostic()?;
    }

    let mut client = client.build::<CardanoSyncClient>().await;

    let req = FetchBlockRequest {
        r#ref: vec![block_ref],
        field_mask: None,
    };

    let response = client.fetch_block(req).await.into_diagnostic()?;
    serde_json::to_string_pretty(&response.into_inner()).into_diagnostic()?;
    Ok(())
}
