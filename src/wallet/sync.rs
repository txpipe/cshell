use ::utxorpc::{
    spec::{
        cardano::{Block, BlockBody, BlockHeader, Tx},
        sync::BlockRef,
    },
    Cardano, CardanoSyncClient, HistoryPage, TipEvent,
};
use clap::Parser;
use futures::future::join_all;
use hex::ToHex;
use miette::{Context, IntoDiagnostic};
use num_bigint::BigInt;
use pallas::{
    applying::utils::get_shelley_address,
    ledger::addresses::{Address, ShelleyAddress},
};
use prost::bytes::Bytes;
use std::sync::{mpsc::Receiver, mpsc::Sender};
use tokio::join;
use tracing::{debug, info, instrument, trace, warn};

use crate::{
    utils::Config,
    utxorpc::{self, config::Utxorpc, dump::build_client},
    wallet::{self, config::Wallet},
};

use super::dal::{
    types::{self, TransactionInfo, TxoInfo},
    WalletDB,
};

#[derive(Parser)]
pub struct Args {
    /// Name of the wallet to sync with the chain
    #[arg(env = "CSHELL_WALLET")]
    wallet: String,
    /// Update from the block with this slot
    #[arg(long, requires = "from_hash")]
    from_slot: Option<u64>,
    /// Update from the block with this hash
    #[arg(long, requires = "from_slot")]
    from_hash: Option<String>,
    /// Number of blocks to pull from the UTxO RPC endpoint at a time
    #[arg(short, long, default_value = "200")]
    page_size: u32,
}

pub async fn run(args: Args, ctx: &crate::Context) -> miette::Result<()> {
    let wallet = Wallet::load_from_raw_name_or_bail(&ctx.dirs, args.wallet).await?;
    let (utxo_cfg, wallet_db) = get_cfg_and_db(ctx, &wallet).await?;

    let start = match (args.from_slot, args.from_hash) {
        (Some(slot), Some(hash)) => Some(BlockRef {
            index: slot,
            hash: hex::decode(&hash).into_diagnostic()?.into(),
        }),
        _ => find_intersect(utxo_cfg.clone(), &wallet_db).await?,
    };

    let start_slot = start.as_ref().map(|s| s.index).unwrap_or(0);
    if start_slot
        < wallet_db
            .get_most_recent_point()
            .await
            .into_diagnostic()?
            .map(|p| p.index)
            .unwrap_or(0)
    {
        info!("Rolling back DB to slot {}", start_slot);
        if let Some(start_ref) = start.as_ref() {
            wallet_db
                .rollback_to_slot(start_ref.index)
                .await
                .into_diagnostic()
                .context("Rolling back DB")?;
        }
    }

    info!(
        "Updating from slot {}",
        start.as_ref().map(|s| s.index).unwrap_or(0)
    );

    update(wallet_db, &wallet, utxo_cfg, start, args.page_size).await
}

async fn get_cfg_and_db(
    ctx: &crate::Context,
    wallet: &Wallet,
) -> miette::Result<(Utxorpc, WalletDB)> {
    let utxo_cfg_fut = Utxorpc::load_or_bail(&ctx.dirs, &wallet.utxorpc_config);

    let dir_path = wallet.dir_path(&ctx.dirs);
    let wallet_db_fut = wallet::dal::WalletDB::open(&wallet.name, &dir_path);

    let (utxo_cfg, wallet_db) = join!(utxo_cfg_fut, wallet_db_fut);
    Ok((utxo_cfg?, wallet_db.into_diagnostic()?))
}

