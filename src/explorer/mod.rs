use std::io;

use clap::Parser;
use miette::IntoDiagnostic;
use pallas::storage::hardano::immutable::primary::layout;
use ratatui::{
    crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind},
    layout::{Constraint, Direction, Layout},
    style::{
        palette::{
            material::{BLUE, GREEN},
            tailwind::SLATE,
        },
        Color, Modifier, Style, Stylize,
    },
    symbols::border,
    text::{Line, Text},
    widgets::{
        Block, HighlightSpacing, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Widget,
    },
    DefaultTerminal, Frame,
};

use crate::Context;

#[derive(Parser)]
pub struct Args {}

#[derive(Default)]
pub struct BlockList {}

impl Widget for BlockList {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
    }
}

const TODO_HEADER_STYLE: Style = Style::new().fg(SLATE.c100).bg(BLUE.c800);
const NORMAL_ROW_BG: Color = SLATE.c950;
const ALT_ROW_BG_COLOR: Color = SLATE.c900;
const SELECTED_STYLE: Style = Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD);
const TEXT_FG_COLOR: Color = SLATE.c200;
const COMPLETED_TEXT_FG_COLOR: Color = GREEN.c500;

const fn alternate_colors(i: usize) -> Color {
    if i % 2 == 0 {
        NORMAL_ROW_BG
    } else {
        ALT_ROW_BG_COLOR
    }
}

#[derive(Default)]
pub struct App {
    done: bool,
    list_state: ListState,
    items: Vec<String>,
}

impl App {
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        for i in 0..100 {
            self.items.push(format!("Item {}", i));
        }

        while !self.done {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(20), Constraint::Fill(1)])
            .split(frame.area());

        let menu = Block::default().title("Menu").border_set(border::THICK);
        //let scroll =
        // Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);

        let items = self
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let color = alternate_colors(i);
                ListItem::from(item.as_str()).bg(color)
            })
            .collect::<Vec<_>>();

        let list = List::new(items)
            .highlight_style(SELECTED_STYLE)
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        frame.render_widget(menu, layout[0]);
        frame.render_stateful_widget(list, layout[1], &mut self.list_state);
    }

    fn handle_events(&mut self) -> io::Result<()> {
        let event = event::read()?;

        if let event::Event::Key(key) = event {
            self.handle_key(key);
        };

        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.done = true,
            KeyCode::Char('h') | KeyCode::Left => self.select_none(),
            KeyCode::Char('j') | KeyCode::Down => self.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.select_previous(),
            KeyCode::Char('g') | KeyCode::Home => self.select_first(),
            KeyCode::Char('G') | KeyCode::End => self.select_last(),
            _ => {}
        }
    }

    fn select_none(&mut self) {
        self.list_state.select(None);
    }

    fn select_next(&mut self) {
        self.list_state.select_next();
    }
    fn select_previous(&mut self) {
        self.list_state.select_previous();
    }

    fn select_first(&mut self) {
        self.list_state.select_first();
    }

    fn select_last(&mut self) {
        self.list_state.select_last();
    }
}

pub fn run(args: Args, ctx: &mut Context) -> miette::Result<()> {
    let mut terminal = ratatui::init();
    let mut app = App::default();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result.into_diagnostic()
}
