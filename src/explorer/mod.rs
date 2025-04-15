use std::{
    io,
    sync::Arc,
    time::{Duration, Instant},
};

use clap::Parser;
use miette::{bail, IntoDiagnostic};
use ratatui::{
    buffer::Buffer,
    crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind},
    layout::{Constraint, Layout},
    widgets::Widget,
    DefaultTerminal,
};

use crate::{provider::types::Provider, store::Store, Context};

pub mod tabs;
pub mod widgets;

use tabs::SelectedTab;
use widgets::{activity::ActivityMonitor, header::Header};

#[derive(Parser)]
pub struct Args {}

#[derive(PartialEq)]
pub enum AppState {
    Running,
    Done,
}

pub struct App {
    state: AppState,
    selected_tab: SelectedTab,
}
impl App {
    pub fn run(mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        let tick_rate = Duration::from_millis(500);
        let mut last_tick = Instant::now();
        while self.state == AppState::Running {
            terminal.draw(|frame| frame.render_widget(&self, frame.area()))?;
            let timeout = tick_rate.saturating_sub(last_tick.elapsed());
            if event::poll(timeout)? {
                if let event::Event::Key(key) = event::read()? {
                    self.handle_key(key)
                }
            }

            if last_tick.elapsed() >= tick_rate {
                // self.on_tick();
                last_tick = Instant::now();
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.state = AppState::Done,
            KeyCode::Tab => self.select_next_tab(),
            KeyCode::BackTab => self.select_previous_tab(),
            _ => {}
        }

        if let SelectedTab::Accounts(tab) = &mut self.selected_tab {
            match key.code {
                KeyCode::Char('j') | KeyCode::Down => tab.select_next(),
                KeyCode::Char('k') | KeyCode::Up => tab.select_previous(),
                KeyCode::Char('g') | KeyCode::Home => tab.select_first(),
                KeyCode::Char('G') | KeyCode::End => tab.select_last(),
                _ => {}
            }
        }
    }

    fn select_previous_tab(&mut self) {
        self.selected_tab = self.selected_tab.previous();
    }

    fn select_next_tab(&mut self) {
        self.selected_tab = self.selected_tab.next();
    }
}

impl Widget for &App {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut Buffer) {
        let [header_area, sparkline_area, inner_area] = Layout::vertical([
            Constraint::Length(5), // Header
            Constraint::Length(5), // Sparkline
            Constraint::Fill(1),   // Rest
        ])
        .areas(area);

        let header = Header::new(self.selected_tab.clone());
        header.render(header_area, buf);

        let sparkline = ActivityMonitor::new();
        sparkline.render(sparkline_area, buf);

        self.selected_tab.clone().render(inner_area, buf);
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
            selected_tab: SelectedTab::Accounts(widgets::accounts_tab::AccountsTab::new(
                context.clone(),
            )),
            state: AppState::Running,
        })
    }
}

pub fn run(_args: Args, ctx: &Context) -> miette::Result<()> {
    let mut terminal = ratatui::init();
    let app = App::try_from(ctx)?;
    let result = app.run(&mut terminal);
    ratatui::restore();
    result.into_diagnostic()
}