// This has not been tested yet due to issues with the Demeter u5c port
async fn find_intersect(
    utxo_cfg: Utxorpc,
    wallet_db: &WalletDB,
) -> miette::Result<Option<BlockRef>> {
    let intersect_refs = wallet_db
        .get_recent_points_spread(None)
        .await
        .into_diagnostic()
        .context("Getting recent points spread for chain intersect points")?;

    if intersect_refs.is_empty() {
        Ok(None)
    } else {
        let mut live_tip = utxorpc::follow_tip::follow_tip(utxo_cfg, intersect_refs).await?;

        loop {
            match live_tip
                .event()
                .await
                .into_diagnostic()
                .context("Following tip to find intersect for update")?
            {
                TipEvent::Apply(block) => {
                    return Ok(Some(
                        types::block_ref_from_block(block)
                            .context("Following tip to get intersect for update")?,
                    ))
                }
                TipEvent::Reset(block_ref) => return Ok(Some(block_ref)),
                TipEvent::Undo(_) => {}
            }
        }
    }
}

#[instrument(skip_all, fields(wallet = wallet.name.raw, utxo_cfg = utxo_cfg.name.raw))]
async fn update(
    wallet_db: WalletDB,
    wallet: &Wallet,
    utxo_cfg: Utxorpc,
    mut start: Option<BlockRef>,
    page_limit: u32,
) -> miette::Result<()> {
    let (tx, rx): (Sender<Option<Vec<Block>>>, Receiver<Option<Vec<Block>>>) =
        std::sync::mpsc::channel();

    let consumer_handle = tokio::spawn(page_consumer(
        rx,
        wallet_db,
        types::shelley_addr_from_general(
            Address::from_bech32(wallet.address(&utxo_cfg)).into_diagnostic()?,
        )?,
    ));

    let mut utxo_client = build_client(&utxo_cfg).await?;

    loop {
        let page = get_history_page(&mut utxo_client, start.clone(), page_limit).await?;
        tx.send(Some(page.items)).into_diagnostic()?;

        if page.next.is_none() {
            tx.send(None).into_diagnostic()?;
            break;
        } else {
            start = page.next;
        }
    }

    consumer_handle.await.into_diagnostic()??;

    Ok(())
}

async fn get_history_page(
    client: &mut CardanoSyncClient,
    start: Option<BlockRef>,
    page_size: u32,
) -> miette::Result<HistoryPage<Cardano>> {
    let start_slot = start.as_ref().map_or(0, |b| b.index);
    trace!("Getting history dump starting from {}.", start_slot);

    let page = utxorpc::dump::dump_history_page(client, start, page_size).await?;

    let end_slot = page
        .next
        .as_ref()
        .map_or(String::from("End"), |b| b.index.to_string());

    trace!("Received history dump from {} to {}.", start_slot, end_slot);

    Ok(page)
}

#[instrument(name = "page_consumer", skip_all)]
async fn page_consumer(
    rx: Receiver<Option<Vec<Block>>>,
    wallet_db: WalletDB,
    wallet_address: ShelleyAddress,
) -> miette::Result<()> {
    let mut total_blocks = 0;

    while let Some(items) = rx.recv().into_diagnostic()? {
        let data = collect_data_from_page(&wallet_db, &wallet_address, &items).await;

        if data.has_data() {
            debug!(
                "Inserting {} blocks, {} txs, {} utxos into DB and removing {} used inputs.",
                data.blocks.len(),
                data.txs.len(),
                data.utxos.len(),
                data.used_inputs.len()
            );
            persist_processing_data(&wallet_db, &data).await?;
        }

        if !data.recent_points.is_empty() {
            persist_recent_points(&wallet_db, data.recent_points).await?;
        }

        total_blocks += items.len();
        trace!("Total blocks processed: {total_blocks}");
    }

    trace!("History page consumer finished.");
    Ok(())
}

#[instrument(skip_all)]
async fn persist_processing_data(
    wallet_db: &WalletDB,
    data: &ChainProcessingData,
) -> miette::Result<()> {
    wallet_db.insert_blocks(&data.blocks).await.unwrap();
    wallet_db
        .insert_history_txs(&data.txs)
        .await
        .into_diagnostic()?;
    wallet_db
        .remove_utxos(&data.used_inputs)
        .await
        .into_diagnostic()?;
    wallet_db
        .insert_utxos(&data.utxos)
        .await
        .into_diagnostic()?;
    Ok(())
}

