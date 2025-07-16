use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pallas::ledger::addresses::Address;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{
        Block, Cell, HighlightSpacing, Padding, Paragraph, Row, Scrollbar, ScrollbarState,
        StatefulWidget, Table, TableState, Widget, Wrap,
    },
};
use regex::Regex;
use utxorpc::spec::cardano::{self, certificate::Certificate, stake_credential, Tx};

use crate::explorer::{App, ChainBlock};

#[derive(Default)]
pub struct TransactionsTabState {
    scroll_state: ScrollbarState,
    table_state: TableState,
    input: String,
    input_mode: InputMode,
    character_index: usize,
    view_mode: ViewMode,
    tx_selected: Option<TxView>,
    detail_state: TransactionsDetailState,
}
impl TransactionsTabState {
    pub fn handle_key(&mut self, key: &KeyEvent) {
        match self.view_mode {
            ViewMode::Normal => match self.input_mode {
                InputMode::Normal => match (key.code, key.modifiers) {
                    (KeyCode::Char('J') | KeyCode::Down, KeyModifiers::SHIFT) => self.last_row(),
                    (KeyCode::Char('j') | KeyCode::Down, _) => self.next_row(),
                    (KeyCode::Char('K') | KeyCode::Up, KeyModifiers::SHIFT) => self.first_row(),
                    (KeyCode::Char('k') | KeyCode::Up, _) => self.previous_row(),
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
                            self.view_mode = ViewMode::Detail;
                            self.tx_selected = None;
                            self.detail_state.vertical_offset = 0;
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
                _ => self.detail_state.handle_key(key),
            },
        }
    }

    pub fn update_scroll_state(&mut self, len: usize) {
        self.scroll_state = self.scroll_state.content_length(
            len.checked_mul(3)
                .and_then(|v| v.checked_sub(2))
                .unwrap_or(0),
        )
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
                let mut txs: Vec<TxView> =
                    self.blocks.borrow().iter().flat_map(TxView::new).collect();
                if !state.input.is_empty() {
                    let input_regex = Regex::new(&state.input).unwrap();

                    txs.retain(|tx| {
                        input_regex.is_match(&tx.hash)
                            || input_regex.is_match(&tx.block_slot.to_string())
                    });
                }

                let rows = txs.iter().enumerate().map(|(i, tx)| {
                    let color = match i % 2 {
                        0 => Color::Black,
                        _ => Color::Reset,
                    };
                    Row::new(vec![
                        format!("\n{}\n", tx.hash),
                        format!("\n{}\n", tx.block_slot),
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
                if state.tx_selected.is_none() {
                    let index = state.table_state.selected().unwrap();

                    let txs: Vec<TxView> = self
                        .blocks
                        .borrow()
                        .iter()
                        .flat_map(TxView::new_with_tx)
                        .collect();

                    state.tx_selected = Some(txs[index].clone());
                }

                TransactionsDetail::new(state.tx_selected.clone().unwrap()).render(
                    area,
                    buf,
                    &mut state.detail_state,
                )
            }
        }
    }
}

#[derive(Default)]
pub struct TransactionsDetailState {
    vertical_offset: u16,
}
impl TransactionsDetailState {
    pub fn handle_key(&mut self, key: &KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.up(),
            KeyCode::Char('k') | KeyCode::Up => self.down(),
            _ => {}
        };
    }

    fn down(&mut self) {
        self.vertical_offset = self.vertical_offset.saturating_sub(1);
    }

    fn up(&mut self) {
        self.vertical_offset += 1;
    }
}

#[derive(Clone)]
pub struct TransactionsDetail {
    tx_view: TxView,
}
impl TransactionsDetail {
    pub fn new(tx_view: TxView) -> Self {
        Self { tx_view }
    }
}
impl StatefulWidget for TransactionsDetail {
    type State = TransactionsDetailState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let block = Block::bordered()
            .title(" Transaction Detail | press ESC to go back ")
            .padding(Padding::symmetric(2, 1));
        block.clone().render(area, buf);

        let area = block.inner(area);

        let tx = self.tx_view.tx.unwrap();

        let mut tx_text = vec![
            Line::from(vec![
                Span::styled("tx: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(hex::encode(tx.hash)),
            ]),
            Line::default(),
        ];

        let inputs = tx
            .inputs
            .iter()
            .map(|i| Line::raw(format!("  {}#{}", hex::encode(&i.tx_hash), i.output_index)));
        tx_text.push(Line::styled(
            "inputs:",
            Style::default().add_modifier(Modifier::BOLD),
        ));
        tx_text.extend(inputs);
        tx_text.push(Line::default());

        let outputs = tx.outputs.iter().flat_map(|o| {
            let address = Address::from_bytes(&o.address)
                .map_or("decoded fail".to_string(), |addr| addr.to_string());

            let mut lines = vec![Line::raw(format!(
                "  {} - {} lovelace - {} assets",
                address,
                o.coin,
                o.assets.len()
            ))];

            if let Some(datum) = &o.datum {
                if !datum.hash.is_empty() {
                    lines.extend(vec![
                        Line::raw("    datum hash:"),
                        Line::raw(format!("      {}", hex::encode(&datum.hash))),
                    ]);
                }
                if let Some(payload) = &datum.payload {
                    let value = serde_json::to_value(payload).unwrap();
                    lines.extend(vec![
                        Line::raw("    datum payload:"),
                        Line::raw(format!("      {value}")),
                        Line::default(),
                    ]);
                }
            }
            lines
        });

        tx_text.push(Line::styled(
            "outputs:",
            Style::default().add_modifier(Modifier::BOLD),
        ));
        tx_text.extend(outputs);
        tx_text.push(Line::default());

        let reference_inputs = tx
            .reference_inputs
            .iter()
            .map(|i| Line::raw(format!("  {}#{}", hex::encode(&i.tx_hash), i.output_index)))
            .collect::<Vec<Line>>();
        if !reference_inputs.is_empty() {
            tx_text.push(Line::styled(
                "reference inputs:",
                Style::default().add_modifier(Modifier::BOLD),
            ));
            tx_text.extend(reference_inputs);
            tx_text.push(Line::default());
        }

        let mints = tx
            .mint
            .iter()
            .flat_map(|i| {
                let mut lines = vec![Line::raw(format!(
                    "  Policy Id: {}",
                    hex::encode(&i.policy_id)
                ))];
                lines.extend(i.assets.iter().flat_map(|a| {
                    vec![
                        Line::raw(format!("    Asset Name: {}", hex::encode(&a.name))),
                        Line::raw(format!("    Mint Coin: {}", a.mint_coin)),
                        Line::raw(format!("    Output Coin: {}", a.output_coin)),
                    ]
                }));
                lines
            })
            .collect::<Vec<Line>>();
        if !mints.is_empty() {
            tx_text.push(Line::styled(
                "mints:",
                Style::default().add_modifier(Modifier::BOLD),
            ));
            tx_text.extend(mints);
            tx_text.extend(vec![Line::default()]);
        }

        if let Some(collateral) = tx.collateral {
            if collateral.total_collateral > 0 {
                tx_text.extend(vec![
                    Line::styled("collateral:", Style::default().add_modifier(Modifier::BOLD)),
                    Line::raw(format!(
                        "  amount: {} lovelace",
                        collateral.total_collateral
                    )),
                    Line::default(),
                ]);
            }
        }

        let certs = tx
            .certificates
            .iter()
            .flat_map(|c| {
                let value = serde_json::to_value(c).unwrap();

                if let Some(certificate) = &c.certificate {
                    return match certificate {
                        Certificate::StakeRegistration(v) => {
                            let mut lines = vec![Line::raw("  Stake Registration")];
                            lines.extend(map_cert_stake_credential(v));
                            lines
                        }
                        Certificate::StakeDeregistration(v) => {
                            let mut lines = vec![Line::raw("  Vote Deregistration")];
                            lines.extend(map_cert_stake_credential(v));
                            lines
                        }
                        Certificate::StakeDelegation(v) => {
                            let mut lines = vec![
                                Line::raw("  Stake Delegation"),
                                Line::raw(format!(
                                    "    Pool Key Hash: {}",
                                    hex::encode(&v.pool_keyhash)
                                )),
                            ];
                            if let Some(v) = &v.stake_credential {
                                lines.extend(map_cert_stake_credential(v));
                            }
                            lines
                        }
                        Certificate::VoteDelegCert(v) => {
                            let mut lines = vec![Line::raw("  Vote Delegation")];
                            if let Some(v) = &v.stake_credential {
                                lines.extend(map_cert_stake_credential(v));
                            }
                            lines
                        }
                        _ => vec![Line::raw(serde_json::to_string(&value).unwrap())],
                    };
                }

                if let Some(redeemer) = &c.redeemer {
                    let value = serde_json::to_value(redeemer).unwrap();
                    return vec![
                        Line::raw("  Redeemer"),
                        Line::raw(serde_json::to_string(&value).unwrap()),
                    ];
                }

                Vec::default()
            })
            .collect::<Vec<Line>>();
        if !certs.is_empty() {
            tx_text.push(Line::styled(
                "certs:",
                Style::default().add_modifier(Modifier::BOLD),
            ));
            tx_text.extend(certs);
            tx_text.extend(vec![Line::default()]);
        }

        tx_text.extend(vec![
            Line::styled("block:", Style::default().add_modifier(Modifier::BOLD)),
            Line::raw(format!("  slot: {}", self.tx_view.block_slot)),
            Line::raw(format!("  hash: {}", self.tx_view.block_hash)),
        ]);

        let paragraph = Paragraph::new(Text::from(tx_text))
            .wrap(Wrap { trim: false })
            .scroll((state.vertical_offset, 0));

        paragraph.render(area, buf);
    }
}

#[derive(Clone)]
pub struct TxView {
    hash: String,
    certs: usize,
    assets: usize,
    amount_ada: u64,
    datum: bool,
    tx: Option<Tx>,
    block_slot: u64,
    block_hash: String,
}
impl TxView {
    pub fn new(chain_block: &ChainBlock) -> Vec<Self> {
        match &chain_block.body {
            Some(body) => body
                .tx
                .iter()
                .map(|tx| Self {
                    hash: hex::encode(&tx.hash),
                    certs: tx.certificates.len(),
                    assets: tx.outputs.iter().map(|o| o.assets.len()).sum(),
                    amount_ada: tx.outputs.iter().map(|o| o.coin).sum(),
                    datum: tx.outputs.iter().any(|o| match &o.datum {
                        Some(datum) => !datum.hash.is_empty(),
                        None => false,
                    }),
                    tx: None,
                    block_slot: chain_block.slot,
                    block_hash: hex::encode(&chain_block.hash),
                })
                .collect(),
            None => Default::default(),
        }
    }

    pub fn new_with_tx(chain_block: &ChainBlock) -> Vec<Self> {
        match &chain_block.body {
            Some(body) => body
                .tx
                .iter()
                .map(|tx| Self {
                    hash: hex::encode(&tx.hash),
                    certs: tx.certificates.len(),
                    assets: tx.outputs.iter().map(|o| o.assets.len()).sum(),
                    amount_ada: tx.outputs.iter().map(|o| o.coin).sum(),
                    datum: tx.outputs.iter().any(|o| match &o.datum {
                        Some(datum) => !datum.hash.is_empty(),
                        None => false,
                    }),
                    tx: Some(tx.clone()),
                    block_slot: chain_block.slot,
                    block_hash: hex::encode(&chain_block.hash),
                })
                .collect(),
            None => Default::default(),
        }
    }
}

fn map_cert_stake_credential(v: &cardano::StakeCredential) -> Vec<Line<'_>> {
    if let Some(stake_credential) = &v.stake_credential {
        vec![
            match stake_credential {
                stake_credential::StakeCredential::AddrKeyHash(addr_key_hash) => {
                    Line::raw(format!("    Key Hash: {}", hex::encode(addr_key_hash)))
                }
                stake_credential::StakeCredential::ScriptHash(script_hash) => {
                    Line::raw(format!("    Script Hash: {}", hex::encode(script_hash)))
                }
            },
            Line::default(),
        ]
    } else {
        Vec::default()
    }
}
