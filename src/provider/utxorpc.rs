use miette::{Context, IntoDiagnostic};
use pallas::ledger::addresses::Address;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utxorpc::{
    spec::{query::any_utxo_pattern::UtxoPattern, sync::BlockRef},
    CardanoQueryClient, CardanoSyncClient, ClientBuilder, InnerService,
};

use crate::{
    types::{Asset, Balance, BalanceAsset, Datum, DetailedBalance, UTxO},
    utils::Name,
};

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct UTxORPCProvider {
    pub name: Name,
    pub url: String,
    pub headers: Option<HashMap<String, String>>,
    pub is_default: Option<bool>,
    pub is_testnet: Option<bool>,
}

impl UTxORPCProvider {
    pub async fn client<T>(&self) -> miette::Result<T>
    where
        T: From<InnerService>,
    {
        let mut client_builder = ClientBuilder::new()
            .uri(self.url.clone())
            .into_diagnostic()?;

        if let Some(headers) = &self.headers {
            for (k, v) in headers {
                client_builder = client_builder.metadata(k, v).into_diagnostic()?;
            }
        }
        Ok(client_builder.build::<T>().await)
    }

    pub async fn get_tip(&self) -> miette::Result<Option<BlockRef>> {
        let mut client: CardanoSyncClient = self.client().await?;
        client.read_tip().await.into_diagnostic()
    }

    pub async fn test(&self) -> miette::Result<()> {
        println!("Executing ReadTip method...");
        match self.get_tip().await? {
            Some(blockref) => {
                println!(
                    "Successfull request, block tip at slot {} and hash {}.",
                    blockref.index,
                    hex::encode(blockref.hash)
                )
            }
            None => println!("Successfull request"),
        }

        Ok(())
    }

    pub async fn get_balance(&self, address: &Address) -> miette::Result<Balance> {
        let mut client: CardanoQueryClient = self.client().await?;

        let predicate = utxorpc::spec::query::UtxoPredicate {
            r#match: Some(utxorpc::spec::query::AnyUtxoPattern {
                utxo_pattern: Some(UtxoPattern::Cardano(
                    utxorpc::spec::cardano::TxOutputPattern {
                        address: Some(utxorpc::spec::cardano::AddressPattern {
                            exact_address: address.to_vec().into(),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                )),
            }),
            ..Default::default()
        };
        let utxos = client
            .search_utxos(predicate, None, u32::MAX)
            .await
            .into_diagnostic()
            .context("failed to query utxos")?;

        let coin: u64 = utxos
            .items
            .clone()
            .into_iter()
            .map(|x| x.parsed.unwrap().coin)
            .sum();

        let assets = utxos
            .items
            .clone()
            .into_iter()
            .flat_map(|x| {
                x.parsed
                    .unwrap()
                    .assets
                    .iter()
                    .map(|asset| BalanceAsset {
                        policy_id: asset.policy_id.to_vec(),
                        assets: asset
                            .assets
                            .iter()
                            .map(|inner| Asset {
                                name: inner.name.to_vec(),
                                output_coin: inner.output_coin.to_string(),
                            })
                            .collect::<Vec<Asset>>(),
                    })
                    .collect::<Vec<BalanceAsset>>()
            })
            .collect();

        let datums = utxos
            .items
            .clone()
            .into_iter()
            .flat_map(|x| match x.parsed.unwrap().datum {
                Some(datum) => {
                    if datum.hash.is_empty() {
                        None
                    } else {
                        Some(Datum {
                            hash: datum.hash.to_vec(),
                        })
                    }
                }
                None => None,
            })
            .collect();

        Ok(Balance {
            coin: coin.to_string(),
            address: address.to_string(),
            assets,
            datums,
        })
    }

    pub async fn get_detailed_balance(&self, address: &Address) -> miette::Result<DetailedBalance> {
        let mut client: CardanoQueryClient = self.client().await?;

        let predicate = utxorpc::spec::query::UtxoPredicate {
            r#match: Some(utxorpc::spec::query::AnyUtxoPattern {
                utxo_pattern: Some(UtxoPattern::Cardano(
                    utxorpc::spec::cardano::TxOutputPattern {
                        address: Some(utxorpc::spec::cardano::AddressPattern {
                            exact_address: address.to_vec().into(),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                )),
            }),
            ..Default::default()
        };
        let utxos = client
            .search_utxos(predicate, None, u32::MAX)
            .await
            .into_diagnostic()
            .context("failed to query utxos")?;

        let mut result = utxos
            .items
            .into_iter()
            .map(|utxo| {
                let txoref = utxo.txo_ref.unwrap();
                let utxo = utxo.parsed.unwrap();
                UTxO {
                    tx: txoref.hash.to_vec(),
                    tx_index: txoref.index as u64,
                    address: address.to_string(),
                    coin: utxo.coin.to_string(),
                    assets: utxo
                        .assets
                        .iter()
                        .map(|asset| BalanceAsset {
                            policy_id: asset.policy_id.to_vec(),
                            assets: asset
                                .assets
                                .iter()
                                .map(|inner| Asset {
                                    name: inner.name.to_vec(),
                                    output_coin: inner.output_coin.to_string(),
                                })
                                .collect::<Vec<Asset>>(),
                        })
                        .collect::<Vec<BalanceAsset>>(),
                    datum: match utxo.datum {
                        Some(datum) => {
                            if datum.hash.is_empty() {
                                None
                            } else {
                                Some(Datum {
                                    hash: datum.hash.to_vec(),
                                })
                            }
                        }
                        None => None,
                    },
                }
            })
            .collect::<DetailedBalance>();

        result.sort_by(|x, y| x.tx.cmp(&y.tx));

        Ok(result)
    }
}
