use std::{cell::RefCell, collections::VecDeque, rc::Rc, sync::Arc};

use anyhow::{bail, Context as _, Result};
use chrono::{DateTime, Utc};
use clap::Parser;
use indexmap::IndexMap;
use pallas::ledger::addresses::Address;
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind},
    layout::{Constraint, Layout},
    DefaultTerminal, Frame,
};
use strum::Display;
use tokio::sync::RwLock;
use utxorpc::spec::cardano::BlockBody;

use crate::{provider::types::Provider, types::DetailedBalance, utils::Name, Context};

pub mod event;
pub mod widgets;

use event::{AppEvent, ConnectionState, Event, EventHandler};
use widgets::{
    activity::ActivityMonitor,
    footer::Footer,
    header::Header,
    popups::{help::HelpPopup, new_address::NewViewAddress},
    tabs::{
        accounts::{AccountsTab, AccountsTabState},
        blocks::{BlocksTab, BlocksTabState},
        transactions::{TransactionsTab, TransactionsTabState},
    },
};

#[derive(Parser)]
pub struct Args {
    #[arg(long, help = "Name of the provider to use")]
    provider: Option<String>,
}

#[derive(Default)]
pub struct ChainState {
    pub tip: Option<u64>,
    // TODO: add a capacity to not have problems with memory
    pub blocks: Rc<RefCell<VecDeque<ChainBlock>>>,
    pub last_block_seen: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
pub struct ChainBlock {
    pub slot: u64,
    pub hash: Vec<u8>,
    pub number: u64,
    pub tx_count: usize,
    pub body: Option<BlockBody>,
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

#[derive(Clone)]
pub enum SelectedPopup {
    Help(HelpPopup),
    NewViewAddress(NewViewAddress),
}

pub struct App {
    done: bool,
    app_state: ConnectionState,

    selected_tab: SelectedTab,
    selected_popup: Option<SelectedPopup>,

    chain: ChainState,
    accounts_tab_state: AccountsTabState,
    blocks_tab_state: BlocksTabState,
    transactions_tab_state: TransactionsTabState,
    activity_monitor: ActivityMonitor,
    pub events: EventHandler,
    pub context: Arc<ExplorerContext>,
}
impl App {
    pub fn new(context: Arc<ExplorerContext>) -> Self {
        Self {
            context: context.clone(),

            selected_tab: SelectedTab::Accounts(AccountsTab::new(context.clone())),
            selected_popup: None,

            activity_monitor: ActivityMonitor::default(),
            done: false,
            app_state: ConnectionState::Disconnected,

            chain: ChainState::default(),
            events: EventHandler::new(context.clone()),
            accounts_tab_state: AccountsTabState::default(),
            blocks_tab_state: BlocksTabState::default(),
            transactions_tab_state: TransactionsTabState::new(Arc::clone(&context)),
        }
    }

    /// Run the application's main loop.
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        while !self.done {
            terminal
                .draw(|frame| self.draw(frame))
                .context("rendering")?;

            match self.events.next().await? {
                Event::Crossterm(event) => {
                    if let crossterm::event::Event::Key(key_event) = event {
                        self.handle_key(key_event).await
                    }
                }
                Event::App(app_event) => match app_event {
                    AppEvent::Reset(tip) => self.handle_reset(tip),
                    AppEvent::NewTip(tip) => self.handle_new_tip(tip),
                    AppEvent::UndoTip(tip) => self.handle_undo_tip(tip),
                    AppEvent::State(app_state) => self.app_state = app_state,
                },
                Event::Tick => self.handle_tick(),
            }
        }
        Ok(())
    }

    async fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        if let Some(popup) = &mut self.selected_popup {
            if key.code == KeyCode::Esc {
                self.selected_popup = None;
                return;
            }

            match popup {
                SelectedPopup::NewViewAddress(widget) => widget.handle_key(&key).await,
                SelectedPopup::Help(_) => {}
            }
        } else {
            match key.code {
                KeyCode::Char('q') => self.done = true,
                KeyCode::Tab | KeyCode::BackTab if self.selected_popup.is_none() => {
                    if key.code == KeyCode::Tab {
                        self.select_next_tab()
                    } else {
                        self.select_previous_tab()
                    }
                }
                KeyCode::Char('?') => {
                    self.selected_popup = Some(SelectedPopup::Help(HelpPopup::new()))
                }

                _ => {}
            }

            match self.selected_tab {
                SelectedTab::Accounts(_) => match key.code {
                    KeyCode::Char('i') => {
                        self.selected_popup = Some(SelectedPopup::NewViewAddress(
                            NewViewAddress::new(self.context.clone()),
                        ))
                    }
                    _ => self.accounts_tab_state.handle_key(&key),
                },
                SelectedTab::Blocks(_) => self.blocks_tab_state.handle_key(&key),
                SelectedTab::Transactions(_) => self.transactions_tab_state.handle_key(&key).await,
            }
        }
    }

    fn handle_tick(&mut self) {}

    fn handle_reset(&mut self, tip: u64) {
        self.chain.tip = Some(tip);
        self.chain.last_block_seen = Some(Utc::now());
        self.activity_monitor = ActivityMonitor::from(&*self);
    }

