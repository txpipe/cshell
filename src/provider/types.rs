use std::collections::HashMap;

use comfy_table::Table;
use jsonrpsee::{
    core::{client::ClientT, params::ObjectParams},
    http_client::HttpClient,
};
use miette::{bail, Context, IntoDiagnostic};
use pallas::ledger::addresses::Address;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utxorpc::{
    spec::{
        query::any_utxo_pattern::UtxoPattern,
        sync::{AnyChainBlock, BlockRef, FetchBlockRequest},
    },
    CardanoQueryClient, CardanoSubmitClient, CardanoSyncClient, ClientBuilder, InnerService,
};

use crate::{
    output::OutputFormatter,
    types::{Asset, Balance, BalanceAsset, Datum, DetailedBalance, UTxO},
    utils::Name,
};

#[derive(Serialize, Deserialize)]
pub struct TrpResponse {
    #[serde(with = "hex::serde")]
    pub tx: Vec<u8>,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
#[serde(tag = "type")]
pub struct Provider {
    pub name: Name,
    pub url: String,
    pub headers: Option<HashMap<String, String>>,
    pub is_default: Option<bool>,
    pub is_testnet: Option<bool>,
    pub trp_url: Option<String>,
    pub trp_headers: Option<HashMap<String, String>>,
}

impl Provider {
    pub fn name(&self) -> String {
        self.name.to_string()
    }

    pub fn parameters(&self) -> Option<Value> {
        Some(json!({
            "url": self.url,
            "headers": self.headers
        }))
    }
    pub fn is_default(&self) -> bool {
        self.is_default.unwrap_or(false)
    }

    pub fn is_testnet(&self) -> bool {
        self.is_testnet.unwrap_or(false)
    }

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
    pub async fn test(&self) -> miette::Result<()> {
        println!("Building client...");
        let mut client: CardanoSyncClient = self.client().await?;

        println!("Executing ReadTip method...");
        let result = client.read_tip().await.into_diagnostic()?;
        match result {
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

        let mut result: DetailedBalance = utxos
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
            .collect();

        result.sort_by(|x, y| x.tx.cmp(&y.tx));

        Ok(result)
    }

    pub async fn submit(&self, tx: &[u8]) -> miette::Result<Vec<u8>> {
        let mut client: CardanoSubmitClient = self.client().await?;
        client
            .submit_tx(vec![tx.to_vec()])
            .await
            .into_diagnostic()
            .map(|x| x.first().unwrap().to_vec())
    }

    pub async fn trp_resolve(&self, params: &ObjectParams) -> miette::Result<TrpResponse> {
        let Some(trp_url) = &self.trp_url else {
            bail!("missing TRP configuration for this provider")
        };

        let mut client_builder = HttpClient::builder();
        if let Some(headers) = &self.trp_headers {
            let headermap = headers.try_into().into_diagnostic()?;
            client_builder = client_builder.set_headers(headermap);
        }
        let client = client_builder.build(trp_url).into_diagnostic()?;

        client
            .request("trp.resolve", params.to_owned())
            .await
            .into_diagnostic()
    }

    pub async fn fetch_block(
        &self,
        refs: Vec<(Vec<u8>, u64)>,
    ) -> miette::Result<Vec<AnyChainBlock>> {
        let mut client: utxorpc::CardanoSyncClient = self.client().await?;

        let refs = refs
            .iter()
            .map(|(hash, index)| BlockRef {
                hash: hash.clone().into(),
                index: *index,
            })
            .collect();

        let request = FetchBlockRequest {
            r#ref: refs,
            ..Default::default()
        };

        let response = client
            .fetch_block(request)
            .await
            .into_diagnostic()?
            .into_inner();

        Ok(response.block)
    }
}

impl OutputFormatter for Provider {
    fn to_table(&self) {
        let mut table = Table::new();

        table.set_header(vec!["Name", "is testnet?", "Is default?", "Parameters"]);
        table.add_row(vec![
            self.name(),
            self.is_testnet().to_string(),
            self.is_default().to_string(),
            match self.parameters() {
                Some(value) => serde_json::to_string(&value).unwrap(),
                None => "".to_string(),
            },
        ]);

        println!("{}", table);
    }

    fn to_json(&self) {
        println!("{}", serde_json::to_string_pretty(self).unwrap());
    }
}

impl OutputFormatter for &Vec<Provider> {
    fn to_table(&self) {
        let mut table = Table::new();
        table.set_header(vec!["Name", "Is default?", "Parameters"]);
        for provider in *self {
            table.add_row(vec![
                provider.name(),
                provider.is_default().to_string(),
                match provider.parameters() {
                    Some(value) => serde_json::to_string(&value).unwrap(),
                    None => "".to_string(),
                },
            ]);
        }
        println!("{}", table);
    }

    fn to_json(&self) {
        println!("{}", serde_json::to_string_pretty(self).unwrap());
    }
}
