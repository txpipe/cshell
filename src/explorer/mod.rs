use std::{collections::HashMap, sync::Arc};

use chrono::{DateTime, Utc};
use clap::Parser;
use miette::{bail, Context as _, IntoDiagnostic};
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind},
    layout::{Constraint, Layout},
    DefaultTerminal, Frame,
};
use strum::Display;

use crate::{provider::types::Provider, store::Store, types::DetailedBalance, Context};

pub mod event;
pub mod widgets;

use event::{AppEvent, Event, EventHandler};
use widgets::{
    accounts_tab::{AccountsTab, AccountsTabState},
    activity::ActivityMonitor,
    blocks_tab::{BlocksTab, BlocksTabState},
    footer::Footer,
    header::Header,
    help::HelpPopup,
    transactions_tab::TransactionsTab,
};

#[derive(Default)]
pub struct ChainState {
    pub tip: Option<u64>,
    pub balances: HashMap<String, DetailedBalance>,
    pub blocks: Vec<ChainBlock>,
    pub last_block_seen: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
pub struct ChainBlock {
    pub slot: u64,
    pub hash: Vec<u8>,
    pub number: u64,
    pub tx_count: usize,
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
    should_show_help: bool,
    selected_tab: SelectedTab,
    chain: ChainState,
    accounts_tab_state: AccountsTabState,
    blocks_tab_state: BlocksTabState,
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
                KeyCode::Char('q') | KeyCode::Esc => {
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

            if let SelectedTab::Accounts(_) = &mut self.selected_tab {
                match key.code {
                    KeyCode::Char('l') | KeyCode::Right => {
                        if self.accounts_tab_state.list_state.selected().is_some() {
                            self.accounts_tab_state.focus_on_table = true;
                            self.accounts_tab_state.table_state.select_next();
                        }
                    }
                    KeyCode::Char('h') | KeyCode::Left => {
                        if self.accounts_tab_state.focus_on_table {
                            self.accounts_tab_state.focus_on_table = false;
                            self.accounts_tab_state.table_state.select(None);
                        }
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        if self.accounts_tab_state.focus_on_table {
                            self.accounts_tab_state.table_state.select_next()
                        } else {
                            self.accounts_tab_state.list_state.select_next()
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if self.accounts_tab_state.focus_on_table {
                            self.accounts_tab_state.table_state.select_previous()
                        } else {
                            self.accounts_tab_state.list_state.select_previous()
                        }
                    }
                    _ => {}
                }
            }

            if let SelectedTab::Blocks(_) = &mut self.selected_tab {
                match key.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        self.blocks_tab_next_row();
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        self.blocks_tab_previous_row();
                    }
                    _ => {}
                }
            }
        } else {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => self.should_show_help = false,
                _ => {}
            }
        }
    }

    fn handle_tick(&mut self) {
        // self.activity_monitor.points.push_front(0);
        // self.activity_monitor.points.pop_back();
        self.activity_monitor.points.push_back(0);
        self.activity_monitor.points.pop_front();
    }

    fn handle_reset(&mut self, tip: u64) {
        self.chain.tip = Some(tip);
        self.chain.last_block_seen = Some(Utc::now());
        self.activity_monitor = ActivityMonitor::from(&*self);
    }

    fn handle_new_tip(&mut self, tip: ChainBlock) {
        self.chain.tip = Some(tip.slot);
        self.chain.last_block_seen = Some(Utc::now());
        self.chain.blocks.push(tip);
        self.activity_monitor = ActivityMonitor::from(&*self);
        self.blocks_tab_state.scroll_state = self
            .blocks_tab_state
            .scroll_state
            .content_length(self.chain.blocks.len() * 3 - 2);
        self.selected_tab = match &self.selected_tab {
            SelectedTab::Blocks(_) => SelectedTab::Blocks(BlocksTab::from(&*self)),
            x => x.clone(),
        }
    }

    fn handle_undo_tip(&mut self, tip: ChainBlock) {
        self.chain.tip = Some(tip.slot);
        self.chain.last_block_seen = Some(Utc::now());
        self.chain.blocks = self
            .chain
            .blocks
            .clone()
            .into_iter()
            .filter(|block| block.number >= tip.number)
            .collect();
        self.activity_monitor = ActivityMonitor::from(&*self);
        self.blocks_tab_state.scroll_state = self
            .blocks_tab_state
            .scroll_state
            .content_length(self.chain.blocks.len() * 3 - 2);
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
            SelectedTab::Accounts(_) => SelectedTab::Transactions(TransactionsTab {}),
            SelectedTab::Blocks(_) => SelectedTab::Accounts(AccountsTab::from(&*self)),
            SelectedTab::Transactions(_) => SelectedTab::Blocks(BlocksTab::from(&*self)),
        }
    }

    fn select_next_tab(&mut self) {
        self.selected_tab = match &self.selected_tab {
            SelectedTab::Accounts(_) => SelectedTab::Blocks(BlocksTab::from(&*self)),
            SelectedTab::Blocks(_) => SelectedTab::Transactions(TransactionsTab {}),
            SelectedTab::Transactions(_) => SelectedTab::Accounts(AccountsTab::from(&*self)),
        }
    }

    pub fn blocks_tab_next_row(&mut self) {
        let i = match self.blocks_tab_state.table_state.selected() {
            Some(i) => {
                if i >= self.chain.blocks.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.blocks_tab_state.table_state.select(Some(i));
        self.blocks_tab_state.scroll_state = self.blocks_tab_state.scroll_state.position(i * 3);
    }

    pub fn blocks_tab_previous_row(&mut self) {
        let i = match self.blocks_tab_state.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.chain.blocks.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.blocks_tab_state.table_state.select(Some(i));
        self.blocks_tab_state.scroll_state = self.blocks_tab_state.scroll_state.position(i * 3);
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
            SelectedTab::Transactions(transactions_tab) => {
                frame.render_widget(transactions_tab, inner_area)
            }
        }

        frame.render_widget(Footer::new(), footer_area);

        if self.should_show_help {
            frame.render_widget(HelpPopup::new(), frame.area());
        }
    }
}

pub struct ExplorerContext {
    pub store: Store,
    pub provider: Provider,
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

        Ok(Self {
            store: ctx.store.clone(),
            provider,
        })
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
            should_show_help: false,
            chain: ChainState::default(),
            events: EventHandler::new(context.clone()),
            accounts_tab_state: AccountsTabState::default(),
            blocks_tab_state: BlocksTabState::default(),
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