    fn handle_new_tip(&mut self, tip: ChainBlock) {
        self.chain.tip = Some(tip.slot);
        self.chain.last_block_seen = Some(Utc::now());
        self.chain.blocks.borrow_mut().push_front(tip);

        self.activity_monitor = ActivityMonitor::from(&*self);

        self.blocks_tab_state
            .update_scroll_state(self.chain.blocks.borrow().len());

        self.transactions_tab_state
            .update_blocks(Rc::clone(&self.chain.blocks));

        self.selected_tab = match &self.selected_tab {
            SelectedTab::Blocks(_) => SelectedTab::Blocks(BlocksTab::from(&*self)),
            x => x.clone(),
        }
    }

    fn handle_undo_tip(&mut self, tip: ChainBlock) {
        self.chain.tip = Some(tip.slot);
        self.chain.last_block_seen = Some(Utc::now());

        self.chain
            .blocks
            .borrow_mut()
            .retain(|block| block.number >= tip.number);

        self.activity_monitor = ActivityMonitor::from(&*self);

        self.blocks_tab_state
            .update_scroll_state(self.chain.blocks.borrow().len());

        self.transactions_tab_state
            .update_blocks(Rc::clone(&self.chain.blocks));

        self.selected_tab = match &self.selected_tab {
            SelectedTab::Blocks(_) => SelectedTab::Blocks(BlocksTab::from(&*self)),
            x => x.clone(),
        }
    }

    fn select_previous_tab(&mut self) {
        self.selected_tab = match &self.selected_tab {
            SelectedTab::Accounts(_) => SelectedTab::Transactions(TransactionsTab {}),
            SelectedTab::Blocks(_) => SelectedTab::Accounts(AccountsTab::new(self.context.clone())),
            SelectedTab::Transactions(_) => SelectedTab::Blocks(BlocksTab::from(&*self)),
        }
    }

    fn select_next_tab(&mut self) {
        self.selected_tab = match &self.selected_tab {
            SelectedTab::Accounts(_) => SelectedTab::Blocks(BlocksTab::from(&*self)),
            SelectedTab::Blocks(_) => SelectedTab::Transactions(TransactionsTab {}),
            SelectedTab::Transactions(_) => {
                SelectedTab::Accounts(AccountsTab::new(self.context.clone()))
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let [header_area, sparkline_area, inner_area, footer_area] = Layout::vertical([
            Constraint::Length(5), // Header
            Constraint::Length(5), // Sparkline
            Constraint::Fill(1),   // Rest
            Constraint::Length(1), // Footer
        ])
        .areas(frame.area());

        let header = Header::from(&*self);
        frame.render_widget(header, header_area);

        frame.render_widget(self.activity_monitor.clone(), sparkline_area);

        match self.selected_tab.clone() {
            SelectedTab::Accounts(accounts_tab) => {
                frame.render_stateful_widget(
                    accounts_tab,
                    inner_area,
                    &mut self.accounts_tab_state,
                );
            }
            SelectedTab::Blocks(blocks_tab) => {
                frame.render_stateful_widget(blocks_tab, inner_area, &mut self.blocks_tab_state)
            }
            SelectedTab::Transactions(transactions_tab) => frame.render_stateful_widget(
                transactions_tab,
                inner_area,
                &mut self.transactions_tab_state,
            ),
        }
        frame.render_widget(Footer::new(), footer_area);

        if let Some(popup) = self.selected_popup.clone() {
            match popup {
                SelectedPopup::Help(widget) => frame.render_widget(widget, frame.area()),
                SelectedPopup::NewViewAddress(widget) => frame.render_widget(widget, frame.area()),
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExplorerWallet {
    pub name: Name,
    pub balance: DetailedBalance,
}
impl ExplorerWallet {
    pub fn new(name: Name) -> Self {
        Self {
            name,
            balance: Default::default(),
        }
    }
}

pub struct ExplorerContext {
    pub provider: Provider,
    pub wallets: RwLock<IndexMap<Address, ExplorerWallet>>,
}
impl ExplorerContext {
    pub fn new(args: &Args, ctx: &Context) -> Result<Self> {
        let provider = match &args.provider {
            Some(name) => match ctx.store.find_provider(name) {
                Some(provider) => provider.clone(),
                None => bail!("Provider not found."),
            },
            None => match ctx.store.default_provider() {
                Some(provider) => provider.clone(),
                None => match ctx.store.providers().first() {
                    Some(provider) => provider.clone(),
                    None => bail!("No providers configured"),
                },
            },
        };

        let wallets = RwLock::new(
            ctx.store
                .wallets()
                .iter()
                .map(|w| {
                    (
                        w.address(provider.is_testnet()),
                        ExplorerWallet::new(w.name.clone()),
                    )
                })
                .collect::<IndexMap<_, _>>(),
        );

        Ok(Self { provider, wallets })
    }

    pub async fn insert_wallet(&self, address: Address, name: Name) {
        let balance = self
            .provider
            .get_detailed_balance(&address)
            .await
            .unwrap_or_default();

        let mut wallet = ExplorerWallet::new(name);
        wallet.balance = balance;

        self.wallets.write().await.insert(address.clone(), wallet);
    }
}

pub async fn run(args: Args, ctx: &Context) -> Result<()> {
    let terminal = ratatui::init();

    let context: Arc<ExplorerContext> = Arc::new(ExplorerContext::new(&args, ctx)?);
    let app = App::new(context.clone());
    let result = app.run(terminal).await;
    ratatui::restore();
    result
}
