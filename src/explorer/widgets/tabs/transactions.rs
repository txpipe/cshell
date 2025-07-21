use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use crossterm::event::{KeyCode, KeyEvent};
use pallas::ledger::addresses::Address;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, StatefulWidget, Widget},
};
use regex::Regex;
use tui_tree_widget::{Tree, TreeItem, TreeState};
use utxorpc::spec::cardano::{self, certificate::Certificate, stake_credential, Tx};

use crate::explorer::ChainBlock;

#[derive(Clone, Default, PartialEq)]
enum InputMode {
    #[default]
    Normal,
    Editing,
}

#[derive(Default)]
pub struct TransactionsTabState {
    input: String,
    input_mode: InputMode,
    character_index: usize,
    tree_state: TreeState<String>,

    items: Vec<TreeItem<'static, String>>,
    blocks: Rc<RefCell<VecDeque<ChainBlock>>>,
}
impl TransactionsTabState {
    pub fn handle_key(&mut self, key: &KeyEvent) {
        match self.input_mode {
            InputMode::Normal => match key.code {
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

                KeyCode::Char('f') | KeyCode::Char('/') => self.input_mode = InputMode::Editing,
                KeyCode::Esc => {
                    if !self.input.is_empty() {
                        self.input.clear()
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
                        self.update_items();
                        self.tree_state.select_first();
                    }
                    self.input_mode = InputMode::Normal
                }

                _ => {}
            },
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

    pub fn update_blocks(&mut self, blocks: Rc<RefCell<VecDeque<ChainBlock>>>) {
        self.blocks = blocks;
        self.update_items();
    }

    fn update_items(&mut self) {
        let mut items = vec![];
        let input_regex = Regex::new(&self.input).unwrap();
        let blocks = self.blocks.borrow();

        for block in blocks.iter() {
            if let Some(body) = &block.body {
                let filtered_tx: Vec<&Tx> = body
                    .tx
                    .iter()
                    .filter(|tx| input_regex.is_match(&hex::encode(&tx.hash)))
                    .collect();

                for tx in filtered_tx {
                    let mut root = vec![];

                    let tx_hash = hex::encode(&tx.hash);

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
                                format!("Hash: {}", hex::encode(&block.hash)),
                                vec![],
                            )
                            .expect("hash"),
                            TreeItem::new(
                                "slot".to_string(),
                                format!("Slot: {}", block.slot),
                                vec![],
                            )
                            .expect("slot"),
                            TreeItem::new(
                                "height".to_string(),
                                format!("Height: {}", block.number),
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

                        let certs_node = TreeItem::new(
                            "certificates".to_string(),
                            "Certificates",
                            certs_children,
                        )
                        .expect("certificates");
                        root.push(certs_node);
                    }

                    items.push(
                        TreeItem::new(
                            format!("tx_hash_{tx_hash}"),
                            format!("Transaction: {tx_hash}"),
                            root,
                        )
                        .expect("unique root")
                        // .style(Style::default().fg(Color::Green).bold())
                        // .height(1),
                    );
                }
            }
        }
        self.items = items;
    }
}

#[derive(Clone)]
pub struct TransactionsTab;
impl StatefulWidget for TransactionsTab {
    type State = TransactionsTabState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State)
    where
        Self: Sized,
    {
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

        let tree = Tree::new(&state.items)
            .expect("all item identifiers are unique")
            .experimental_scrollbar(Some(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(None)
                    .track_symbol(None)
                    .end_symbol(None),
            ))
            .highlight_style(
                Style::new()
                    .fg(Color::Black)
                    .bg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            );

        StatefulWidget::render(tree, txs_area, buf, &mut state.tree_state);
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