async fn persist_recent_points(
    wallet_db: &WalletDB,
    recent_points: Vec<(u64, Vec<u8>)>,
) -> miette::Result<()> {
    wallet_db
        .insert_recent_points(recent_points)
        .await
        .into_diagnostic()
        .context("Inserting recent points")
}

#[instrument(skip_all)]
async fn collect_data_from_page(
    wallet_db: &WalletDB,
    wallet_address: &ShelleyAddress,
    history_items: &Vec<Block>,
) -> ChainProcessingData {
    trace!(
        "Extracting data from page of {} blocks starting from {}",
        history_items.len(),
        history_items
            .first()
            .map_or("No items!".to_owned(), |item| item
                .header
                .as_ref()
                .unwrap()
                .slot
                .to_string())
    );

    let mut data = ChainProcessingData::empty();

    let blocks = history_items
        .iter()
        .flat_map(|block| match (&block.header, &block.body) {
            (Some(header), Some(body)) => Some((block, header, body)),
            _ => {
                warn!(
                    "A block was found that either did not have a header or did not have a body."
                );
                None
            }
        });

    for (block, header, body) in blocks {
        collect_data_from_block(&mut data, wallet_db, wallet_address, block, header, body).await
    }

    data
}

async fn collect_data_from_block(
    data: &mut ChainProcessingData,
    wallet_db: &WalletDB,
    wallet_address: &ShelleyAddress,
    block: &Block,
    header: &BlockHeader,
    body: &BlockBody,
) {
    let mut should_record_block = false;
    for (tx_idx, tx) in body.tx.iter().enumerate() {
        should_record_block = should_record_block
            || collect_data_from_tx(
                data,
                wallet_db,
                wallet_address,
                header.slot,
                &header.hash,
                tx,
                tx_idx,
            )
            .await
    }

    // Push block
    if should_record_block {
        debug!(
            "Found relevant block: slot {}",
            block
                .header
                .as_ref()
                .map_or("error".to_owned(), |h| h.slot.to_string())
        );
        data.blocks.push(block.clone());
    }

    // Push recent point
    data.recent_points.push((header.slot, header.hash.to_vec()));
}

async fn collect_data_from_tx(
    data: &mut ChainProcessingData,
    wallet_db: &WalletDB,
    wallet_address: &ShelleyAddress,
    slot: u64,
    block_hash: &Bytes,
    tx: &Tx,
    tx_idx: usize,
) -> bool {
    let used_inputs_value = collect_used_inputs(data, wallet_db, wallet_address, slot, tx).await;

    // Collect UTxOs
    let utxo_value = collect_utxos(data, wallet_address, slot, tx);

    // Push Tx
    if utxo_value.is_some() || used_inputs_value.is_some() {
        data.txs.push(TransactionInfo {
            hash: tx.hash.clone(),
            block_hash: block_hash.clone(),
            slot,
            tx_index: tx_idx as u16,
            delta: utxo_value.unwrap_or(BigInt::ZERO) - used_inputs_value.unwrap_or(BigInt::ZERO),
        });
        true
    } else {
        false
    }
}

#[instrument(name = "resolve_used_inputs", skip_all)]
async fn collect_used_inputs(
    data: &mut ChainProcessingData,
    wallet_db: &WalletDB,
    wallet_address: &ShelleyAddress,
    slot: u64,
    tx: &Tx,
) -> Option<BigInt> {
    let inputs_as_txo_infos = get_used_inputs_as_txo_infos(wallet_db, tx, slot).await;

    // Collect used inputs as TxoInfo in `data` and return value of used inputs
    collect_txo_info(
        wallet_address,
        slot,
        tx,
        &inputs_as_txo_infos,
        &mut data.used_inputs,
    )
}

