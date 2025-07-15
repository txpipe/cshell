use std::{collections::HashMap, fmt::Display, sync::Arc, time::Duration};

use backoff::{backoff::Backoff, ExponentialBackoff};
use futures::{FutureExt, StreamExt};
use miette::{Context, IntoDiagnostic};
use pallas::ledger::addresses::Address;
use ratatui::crossterm::event::Event as CrosstermEvent;
use tokio::{
    sync::{mpsc, RwLock},
    time::sleep,
};
use utxorpc::{CardanoSyncClient, TipEvent};

use crate::types::DetailedBalance;

use super::{ChainBlock, ExplorerContext};

#[derive(Clone, Debug)]
pub enum Event {
    Crossterm(CrosstermEvent),
    App(AppEvent),
    Tick,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConnectionState {
    Connected,
    Retrying,
    Disconnected,
}
impl Display for ConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionState::Connected => write!(f, "connected"),
            ConnectionState::Retrying => write!(f, "retrying"),
            ConnectionState::Disconnected => write!(f, "disconnected"),
        }
    }
}

#[derive(Clone, Debug)]
pub enum AppEvent {
    Reset(u64),
    NewTip(ChainBlock),
    UndoTip(ChainBlock),
    BalanceUpdate((String, DetailedBalance)),
    State(ConnectionState),
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
    state: RwLock<ConnectionState>,
}

impl EventTask {
    fn new(sender: mpsc::UnboundedSender<Event>, context: Arc<ExplorerContext>) -> Self {
        Self {
            sender,
            context,
            state: RwLock::new(ConnectionState::Disconnected),
        }
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

        let follow_tip = async { self.run_follow_tip().await };

        tokio::try_join!(sender, keys(), follow_tip, ticks())?;
        Ok(())
    }

    fn send(&self, event: Event) -> miette::Result<()> {
        self.sender
            .send(event)
            .into_diagnostic()
            .context("sending event")
    }

    async fn get_balance(&self, address: &Address) -> miette::Result<DetailedBalance> {
        self.context.provider.get_detailed_balance(address).await
    }

    async fn check_balances(
        &self,
        balances: &mut HashMap<String, DetailedBalance>,
    ) -> miette::Result<()> {
        for (address, _) in self.context.wallets.read().await.iter() {
            let new = self.get_balance(address).await?;
            let key = address.to_string();
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

    async fn update_connection(&self, connection: ConnectionState) -> miette::Result<()> {
        *self.state.write().await = connection.clone();
        self.send(Event::App(AppEvent::State(connection)))
    }

    async fn run_follow_tip(&self) -> miette::Result<()> {
        let max_elapsed_time = Duration::from_secs(60 * 5);

        let mut backoff = ExponentialBackoff {
            max_elapsed_time: Some(max_elapsed_time),
            ..Default::default()
        };

        loop {
            if self.follow_tip().await.is_err() {
                if self.state.read().await.clone() == ConnectionState::Connected {
                    backoff = ExponentialBackoff {
                        max_elapsed_time: Some(max_elapsed_time),
                        ..Default::default()
                    };
                }

                self.update_connection(ConnectionState::Retrying).await?;

                if let Some(duration) = backoff.next_backoff() {
                    sleep(duration).await;
                } else {
                    self.update_connection(ConnectionState::Disconnected)
                        .await?;
                    break;
                }
            }
        }

        Ok(())
    }

    async fn follow_tip(&self) -> miette::Result<()> {
        let mut balances = HashMap::new();

        for (address, _) in self.context.wallets.read().await.iter() {
            let value = self.get_balance(address).await?;
            let key = address.to_string();
            self.send(Event::App(AppEvent::BalanceUpdate((
                key.clone(),
                value.clone(),
            ))))?;
            balances.insert(key, value);
        }

        let mut client: CardanoSyncClient = self.context.provider.client().await?;
        let mut tip = client.follow_tip(vec![]).await.into_diagnostic()?;

        self.update_connection(ConnectionState::Connected).await?;

        while let Some(event) = tip.event().await.into_diagnostic()? {
            match event {
                TipEvent::Apply(block) => {
                    let header = block.parsed.clone().unwrap().header.unwrap();
                    let body = block.parsed.and_then(|b| b.body);
                    let tx_count = body.as_ref().map_or(0, |b| b.tx.len());

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
                    let tx_count = block.parsed.and_then(|p| p.body).map_or(0, |b| b.tx.len());

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

        Err(miette::miette!("Tip stream ended unexpectedly"))
    }
}
