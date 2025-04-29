use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::Text,
    widgets::{
        Block, Cell, HighlightSpacing, Paragraph, Row, Scrollbar, ScrollbarState, StatefulWidget,
        Table, TableState, Widget,
    },
};
use regex::Regex;
use utxorpc::spec::cardano::{self, Tx};

use crate::explorer::{App, ChainBlock};

#[derive(Default)]
pub struct TransactionsTabState {
    scroll_state: ScrollbarState,
    table_state: TableState,
    input: String,
    input_mode: InputMode,
    character_index: usize,
    view_mode: ViewMode,
}
impl TransactionsTabState {
    pub fn handle_key(&mut self, key: &KeyEvent) {
        match self.view_mode {
            ViewMode::Normal => match self.input_mode {
                InputMode::Normal => match (key.code, key.modifiers) {
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
                    (KeyCode::Char('f') | KeyCode::Char('/'), _) => {
                        self.input_mode = InputMode::Editing
                    }
                    (KeyCode::Esc, _) => {
                        if !self.input.is_empty() {
                            self.input.clear()
                        }
                    }
                    (KeyCode::Enter, _) => {
                        if self.table_state.selected().is_some() {
                            self.view_mode = ViewMode::Detail
                        }
                    }

                    _ => {}
                },
                InputMode::Editing => match key.code {
                    KeyCode::Char(to_insert) => self.search_enter_char(to_insert),
                    KeyCode::Esc => self.input_mode = InputMode::Normal,
                    KeyCode::Backspace => self.search_delete_char(),
                    KeyCode::Enter => {
                        if !self.input.is_empty() {
                            self.table_state.select_first();
                        }

                        self.input_mode = InputMode::Normal
                    }

                    _ => {}
                },
            },
            #[allow(clippy::single_match)]
            ViewMode::Detail => match key.code {
                KeyCode::Esc => self.view_mode = ViewMode::Normal,
                _ => {}
            },
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

    fn search_enter_char(&mut self, new_char: char) {
        let index = self
            .input
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.character_index)
            .unwrap_or(self.input.len());

        self.input.insert(index, new_char);
        self.search_move_cursor_right();
    }

    fn search_delete_char(&mut self) {
        let is_not_cursor_leftmost = self.character_index != 0;
        if is_not_cursor_leftmost {
            let current_index = self.character_index;
            let from_left_to_current_index = current_index - 1;

            let before_char_to_delete = self.input.chars().take(from_left_to_current_index);
            let after_char_to_delete = self.input.chars().skip(current_index);

            self.input = before_char_to_delete.chain(after_char_to_delete).collect();
            self.search_move_cursor_left();
        }
    }

    fn search_move_cursor_left(&mut self) {
        let cursor_moved_left = self.character_index.saturating_sub(1);
        self.character_index = self.search_clamp_cursor(cursor_moved_left);
    }

    fn search_move_cursor_right(&mut self) {
        let cursor_moved_right = self.character_index.saturating_add(1);
        self.character_index = self.search_clamp_cursor(cursor_moved_right);
    }

    fn search_clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.input.chars().count())
    }
}

#[derive(Clone)]
pub struct TransactionsTab {
    blocks: Rc<RefCell<VecDeque<ChainBlock>>>,
}
impl From<&App> for TransactionsTab {
    fn from(value: &App) -> Self {
        Self {
            blocks: Rc::clone(&value.chain.blocks),
        }
    }
}

#[derive(Clone, Default, PartialEq)]
enum InputMode {
    #[default]
    Normal,
    Editing,
}

#[derive(Clone, Default)]
enum ViewMode {
    #[default]
    Normal,
    Detail,
}