async fn get_used_inputs_as_txo_infos(wallet_db: &WalletDB, tx: &Tx, slot: u64) -> Vec<TxoInfo> {
    let inputs_as_txo_info_futs: Vec<_> = tx
        .inputs
        .iter()
        .map(|input| async move {
            let resolved_from_as_output = input
                .as_output
                .as_ref()
                .map(|output| TxoInfo::from_tx_input_output(output, input, slot));

            // as_output seems to be broken so try to fetch the TxOutput info for this input from the DB
            let resolved_from_db = match resolved_from_as_output {
                Some(resolved) => Some(resolved),
                None => {
                    debug!(
                        tx_hash = hex::encode(&input.tx_hash),
                        output_index = input.output_index,
                        "input.as_output failed"
                    );
                    wallet_db
                        .resolve_utxo(&tx.hash.to_vec(), input.output_index)
                        .await
                        .ok()
                        .flatten()
                }
            };

            if let None = resolved_from_db {
                debug!(
                    tx_hash = hex::encode(&input.tx_hash),
                    output_index = input.output_index,
                    "Resolving from DB failed"
                );
            }
            resolved_from_db
        })
        .collect();

    let inputs_as_txo_info: Vec<TxoInfo> = join_all(inputs_as_txo_info_futs)
        .await
        .into_iter()
        .flatten()
        .collect();

    if tx.inputs.len() > inputs_as_txo_info.len() {
        warn!(tx_hash = hex::encode(&tx.hash), "Could not find {} of {} inputs as outputs. There may be UTxOs in the DB that should have been removed.",
        tx.inputs.len() - inputs_as_txo_info.len(), tx.inputs.len());
    }

    inputs_as_txo_info
}

fn collect_utxos(
    data: &mut ChainProcessingData,
    wallet_address: &ShelleyAddress,
    slot: u64,
    tx: &Tx,
) -> Option<BigInt> {
    let utxos_as_txo_info = tx
        .outputs
        .iter()
        .enumerate()
        .map(|(txo_idx, output)| TxoInfo::from_parts(output, tx.hash.clone(), txo_idx as u32, slot))
        .collect();

    // Collect Utxos as TxoInfo in `data` and return value of UTxOs
    collect_txo_info(
        wallet_address,
        slot,
        tx,
        &utxos_as_txo_info,
        &mut data.utxos,
    )
}

fn collect_txo_info(
    wallet_address: &ShelleyAddress,
    slot: u64,
    tx: &Tx,
    txos: &Vec<TxoInfo>,
    collector: &mut Vec<TxoInfo>,
) -> Option<BigInt> {
    let mut txos_total_value: Option<BigInt> = None; // (0 as u8).into();

    for (txo_idx, txo) in txos.iter().enumerate() {
        // Get address from TxO -- if not a Shelly address, continue with warning.
        let utxo_addr = match get_shelley_address(&txo.address) {
            Some(addr) => addr,
            None => {
                warn!("Encountered an address that was not a Shelley address.");
                continue;
            }
        };

        if utxo_addr == *wallet_address {
            // TODO: Use payment part or full address?
            let info = TxoInfo {
                tx_hash: tx.hash.clone(),
                txo_index: txo_idx as u32,
                address: txo.address.clone(),
                slot,
                coin: txo.coin,
            };

            debug!(
                tx_hash = tx.hash.as_ref().encode_hex::<String>(),
                txo_idx,
                slot,
                coin = txo.coin,
                "Found (U)TxO"
            );

            collector.push(info);
            txos_total_value = {
                let old_val = txos_total_value.unwrap_or(0.into());
                Some(old_val + txo.coin)
            };
        }
    }

    txos_total_value
}

struct ChainProcessingData {
    blocks: Vec<Block>,
    txs: Vec<TransactionInfo>,
    used_inputs: Vec<TxoInfo>,
    utxos: Vec<TxoInfo>,
    recent_points: Vec<(u64, Vec<u8>)>,
}
impl ChainProcessingData {
    fn empty() -> Self {
        Self {
            blocks: vec![],
            txs: vec![],
            used_inputs: vec![],
            utxos: vec![],
            recent_points: vec![],
        }
    }

    fn has_data(&self) -> bool {
        // self.blocks _should_ be enought to determine on its own, but check all for safety
        !(self.blocks.is_empty()
            && self.txs.is_empty()
            && self.used_inputs.is_empty()
            && self.utxos.is_empty())
    }
}
