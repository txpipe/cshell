use std::{collections::HashMap, sync::Arc};

use futures::{FutureExt, StreamExt};
use miette::{Context, IntoDiagnostic};
use ratatui::crossterm::event::Event as CrosstermEvent;
use tokio::sync::mpsc;
use utxorpc::{CardanoSyncClient, TipEvent};

use crate::{provider::types::Provider, types::DetailedBalance, wallet::types::Wallet};

use super::ExplorerContext;

#[derive(Clone, Debug)]
pub enum Event {
    Crossterm(CrosstermEvent),
    App(AppEvent),
}

#[derive(Clone, Debug)]
pub enum AppEvent {
    NewTip(Option<u64>),
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

        let follow_tip = async { self.follow_tip().await };

        tokio::try_join!(sender, keys(), follow_tip)?;
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
        let provider = match &self.context.provider {
            Provider::UTxORPC(provider) => provider,
            #[allow(unreachable_patterns)]
            _ => return Ok(()),
        };

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
            let mut client: CardanoSyncClient = provider.client().await?;
            let mut tip = client.follow_tip(vec![]).await.unwrap();

            while let Ok(event) = tip.event().await {
                match event {
                    TipEvent::Apply(block) => {
                        self.send(Event::App(AppEvent::NewTip(Some(
                            block.parsed.unwrap().header.unwrap().slot,
                        ))))?;
                        self.check_balances(&mut balances).await?;
                    }
                    TipEvent::Undo(block) => {
                        self.send(Event::App(AppEvent::NewTip(Some(
                            block.parsed.unwrap().header.unwrap().slot,
                        ))))?;
                        self.check_balances(&mut balances).await?;
                    }
                    TipEvent::Reset(point) => {
                        self.send(Event::App(AppEvent::NewTip(Some(point.index))))?;
                        self.check_balances(&mut balances).await?;
                    }
                }
            }
        }
    }
}
