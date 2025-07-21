use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pallas::ledger::addresses::Address;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::Text,
    widgets::{
        Block, Cell, HighlightSpacing, Padding, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, StatefulWidget, Table, TableState, Widget,
    },
};
use regex::Regex;
use tui_tree_widget::{Tree, TreeItem, TreeState};
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
                            // self.detail_state.vertical_offset = 0;
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
    tree_state: TreeState<String>,
}
impl TransactionsDetailState {
    pub fn handle_key(&mut self, key: &KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                self.tree_state.toggle_selected();
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.tree_state.key_left();
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.tree_state.key_right();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.tree_state.key_down();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.tree_state.key_up();
            }
            _ => {}
        };
    }
}

#[derive(Clone)]
pub struct TransactionsDetail {
    items: Vec<TreeItem<'static, String>>,
}
impl TransactionsDetail {
    pub fn new(tx_view: TxView) -> Self {
        let items = Self::build_tree_items(tx_view);
        Self { items }
    }

    fn build_tree_items(tx_view: TxView) -> Vec<TreeItem<'static, String>> {
        let mut root = vec![];

        let tx_hash = hex::encode(&tx_view.hash);
        let tx = tx_view.tx.as_ref().unwrap();

        root.push(
            TreeItem::new(
                "tx_hash_info".to_string(),
                format!("Tx Hash: {tx_hash}"),
                vec![],
            )
            .expect("block info"),
        );

        let block_node = TreeItem::new(
            "block_info".to_string(),
            "Block Info",
            vec![
                TreeItem::new(
                    "hash".to_string(),
                    format!("Hash: {}", hex::encode(&tx_view.block_hash)),
                    vec![],
                )
                .expect("hash"),
                TreeItem::new(
                    "slot".to_string(),
                    format!("Slot: {}", tx_view.block_slot),
                    vec![],
                )
                .expect("slot"),
                TreeItem::new(
                    "height".to_string(),
                    format!("Height: {}", tx_view.block_height),
                    vec![],
                )
                .expect("height"),
            ],
        )
        .expect("block info");
        root.push(block_node);

        let inputs_node = TreeItem::new(
            "inputs".to_string(),
            "Inputs",
            tx.inputs
                .iter()
                .map(|i| {
                    let tx_hash = hex::encode(&i.tx_hash);
                    let output_index = i.output_index;

                    TreeItem::new(
                        format!("input_{tx_hash}_{output_index}"),
                        format!("{tx_hash}#{output_index}"),
                        vec![],
                    )
                    .expect("unique inputs")
                })
                .collect(),
        )
        .expect("unique inputs group");
        root.push(inputs_node);

        let outputs_node = TreeItem::new(
            "outputs".to_string(),
            "Outputs",
            tx.outputs
                .iter()
                .enumerate()
                .map(|(i, o)| {
                    let address = Address::from_bytes(&o.address)
                        .map_or("decoded fail".to_string(), |addr| addr.to_string());

                    TreeItem::new(
                        format!("output_{tx_hash}_{i}"),
                        format!(
                            "{address} - {} lovelace - {} assets",
                            o.coin,
                            o.assets.len()
                        ),
                        vec![],
                    )
                    .expect("unique outputs")
                })
                .collect(),
        )
        .expect("unique outputs group");
        root.push(outputs_node);

        // Reference Inputs
        if !tx.reference_inputs.is_empty() {
            let ref_node = TreeItem::new(
                "reference_inputs".to_string(),
                "Reference Inputs",
                tx.reference_inputs
                    .iter()
                    .map(|i| {
                        let tx_hash = hex::encode(&i.tx_hash);
                        let output_index = i.output_index;
                        TreeItem::new(
                            format!("refinput_{tx_hash}_{output_index}"),
                            format!("{tx_hash}#{output_index}"),
                            vec![],
                        )
                        .expect("unique ref inputs")
                    })
                    .collect(),
            )
            .expect("unique ref group");
            root.push(ref_node);
        }

