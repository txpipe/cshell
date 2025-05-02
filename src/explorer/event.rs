use std::{collections::HashMap, sync::Arc, time::Duration};

use futures::{FutureExt, StreamExt};
use miette::{Context, IntoDiagnostic};
use ratatui::crossterm::event::Event as CrosstermEvent;
use tokio::sync::mpsc;
use utxorpc::{
    spec::sync::{any_chain_block::Chain, BlockRef, FetchBlockRequest},
    CardanoSyncClient, TipEvent,
};

use crate::{types::DetailedBalance, wallet::types::Wallet};

use super::{ChainBlock, ExplorerContext};

#[derive(Clone, Debug)]
pub enum Event {
    Crossterm(CrosstermEvent),
    App(AppEvent),
    Tick,
}

#[derive(Clone, Debug)]
pub enum AppEvent {
    Reset(u64),
    NewTip(ChainBlock),
    UndoTip(ChainBlock),
    BalanceUpdate((String, DetailedBalance)),
}

#[derive(Debug)]
pub struct EventHandler {
    receiver: mpsc::UnboundedReceiver<Event>,
}

impl EventHandler {
    pub fn new(context: Arc<ExplorerContext>) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let actor = EventTask::new(sender.clone(), context);
        tokio::spawn(async { actor.run().await });
        Self { receiver }
    }

    pub async fn next(&mut self) -> miette::Result<Event> {
        self.receiver
            .recv()
            .await
            .ok_or(miette::Report::msg("failed to receive event"))
    }
}

struct EventTask {
    sender: mpsc::UnboundedSender<Event>,
    context: Arc<ExplorerContext>,
}

impl EventTask {
    fn new(sender: mpsc::UnboundedSender<Event>, context: Arc<ExplorerContext>) -> Self {
        Self { sender, context }
    }

    async fn run(self) -> miette::Result<()> {
        let keys = async || -> miette::Result<()> {
            let mut reader = crossterm::event::EventStream::new();

            loop {
                if let Some(Ok(evt)) = reader.next().fuse().await {
                    self.send(Event::Crossterm(evt))?;
                }
            }
        };

        let sender = async {
            self.sender.closed().await;
            Ok::<_, miette::Error>(())
        };

        let ticks = async || -> miette::Result<()> {
            let tick_rate = Duration::from_secs(1);
            let mut tick = tokio::time::interval(tick_rate);
            loop {
                let _ = tick.tick().await;
                self.send(Event::Tick)?
            }
        };

        let follow_tip = async { self.follow_tip().await };

        tokio::try_join!(sender, keys(), follow_tip, ticks())?;
        Ok(())
    }

    fn send(&self, event: Event) -> miette::Result<()> {
        self.sender
            .send(event)
            .into_diagnostic()
            .context("sending event")
    }

    async fn get_balance(&self, wallet: &Wallet) -> miette::Result<DetailedBalance> {
        self.context
            .provider
            .get_detailed_balance(&wallet.address(self.context.provider.is_testnet()))
            .await
    }

    async fn check_balances(
        &self,
        balances: &mut HashMap<String, DetailedBalance>,
    ) -> miette::Result<()> {
        for wallet in self.context.store.wallets() {
            let key = wallet
                .address(self.context.provider.is_testnet())
                .to_string();
            let new = self.get_balance(wallet).await?;
            match balances.get(&key) {
                Some(old) => {
                    if new != *old {
                        balances.insert(key.clone(), new.clone());
                        self.send(Event::App(AppEvent::BalanceUpdate((key, new))))?;
                    }
                }
                None => {
                    balances.insert(key.clone(), new.clone());
                    self.send(Event::App(AppEvent::BalanceUpdate((key, new))))?;
                }
            };
        }

        Ok(())
    }

    async fn follow_tip(&self) -> miette::Result<()> {
        let mut balances = HashMap::new();
        for wallet in self.context.store.wallets() {
            let key = wallet
                .address(self.context.provider.is_testnet())
                .to_string();
            let value = self.get_balance(wallet).await?;
            self.send(Event::App(AppEvent::BalanceUpdate((
                key.clone(),
                value.clone(),
            ))))?;
            balances.insert(key, value);
        }

        loop {
            let mut client: CardanoSyncClient = self.context.provider.client().await?;
            let mut tip = client.follow_tip(vec![]).await.unwrap();

            while let Ok(event) = tip.event().await {
                match event {
                    TipEvent::Apply(block) => {
                        let header = block.parsed.clone().unwrap().header.unwrap();
                        let tx_count = match block.parsed {
                            Some(parsed) => match parsed.body {
                                Some(body) => body.tx.len(),
                                None => 0,
                            },
                            None => 0,
                        };

                        let response = client
                            .fetch_block(FetchBlockRequest {
                                r#ref: vec![BlockRef {
                                    hash: header.hash.clone(),
                                    index: header.slot,
                                }],
                                ..Default::default()
                            })
                            .await
                            .unwrap();
                        let fetch_block_response = response.into_inner();
                        let body = match &fetch_block_response.block.first().unwrap().chain {
                            Some(chain) => match chain {
                                Chain::Cardano(block) => block.body.clone(),
                            },
                            None => None,
                        };

                        let chainblock = ChainBlock {
                            slot: header.slot,
                            hash: header.hash.to_vec(),
                            number: header.height,
                            tx_count,
                            body,
                        };

                        self.send(Event::App(AppEvent::NewTip(chainblock)))?;
                        self.check_balances(&mut balances).await?;
                    }
                    TipEvent::Undo(block) => {
                        let header = block.parsed.clone().unwrap().header.unwrap();
                        let tx_count = match block.parsed {
                            Some(parsed) => match parsed.body {
                                Some(body) => body.tx.len(),
                                None => 0,
                            },
                            None => 0,
                        };
                        let chainblock = ChainBlock {
                            slot: header.slot,
                            hash: header.hash.to_vec(),
                            number: header.height,
                            tx_count,
                            body: None,
                        };
                        self.send(Event::App(AppEvent::UndoTip(chainblock)))?;
                        self.check_balances(&mut balances).await?;
                    }
                    TipEvent::Reset(point) => {
                        self.send(Event::App(AppEvent::Reset(point.index)))?;
                        self.check_balances(&mut balances).await?;
                    }
                }
            }
        }
    }
}
