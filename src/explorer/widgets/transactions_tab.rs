use crossterm::event::{KeyCode, KeyEvent};
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
use utxorpc::spec::cardano;

use crate::explorer::{App, ChainBlock};

#[derive(Default)]
pub struct TransactionsTabState {
    scroll_state: ScrollbarState,
    table_state: TableState,
    input: String,
    input_mode: InputMode,
    character_index: usize,
}
impl TransactionsTabState {
    fn byte_index(&self) -> usize {
        self.input
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.character_index)
            .unwrap_or(self.input.len())
    }

    pub fn enter_char(&mut self, new_char: char) {
        let index = self.byte_index();
        self.input.insert(index, new_char);
        self.move_cursor_right();
    }

    pub fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.character_index.saturating_sub(1);
        self.character_index = self.clamp_cursor(cursor_moved_left);
    }

    pub fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.character_index.saturating_add(1);
        self.character_index = self.clamp_cursor(cursor_moved_right);
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.input.chars().count())
    }

    pub fn delete_char(&mut self) {
        let is_not_cursor_leftmost = self.character_index != 0;
        if is_not_cursor_leftmost {
            let current_index = self.character_index;
            let from_left_to_current_index = current_index - 1;

            let before_char_to_delete = self.input.chars().take(from_left_to_current_index);
            let after_char_to_delete = self.input.chars().skip(current_index);

            self.input = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    pub fn next_row(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => i + 1,
            None => 0,
        };

        self.table_state.select(Some(i));
        self.scroll_state = self.scroll_state.position(i * 3);
    }

    pub fn previous_row(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    0
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
        self.scroll_state = self.scroll_state.position(i * 3);
    }

    pub fn handle_key(&mut self, key: &KeyEvent) {
        match self.input_mode {
            InputMode::Normal => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.next_row();
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.previous_row();
                }
                KeyCode::Char('e') => self.input_mode = InputMode::Editing,
                _ => {}
            },
            InputMode::Editing => match key.code {
                KeyCode::Char(to_insert) => self.enter_char(to_insert),
                KeyCode::Left => self.move_cursor_left(),
                KeyCode::Right => self.move_cursor_right(),
                KeyCode::Esc => self.input_mode = InputMode::Normal,
                KeyCode::Backspace => self.delete_char(),
                _ => {}
            },
        }
    }
}

#[derive(Clone)]
pub struct TransactionsTab {
    pub blocks: Vec<ChainBlock>,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub enum InputMode {
    #[default]
    Normal,
    Editing,
}

impl From<&App> for TransactionsTab {
    fn from(value: &App) -> Self {
        Self {
            blocks: value.chain.blocks.clone(),
        }
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

impl StatefulWidget for TransactionsTab {
    type State = TransactionsTabState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State)
    where
        Self: Sized,
    {
        let [search_area, txs_area] =
            Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).areas(area);

        let input = Paragraph::new(state.input.as_str())
            .style(match state.input_mode {
                InputMode::Normal => Style::default(),
                InputMode::Editing => Style::default().fg(Color::White),
            })
            .block(
                Block::bordered()
                    .title(" Search ")
                    .border_style(match state.input_mode {
                        InputMode::Normal => Style::new().dark_gray(),
                        InputMode::Editing => Style::new().white(),
                    }),
            );

        input.render(search_area, buf);

        let header = ["Hash", "Slot", "Certs", "Assets", "Total Coin", "Datum"]
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(Style::default().fg(Color::Green).bold())
            .height(1);
        let mut txs: Vec<TxView> = self
            .blocks
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
            txs = txs
                .into_iter()
                .filter(|tx| {
                    input_regex.is_match(&tx.hash) || input_regex.is_match(&tx.slot.to_string())
                })
                .collect();
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
        .highlight_spacing(HighlightSpacing::Always)
        .block(Block::bordered());

        StatefulWidget::render(table, txs_area, buf, &mut state.table_state);
        StatefulWidget::render(
            Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight),
            txs_area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
            buf,
            &mut state.scroll_state,
        );
    }
}
