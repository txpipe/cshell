use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Margin, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::Text,
    widgets::{
        Block, Cell, HighlightSpacing, Row, Scrollbar, ScrollbarState, StatefulWidget, Table,
        TableState,
    },
};

use crate::explorer::{App, ChainBlock};

#[derive(Default)]
pub struct BlocksTabState {
    scroll_state: ScrollbarState,
    table_state: TableState,
}
impl BlocksTabState {
    pub fn handle_key(&mut self, key: &KeyEvent) {
        match (key.code, key.modifiers) {
            (KeyCode::Char('J') | KeyCode::Down, KeyModifiers::SHIFT) => {
                self.last_row();
            }
            (KeyCode::Char('j') | KeyCode::Down, _) => {
                self.next_row();
            }
            (KeyCode::Char('K') | KeyCode::Up, KeyModifiers::SHIFT) => {
                self.first_row();
            }
            (KeyCode::Char('k') | KeyCode::Up, _) => {
                self.previous_row();
            }
            _ => {}
        }
    }

    pub fn update_scroll_state(&mut self, len: usize) {
        self.scroll_state = self.scroll_state.content_length(len * 3 - 2)
    }

    fn next_row(&mut self) {
        let i = self.table_state.selected().map(|i| i + 1).unwrap_or(0);
        self.table_state.select(Some(i));
        self.scroll_state = self.scroll_state.position(i * 3);
    }

    fn previous_row(&mut self) {
        let i = self.table_state.selected().unwrap_or(0).saturating_sub(1);
        self.table_state.select(Some(i));
        self.scroll_state = self.scroll_state.position(i * 3);
    }

    fn first_row(&mut self) {
        self.table_state.select_first();
        if let Some(i) = self.table_state.selected() {
            self.scroll_state = self.scroll_state.position(i * 3);
        }
    }

    fn last_row(&mut self) {
        self.table_state.select_last();
        if let Some(i) = self.table_state.selected() {
            self.scroll_state = self.scroll_state.position(i);
        }
    }
}

#[derive(Clone)]
pub struct BlocksTab {
    blocks: Rc<RefCell<VecDeque<ChainBlock>>>,
}
impl From<&App> for BlocksTab {
    fn from(value: &App) -> Self {
        Self {
            blocks: Rc::clone(&value.chain.blocks),
        }
    }
}

impl StatefulWidget for BlocksTab {
    type State = BlocksTabState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State)
    where
        Self: Sized,
    {
        let header = ["Slot", "Hash", "Number", "Tx Count"]
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(Style::default().fg(Color::Green).bold())
            .height(1);

        let rows: Vec<Row> = self
            .blocks
            .borrow()
            .iter()
            .enumerate()
            .map(|(i, block)| {
                let color = match i % 2 {
                    0 => Color::Black,
                    _ => Color::Reset,
                };
                Row::new(vec![
                    format!("\n{}\n", block.slot),
                    format!("\n{}\n", hex::encode(&block.hash)),
                    format!("\n{}\n", block.number),
                    format!("\n{}\n", block.tx_count),
                ])
                .style(Style::new().fg(Color::White).bg(color))
                .height(3)
            })
            .collect();

        let bar = " █ ";
        let table = Table::new(
            rows,
            [
                Constraint::Length(12),
                Constraint::Fill(1),
                Constraint::Length(12),
                Constraint::Length(10),
            ],
        )
        .header(header)
        .row_highlight_style(Modifier::BOLD)
        // .column_highlight_style((Color::LightGreen, Modifier::BOLD))
        // .cell_highlight_style((Color::LightGreen, Modifier::BOLD))
        .highlight_symbol(Text::from(vec!["".into(), bar.into(), "".into()]))
        .highlight_spacing(HighlightSpacing::Always)
        .block(Block::bordered().title(" Blocks "));
        StatefulWidget::render(table, area, buf, &mut state.table_state);

        StatefulWidget::render(
            Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight),
            area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
            buf,
            &mut state.scroll_state,
        );
    }
}