impl StatefulWidget for TransactionsTab {
    type State = TransactionsTabState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State)
    where
        Self: Sized,
    {
        match state.view_mode {
            ViewMode::Normal => {
                let block = Block::bordered().title(" Transactions ");
                block.clone().render(area, buf);
                let area = block.inner(area);

                let [search_area, txs_area] =
                    Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).areas(area);

                let input = match state.input_mode {
                    InputMode::Normal => Paragraph::new(state.input.as_str())
                        .style(Style::default().fg(Color::DarkGray))
                        .block(
                            Block::bordered()
                                .title(" Search | press f to filter ")
                                .border_style(Style::new().dark_gray()),
                        ),
                    InputMode::Editing => Paragraph::new(state.input.as_str())
                        .style(Style::default().fg(Color::White))
                        .block(
                            Block::bordered()
                                .title(" Search | press ESC to leave ")
                                .border_style(Style::new().white()),
                        ),
                };
                input.render(search_area, buf);

                let header = ["Hash", "Slot", "Certs", "Assets", "Total coin", "Datum"]
                    .into_iter()
                    .map(Cell::from)
                    .collect::<Row>()
                    .style(Style::default().fg(Color::Green).bold())
                    .height(1);
                let mut txs: Vec<TxView> = self
                    .blocks
                    .borrow()
                    .iter()
                    .flat_map(|chain_block| {
                        if let Some(body) = &chain_block.body {
                            return TxView::new(chain_block.slot, body);
                        }
                        Default::default()
                    })
                    .collect();
                if !state.input.is_empty() {
                    let input_regex = Regex::new(&state.input).unwrap();

                    txs.retain(|tx| {
                        input_regex.is_match(&tx.hash) || input_regex.is_match(&tx.slot.to_string())
                    });
                }

                let rows = txs.iter().enumerate().map(|(i, tx)| {
                    let color = match i % 2 {
                        0 => Color::Black,
                        _ => Color::Reset,
                    };
                    Row::new(vec![
                        format!("\n{}\n", tx.hash),
                        format!("\n{}\n", tx.slot),
                        format!("\n{}\n", tx.certs),
                        format!("\n{}\n", tx.assets),
                        format!("\n{}\n", tx.amount_ada),
                        format!("\n{}\n", if tx.datum { "yes" } else { "no" }),
                    ])
                    .style(Style::new().fg(Color::White).bg(color))
                    .height(3)
                });
                let bar = " █ ";
                let table = Table::new(
                    rows,
                    [
                        Constraint::Fill(1),
                        Constraint::Length(12),
                        Constraint::Length(12),
                        Constraint::Length(12),
                        Constraint::Length(12),
                        Constraint::Length(12),
                    ],
                )
                .header(header)
                .row_highlight_style(Modifier::BOLD)
                .highlight_symbol(Text::from(vec!["".into(), bar.into(), "".into()]))
                .highlight_spacing(HighlightSpacing::Always);

                StatefulWidget::render(table, txs_area, buf, &mut state.table_state);
                StatefulWidget::render(
                    Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight),
                    txs_area.inner(Margin {
                        vertical: 1,
                        horizontal: 0,
                    }),
                    buf,
                    &mut state.scroll_state,
                );
            }
            ViewMode::Detail => {
                let index = state.table_state.selected().unwrap();

                let txs: Vec<Tx> = self
                    .blocks
                    .borrow()
                    .iter()
                    .flat_map(|chain_block| {
                        if let Some(body) = &chain_block.body {
                            return body.tx.clone();
                        }
                        Default::default()
                    })
                    .collect();

                TransactionsDetail::new(txs[index].clone()).render(area, buf)
            }
        }
    }
}

#[derive(Clone)]
pub struct TransactionsDetail {
    tx: Tx,
}
impl TransactionsDetail {
    pub fn new(tx: Tx) -> Self {
        Self { tx }
    }
}
impl Widget for TransactionsDetail {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let block = Block::bordered().title(" Transaction Detail | press ESC to go back ");
        block.clone().render(area, buf);

        let inner = block.inner(area);

        Paragraph::new(hex::encode(self.tx.hash))
            .centered()
            .render(inner, buf);
    }
}

struct TxView {
    hash: String,
    slot: u64,
    certs: usize,
    assets: usize,
    amount_ada: u64,
    datum: bool,
}
impl TxView {
    pub fn new(slot: u64, body: &cardano::BlockBody) -> Vec<Self> {
        body.tx
            .iter()
            .map(|tx| Self {
                hash: hex::encode(&tx.hash),
                slot,
                certs: tx.certificates.len(),
                assets: tx.outputs.iter().map(|o| o.assets.len()).sum(),
                amount_ada: tx.outputs.iter().map(|o| o.coin).sum(),
                datum: tx.outputs.iter().any(|o| match &o.datum {
                    Some(datum) => !datum.hash.is_empty(),
                    None => false,
                }),
            })
            .collect()
    }
}
