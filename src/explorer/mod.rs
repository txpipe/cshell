use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    rc::Rc,
    sync::Arc,
};

use chrono::{DateTime, Utc};
use clap::Parser;
use miette::{bail, Context as _, IntoDiagnostic};
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
    accounts_tab::{AccountsTab, AccountsTabState},
    activity::ActivityMonitor,
    blocks_tab::{BlocksTab, BlocksTabState},
    footer::Footer,
    header::Header,
    help::HelpPopup,
    transactions_tab::{TransactionsTab, TransactionsTabState},
};

#[derive(Default)]
pub struct ChainState {
    pub tip: Option<u64>,
    pub balances: HashMap<String, DetailedBalance>,
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

#[derive(Parser)]
pub struct Args {
    #[arg(long, help = "Name of the provider to use")]
    provider: Option<String>,
}

pub struct App {
    done: bool,
    app_state: ConnectionState,
    should_show_help: bool,
    selected_tab: SelectedTab,
    chain: ChainState,
    accounts_tab_state: AccountsTabState,
    blocks_tab_state: BlocksTabState,
    transactions_tab_state: TransactionsTabState,
    activity_monitor: ActivityMonitor,
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
                    AppEvent::Reset(tip) => self.handle_reset(tip),
                    AppEvent::NewTip(tip) => self.handle_new_tip(tip),
                    AppEvent::UndoTip(tip) => self.handle_undo_tip(tip),
                    AppEvent::BalanceUpdate((address, balance)) => {
                        self.handle_balance_update(address, balance)
                    }
                    AppEvent::State(app_state) => self.app_state = app_state,
                },
                Event::Tick => self.handle_tick(),
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        if !self.should_show_help {
            match key.code {
                KeyCode::Char('q') => {
                    if self.should_show_help {
                        self.should_show_help = false
                    } else {
                        self.done = true
                    }
                }
                KeyCode::Tab => self.select_next_tab(),
                KeyCode::BackTab => self.select_previous_tab(),
                KeyCode::Char('?') => self.should_show_help = true,
                _ => {}
            }

            match self.selected_tab {
                SelectedTab::Accounts(_) => self.accounts_tab_state.handle_key(&key),
                SelectedTab::Blocks(_) => self.blocks_tab_state.handle_key(&key),
                SelectedTab::Transactions(_) => self.transactions_tab_state.handle_key(&key),
            }
        } else {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => self.should_show_help = false,
                _ => {}
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
            .update_scroll_state(self.chain.blocks.borrow().iter().map(|b| b.tx_count).sum());

        self.selected_tab = match &self.selected_tab {
            SelectedTab::Blocks(_) => SelectedTab::Blocks(BlocksTab::from(&*self)),
            SelectedTab::Transactions(_) => {
                SelectedTab::Transactions(TransactionsTab::from(&*self))
            }
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
            .update_scroll_state(self.chain.blocks.borrow().iter().map(|b| b.tx_count).sum());

        self.selected_tab = match &self.selected_tab {
            SelectedTab::Blocks(_) => SelectedTab::Blocks(BlocksTab::from(&*self)),
            x => x.clone(),
        }
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
            SelectedTab::Accounts(_) => SelectedTab::Transactions(TransactionsTab::from(&*self)),
            SelectedTab::Blocks(_) => SelectedTab::Accounts(AccountsTab::from(&*self)),
            SelectedTab::Transactions(_) => SelectedTab::Blocks(BlocksTab::from(&*self)),
        }
    }

    fn select_next_tab(&mut self) {
        self.selected_tab = match &self.selected_tab {
            SelectedTab::Accounts(_) => SelectedTab::Blocks(BlocksTab::from(&*self)),
            SelectedTab::Blocks(_) => SelectedTab::Transactions(TransactionsTab::from(&*self)),
            SelectedTab::Transactions(_) => SelectedTab::Accounts(AccountsTab::from(&*self)),
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

        if self.should_show_help {
            frame.render_widget(HelpPopup::new(), frame.area());
        }
    }
}

pub struct ExplorerContext {
    pub provider: Provider,
    pub wallets: RwLock<HashMap<Address, Name>>,
}
impl TryFrom<(Args, &Context)> for ExplorerContext {
    type Error = miette::Error;
    fn try_from(value: (Args, &Context)) -> Result<Self, Self::Error> {
        let (args, ctx) = value;
        let provider = match args.provider {
            Some(name) => match ctx.store.find_provider(&name) {
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
                .map(|w| (w.address(provider.is_testnet()), w.name.clone()))
                .collect::<HashMap<_, _>>(),
        );

        Ok(Self { provider, wallets })
    }
}
impl TryFrom<(Args, &Context)> for App {
    type Error = miette::Error;
    fn try_from(value: (Args, &Context)) -> Result<Self, Self::Error> {
        let context: Arc<ExplorerContext> = Arc::new(value.try_into()?);
        Ok(Self {
            context: context.clone(),
            selected_tab: SelectedTab::Accounts(widgets::accounts_tab::AccountsTab {
                context: context.clone(),
                balances: Default::default(),
            }),
            activity_monitor: ActivityMonitor::default(),
            done: false,
            app_state: ConnectionState::Disconnected,
            should_show_help: false,
            chain: ChainState::default(),
            events: EventHandler::new(context.clone()),
            accounts_tab_state: AccountsTabState::default(),
            blocks_tab_state: BlocksTabState::default(),
            transactions_tab_state: TransactionsTabState::default(),
        })
    }
}

pub async fn run(args: Args, ctx: &Context) -> miette::Result<()> {
    let terminal = ratatui::init();
    let app = App::try_from((args, ctx))?;
    let result = app.run(terminal).await;
    ratatui::restore();
    result
}