        // Mint
        if !tx.mint.is_empty() {
            let mint_node = TreeItem::new(
                "mints".to_string(),
                "Mints",
                tx.mint
                    .iter()
                    .map(|mint| {
                        let policy_id = hex::encode(&mint.policy_id);

                        TreeItem::new(
                            format!("policy_{policy_id}"),
                            format!("Policy Id: {policy_id}"),
                            mint.assets
                                .iter()
                                .map(|a| {
                                    let name_hex = hex::encode(&a.name);
                                    let name_utf8 = String::from_utf8(a.name.to_vec())
                                        .unwrap_or(name_hex.clone());

                                    TreeItem::new(
                                        format!("asset_{policy_id}{name_hex}"),
                                        format!("Asset: {name_utf8}"),
                                        vec![
                                            TreeItem::new(
                                                format!("asset_mintcoin_{policy_id}{name_hex}"),
                                                format!("Mint Coin: {}", a.mint_coin),
                                                vec![],
                                            )
                                            .expect("mint coin"),
                                            TreeItem::new(
                                                format!("asset_outputcoin_{policy_id}{name_hex}"),
                                                format!("Output Coin: {}", a.output_coin),
                                                vec![],
                                            )
                                            .expect("output coin"),
                                        ],
                                    )
                                    .expect("unique asset")
                                })
                                .collect(),
                        )
                        .expect("unique policy")
                    })
                    .collect(),
            )
            .expect("unique mint group");
            root.push(mint_node);
        }

        // Collateral
        if let Some(collateral) = &tx.collateral {
            if collateral.total_collateral > 0 {
                let collateral_node = TreeItem::new(
                    "collateral".to_string(),
                    "Collateral",
                    vec![TreeItem::new(
                        "collateral_amount".to_string(),
                        format!("Amount: {} lovelace", collateral.total_collateral),
                        vec![],
                    )
                    .expect("collateral amount")],
                )
                .expect("collateral");
                root.push(collateral_node);
            }
        }

        // Certificates
        if !tx.certificates.is_empty() {
            let certs_children = tx
                .certificates
                .iter()
                .map(|cert| match &cert.certificate {
                    Some(Certificate::StakeRegistration(v)) => {
                        let stake_children = map_cert_stake_credential(v);
                        TreeItem::new(
                            "stake_registration".to_string(),
                            "Stake Registration",
                            stake_children,
                        )
                        .expect("stake registration")
                    }
                    Some(Certificate::StakeDeregistration(v)) => {
                        let dereg_children = map_cert_stake_credential(v);
                        TreeItem::new(
                            "stake_deregistration".to_string(),
                            "Stake Deregistration",
                            dereg_children,
                        )
                        .expect("stake deregistration")
                    }
                    Some(Certificate::StakeDelegation(v)) => {
                        let mut deleg_children = vec![TreeItem::new(
                            "pool_keyhash".to_string(),
                            format!("Pool Key Hash: {}", hex::encode(&v.pool_keyhash)),
                            vec![],
                        )
                        .expect("pool keyhash")];
                        if let Some(c) = &v.stake_credential {
                            deleg_children.extend(map_cert_stake_credential(c));
                        }
                        TreeItem::new(
                            "stake_delegation".to_string(),
                            "Stake Delegation",
                            deleg_children,
                        )
                        .expect("stake delegation")
                    }
                    Some(Certificate::VoteDelegCert(v)) => {
                        let vote_children = v
                            .stake_credential
                            .as_ref()
                            .map(map_cert_stake_credential)
                            .unwrap_or_default();

                        TreeItem::new(
                            "vote_delegation".to_string(),
                            "Vote Delegation",
                            vote_children,
                        )
                        .expect("vote deleg")
                    }
                    _ => TreeItem::new(
                        "unknown_cert".to_string(),
                        serde_json::to_string(cert).unwrap(),
                        vec![],
                    )
                    .expect("unknown cert"),
                })
                .collect::<Vec<_>>();

            let certs_node =
                TreeItem::new("certificates".to_string(), "Certificates", certs_children)
                    .expect("certificates");
            root.push(certs_node);
        }

        root
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

        let tree = Tree::new(&self.items)
            .expect("all item identifiers are unique")
            .experimental_scrollbar(Some(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(None)
                    .track_symbol(None)
                    .end_symbol(None),
            ))
            .highlight_style(Style::new().add_modifier(Modifier::BOLD));

        StatefulWidget::render(tree, area, buf, &mut state.tree_state);
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
    block_height: u64,
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
                    block_height: chain_block.number,
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
                    block_height: chain_block.number,
                    block_hash: hex::encode(&chain_block.hash),
                })
                .collect(),
            None => Default::default(),
        }
    }
}

fn map_cert_stake_credential<'a>(v: &cardano::StakeCredential) -> Vec<TreeItem<'a, String>> {
    if let Some(stake_credential) = &v.stake_credential {
        let content = match stake_credential {
            stake_credential::StakeCredential::AddrKeyHash(addr_key_hash) => {
                format!("Key Hash: {}", hex::encode(addr_key_hash))
            }
            stake_credential::StakeCredential::ScriptHash(script_hash) => {
                format!("Script Hash: {}", hex::encode(script_hash))
            }
        };

        return vec![
            TreeItem::new(content.clone(), content, vec![]).expect("unique stake credential")
        ];
    }

    vec![]
}
