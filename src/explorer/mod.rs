use std::{collections::HashMap, sync::Arc};

use clap::Parser;
use event::{AppEvent, Event, EventHandler};
use miette::{bail, Context as _, IntoDiagnostic};
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind},
    layout::{Constraint, Layout},
    widgets::ListState,
    DefaultTerminal, Frame,
};
use strum::Display;

use crate::{provider::types::Provider, store::Store, types::DetailedBalance, Context};

pub mod event;
pub mod widgets;

use widgets::{
    accounts_tab::AccountsTab, activity::ActivityMonitor, blocks_tab::BlocksTab, header::Header,
    transactions_tab::TransactionsTab,
};

#[derive(Default)]
pub struct ChainState {
    pub tip: Option<u64>,
    pub balances: HashMap<String, DetailedBalance>,
}

#[derive(Clone, Display)]
pub enum SelectedTab {
    #[strum(to_string = "Accounts")]
    Accounts(AccountsTab),
    #[strum(to_string = "Blocks")]
    Blocks(BlocksTab),
    #[strum(to_string = "Txs")]
    Transactions(TransactionsTab),
}

#[derive(Parser)]
pub struct Args {}

pub struct App {
    done: bool,
    selected_tab: SelectedTab,
    chain: ChainState,
    accounts_tab_list_state: ListState,
    pub events: EventHandler,
    pub context: Arc<ExplorerContext>,
}
impl App {
    /// Run the application's main loop.
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> miette::Result<()> {
        while !self.done {
            terminal
                .draw(|frame| self.draw(frame))
                .into_diagnostic()
                .context("rendering")?;
            match self.events.next().await? {
                Event::Crossterm(event) => {
                    if let crossterm::event::Event::Key(key_event) = event {
                        self.handle_key(key_event)
                    }
                }
                Event::App(app_event) => match app_event {
                    AppEvent::NewTip(tip) => self.handle_new_tip(tip),
                    AppEvent::BalanceUpdate((address, balance)) => {
                        self.handle_balance_update(address, balance)
                    }
                },
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.done = true,
            KeyCode::Tab => self.select_next_tab(),
            KeyCode::BackTab => self.select_previous_tab(),
            _ => {}
        }

        if let SelectedTab::Accounts(_) = &mut self.selected_tab {
            match key.code {
                KeyCode::Char('j') | KeyCode::Down => self.accounts_tab_list_state.select_next(),
                KeyCode::Char('k') | KeyCode::Up => self.accounts_tab_list_state.select_previous(),
                KeyCode::Char('g') | KeyCode::Home => self.accounts_tab_list_state.select_first(),
                KeyCode::Char('G') | KeyCode::End => self.accounts_tab_list_state.select_last(),
                _ => {}
            }
        }
    }

    fn handle_new_tip(&mut self, tip: Option<u64>) {
        self.chain.tip = tip;
    }

    fn handle_balance_update(&mut self, key: String, balance: DetailedBalance) {
        self.chain.balances.insert(key, balance);
        self.selected_tab = match &self.selected_tab {
            SelectedTab::Accounts(_) => SelectedTab::Accounts(AccountsTab::from(&*self)),
            x => x.clone(),
        }
    }

    fn select_previous_tab(&mut self) {
        self.selected_tab = match &self.selected_tab {
            SelectedTab::Accounts(tab) => {
                SelectedTab::Transactions(TransactionsTab::new(tab.context.clone()))
            }
            SelectedTab::Blocks(_) => SelectedTab::Accounts(AccountsTab::from(&*self)),
            SelectedTab::Transactions(tab) => {
                SelectedTab::Blocks(BlocksTab::new(tab.context.clone()))
            }
        }
    }

    fn select_next_tab(&mut self) {
        self.selected_tab = match &self.selected_tab {
            SelectedTab::Accounts(tab) => SelectedTab::Blocks(BlocksTab::new(tab.context.clone())),
            SelectedTab::Blocks(tab) => {
                SelectedTab::Transactions(TransactionsTab::new(tab.context.clone()))
            }
            SelectedTab::Transactions(_) => SelectedTab::Accounts(AccountsTab::from(&*self)),
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let [header_area, sparkline_area, inner_area] = Layout::vertical([
            Constraint::Length(5), // Header
            Constraint::Length(5), // Sparkline
            Constraint::Fill(1),   // Rest
        ])
        .areas(frame.area());

        let header = Header::from(&*self);
        frame.render_widget(header, header_area);

        let activity = ActivityMonitor::new();
        frame.render_widget(activity, sparkline_area);

        match self.selected_tab.clone() {
            SelectedTab::Accounts(accounts_tab) => {
                frame.render_stateful_widget(
                    accounts_tab,
                    inner_area,
                    &mut self.accounts_tab_list_state,
                );
            }
            SelectedTab::Blocks(blocks_tab) => frame.render_widget(blocks_tab, inner_area),
            SelectedTab::Transactions(transactions_tab) => {
                frame.render_widget(transactions_tab, inner_area)
            }
        }
    }
}

pub struct ExplorerContext {
    pub store: Store,
    pub provider: Provider,
}
impl TryFrom<&Context> for ExplorerContext {
    type Error = miette::Error;
    fn try_from(value: &Context) -> Result<Self, Self::Error> {
        let provider = match value.store.default_provider() {
            Some(provider) => provider,
            None => match value.store.providers().first() {
                Some(provider) => provider,
                None => bail!("No providers configured"),
            },
        };
        Ok(Self {
            store: value.store.clone(),
            provider: provider.clone(),
        })
    }
}
impl TryFrom<&Context> for App {
    type Error = miette::Error;
    fn try_from(value: &Context) -> Result<Self, Self::Error> {
        let context: Arc<ExplorerContext> = Arc::new(value.try_into()?);
        Ok(Self {
            context: context.clone(),
            selected_tab: SelectedTab::Accounts(widgets::accounts_tab::AccountsTab {
                context: context.clone(),
                balances: Default::default(),
            }),
            done: false,
            chain: ChainState::default(),
            events: EventHandler::new(context.clone()),
            accounts_tab_list_state: ListState::default(),
        })
    }
}

pub async fn run(_args: Args, ctx: &Context) -> miette::Result<()> {
    let terminal = ratatui::init();
    let app = App::try_from(ctx)?;
    let result = app.run(terminal).await;
    ratatui::restore();
    result
}
