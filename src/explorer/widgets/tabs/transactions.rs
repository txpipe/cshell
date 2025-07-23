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
use utxorpc::spec::cardano::{
    self, big_int,
    certificate::Certificate,
    d_rep, metadatum, native_script, plutus_data,
    script::{self},
    stake_credential, AuxData, Datum, Metadatum, NativeScript, PlutusData, Redeemer,
    RedeemerPurpose, Script, Tx, TxInput, TxOutput, TxValidity, VKeyWitness, Withdrawal,
    WitnessSet,
};

use crate::explorer::{App, ChainBlock};

#[derive(Default)]
pub struct TransactionsTabState {
    scroll_state: ScrollbarState,
    table_state: TableState,
    search_input: String,
    input_mode: InputMode,
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
                        if !self.search_input.is_empty() {
                            self.search_input.clear()
                        }
                    }
                    (KeyCode::Enter, _) => {
                        if self.table_state.selected().is_some() {
                            self.detail_state.tree_state.close_all();
                            self.view_mode = ViewMode::Detail;
                            self.tx_selected = None;
                        }
                    }
                    _ => {}
                },
                InputMode::Editing => match key.code {
                    KeyCode::Char(c) => self.search_input.push(c),
                    KeyCode::Backspace => {
                        self.search_input.pop();
                    }
                    KeyCode::Esc => self.input_mode = InputMode::Normal,
                    KeyCode::Enter => {
                        self.table_state.select_first();
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
                    InputMode::Normal => Paragraph::new(state.search_input.as_str())
                        .style(Style::default().fg(Color::DarkGray))
                        .block(
                            Block::bordered()
                                .title(" Search | press f to filter ")
                                .border_style(Style::new().dark_gray()),
                        ),
                    InputMode::Editing => Paragraph::new(format!("{}│", state.search_input))
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
                if !state.search_input.is_empty() {
                    let input_regex = Regex::new(&state.search_input).unwrap();

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
        let tx = tx_view.tx.as_ref().unwrap();
        let tx_hash = hex::encode(&tx.hash);

        let mut root = vec![
            TreeItem::new_leaf("tx_hash_info".to_string(), format!("Hash: {tx_hash}")),
            TreeItem::new_leaf("tx_fee_info".to_string(), format!("Fee: {}", tx.fee)),
        ];

        // Block Info
        let block_node = TreeItem::new(
            "block_info".to_string(),
            "Block Info",
            vec![
                TreeItem::new_leaf(
                    "hash".to_string(),
                    format!("Hash: {}", hex::encode(&tx_view.block_hash)),
                ),
                TreeItem::new_leaf("slot".to_string(), format!("Slot: {}", tx_view.block_slot)),
                TreeItem::new_leaf(
                    "height".to_string(),
                    format!("Height: {}", tx_view.block_height),
                ),
            ],
        )
        .expect("Failed to create block info node");
        root.push(block_node);

        // Inputs
        let inputs_node = TreeItem::new(
            "inputs".to_string(),
            "Inputs",
            tx.inputs
                .iter()
                .enumerate()
                .flat_map(|(i, input)| map_tx_input(input, &i.to_string(), &tx_hash))
                .collect(),
        )
        .expect("Failed to create inputs node");
        root.push(inputs_node);

        // Outputs
        let outputs_node = TreeItem::new(
            "outputs".to_string(),
            "Outputs",
            tx.outputs
                .iter()
                .enumerate()
                .map(|(i, output)| map_tx_output(output, i, &tx_hash))
                .collect(),
        )
        .expect("Failed to create outputs node");
        root.push(outputs_node);

        // Reference Inputs
        if !tx.reference_inputs.is_empty() {
            let ref_node = TreeItem::new(
                "reference_inputs".to_string(),
                "Reference Inputs",
                tx.reference_inputs
                    .iter()
                    .enumerate()
                    .flat_map(|(i, input)| map_tx_input(input, &format!("reference_{i}"), &tx_hash))
                    .collect(),
            )
            .expect("Failed to create reference inputs node");
            root.push(ref_node);
        }

        // Mint
        if !tx.mint.is_empty() {
            let mint_node = TreeItem::new(
                "mints".to_string(),
                "Mints",
                tx.mint
                    .iter()
                    .enumerate()
                    .map(|(i, mint)| {
                        let policy_id = hex::encode(&mint.policy_id);
                        let mut children = mint
                            .assets
                            .iter()
                            .enumerate()
                            .map(|(j, asset)| {
                                let name = String::try_from(asset.name.to_vec())
                                    .unwrap_or(" - ".to_string());
                                TreeItem::new(
                                    format!("mint_asset_{policy_id}_{i}_{j}"),
                                    format!("Asset: {name}"),
                                    vec![
                                        TreeItem::new_leaf(
                                            format!("mint_asset_mintcoin_{policy_id}_{i}_{j}"),
                                            format!("Mint Coin: {}", asset.mint_coin),
                                        ),
                                        TreeItem::new_leaf(
                                            format!("mint_asset_outputcoin_{policy_id}_{i}_{j}"),
                                            format!("Output Coin: {}", asset.output_coin),
                                        ),
                                    ],
                                )
                                .expect("Failed to create mint asset node")
                            })
                            .collect::<Vec<_>>();
                        children.extend(map_redeemer(&mint.redeemer, &format!("mint_{i}")));
                        TreeItem::new(
                            format!("mint_policy_{policy_id}_{i}"),
                            format!("Policy: {policy_id}"),
                            children,
                        )
                        .expect("Failed to create mint policy node")
                    })
                    .collect(),
            )
            .expect("Failed to create mints node");
            root.push(mint_node);
        }

        // Collateral
        if let Some(collateral) = &tx.collateral {
            let mut children = vec![];
            if !collateral.collateral.is_empty() {
                children.push(
                    TreeItem::new(
                        "collateral_inputs".to_string(),
                        "Collateral Inputs",
                        collateral
                            .collateral
                            .iter()
                            .enumerate()
                            .flat_map(|(i, input)| {
                                map_tx_input(input, &format!("collateral_{i}"), &tx_hash)
                            })
                            .collect(),
                    )
                    .expect("Failed to create collateral inputs node"),
                );
            }
            children.push(TreeItem::new_leaf(
                "total_collateral".to_string(),
                format!("Total Collateral: {}", collateral.total_collateral),
            ));
            let collateral_node =
                TreeItem::new("collateral".to_string(), "Collateral".to_string(), children)
                    .expect("Failed to create collateral node");
            root.push(collateral_node);
        }

        // Withdrawals
        if !tx.withdrawals.is_empty() {
            let withdrawals_node = TreeItem::new(
                "withdrawals".to_string(),
                "Withdrawals",
                tx.withdrawals
                    .iter()
                    .enumerate()
                    .flat_map(|(i, withdrawal)| {
                        map_withdrawal(withdrawal, &format!("withdrawal_{i}"))
                    })
                    .collect(),
            )
            .expect("Failed to create withdrawals node");
            root.push(withdrawals_node);
        }

        // Witness Set
        root.extend(map_witness_set(&tx.witnesses, 0));

        // Validity
        root.extend(map_tx_validity(&tx.validity, 0));

        // Auxiliary Data
        root.extend(map_aux_data(&tx.auxiliary, 0));

        // Certificates
        if !tx.certificates.is_empty() {
            let certs_node = map_cert(tx);
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

        match Tree::new(&self.items) {
            Ok(tree) => {
                let widget = tree
                    .experimental_scrollbar(Some(
                        Scrollbar::new(ScrollbarOrientation::VerticalRight)
                            .begin_symbol(None)
                            .track_symbol(None)
                            .end_symbol(None),
                    ))
                    .highlight_style(Style::new().add_modifier(Modifier::BOLD));

                StatefulWidget::render(widget, area, buf, &mut state.tree_state);
            }
            Err(err) => {
                panic!("error: {err:?}")
            }
        };
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

        return vec![TreeItem::new_leaf(content.clone(), content)];
    }

    vec![]
}

fn map_drep<'a>(drep: &Option<cardano::DRep>, index: usize) -> Vec<TreeItem<'a, String>> {
    let drep_content = match drep {
        Some(drep) => match &drep.drep {
            Some(d_rep::Drep::AddrKeyHash(hash)) => {
                format!("DRep Key Hash: {}", hex::encode(hash))
            }
            Some(d_rep::Drep::ScriptHash(hash)) => {
                format!("DRep Script Hash: {}", hex::encode(hash))
            }
            Some(d_rep::Drep::Abstain(_)) => "DRep: Abstain".to_string(),
            Some(d_rep::Drep::NoConfidence(_)) => "DRep: No Confidence".to_string(),
            None => "DRep: None".to_string(),
        },
        None => "DRep: None".to_string(),
    };
    vec![TreeItem::new_leaf(format!("drep_{index}"), drep_content)]
}

fn map_cert<'a>(tx: &Tx) -> TreeItem<'a, String> {
    let certs_children = tx
        .certificates
        .iter()
        .enumerate()
        .map(|(i, cert)| match &cert.certificate {
            Some(Certificate::StakeRegistration(v)) => {
                let stake_children = map_cert_stake_credential(v);
                TreeItem::new(
                    format!("stake_registration_{i}"),
                    "Stake Registration",
                    stake_children,
                )
                .expect("Failed to create stake registration node")
            }
            Some(Certificate::StakeDeregistration(v)) => {
                let dereg_children = map_cert_stake_credential(v);
                TreeItem::new(
                    format!("stake_deregistration_{i}"),
                    "Stake Deregistration",
                    dereg_children,
                )
                .expect("Failed to create stake deregistration node")
            }
            Some(Certificate::StakeDelegation(v)) => {
                let mut deleg_children = vec![TreeItem::new_leaf(
                    format!("pool_keyhash_{i}"),
                    format!("Pool Key Hash: {}", hex::encode(&v.pool_keyhash)),
                )];
                if let Some(c) = &v.stake_credential {
                    deleg_children.extend(map_cert_stake_credential(c));
                }
                TreeItem::new(
                    format!("stake_delegation_{i}"),
                    "Stake Delegation",
                    deleg_children,
                )
                .expect("Failed to create stake delegation node")
            }
            Some(Certificate::VoteDelegCert(v)) => {
                let mut vote_children = v
                    .stake_credential
                    .as_ref()
                    .map(map_cert_stake_credential)
                    .unwrap_or_default();
                vote_children.extend(map_drep(&v.drep, i));
                TreeItem::new(
                    format!("vote_delegation_{i}"),
                    "Vote Delegation",
                    vote_children,
                )
                .expect("Failed to create vote delegation node")
            }
            Some(Certificate::PoolRegistration(v)) => {
                let mut pool_children = vec![
                    TreeItem::new_leaf(
                        format!("pool_operator_{i}"),
                        format!("Operator Key Hash: {}", hex::encode(&v.operator)),
                    ),
                    TreeItem::new_leaf(
                        format!("vrf_keyhash_{i}"),
                        format!("VRF Key Hash: {}", hex::encode(&v.vrf_keyhash)),
                    ),
                    TreeItem::new_leaf(format!("pledge_{i}"), format!("Pledge: {}", v.pledge)),
                    TreeItem::new_leaf(format!("cost_{i}"), format!("Cost: {}", v.cost)),
                    TreeItem::new_leaf(
                        format!("reward_account_{i}"),
                        format!("Reward Account: {}", hex::encode(&v.reward_account)),
                    ),
                ];
                if let Some(margin) = &v.margin {
                    pool_children.push(TreeItem::new_leaf(
                        format!("margin_{i}"),
                        format!("Margin: {}/{}", margin.numerator, margin.denominator),
                    ));
                }
                if !v.pool_owners.is_empty() {
                    pool_children.push(
                        TreeItem::new(
                            format!("pool_owners_{i}"),
                            "Pool Owners",
                            v.pool_owners
                                .iter()
                                .enumerate()
                                .map(|(j, owner)| {
                                    TreeItem::new_leaf(
                                        format!("owner_{i}_{j}"),
                                        format!("Owner: {}", hex::encode(owner)),
                                    )
                                })
                                .collect(),
                        )
                        .expect("Failed to create pool owners node"),
                    );
                }
                if !v.relays.is_empty() {
                    pool_children.push(
                        TreeItem::new(
                            format!("relays_{i}"),
                            "Relays",
                            v.relays
                                .iter()
                                .enumerate()
                                .map(|(j, relay)| {
                                    TreeItem::new_leaf(
                                        format!("relay_{i}_{j}"),
                                        format!("Relay: {relay:?}"),
                                    )
                                })
                                .collect(),
                        )
                        .expect("Failed to create relays node"),
                    );
                }
                if let Some(metadata) = &v.pool_metadata {
                    pool_children.push(TreeItem::new_leaf(
                        format!("metadata_{i}"),
                        format!("Metadata: {metadata:?}"),
                    ));
                }
                TreeItem::new(
                    format!("pool_registration_{i}"),
                    "Pool Registration",
                    pool_children,
                )
                .expect("Failed to create pool registration node")
            }
            Some(Certificate::PoolRetirement(v)) => {
                let retirement_children = vec![
                    TreeItem::new_leaf(
                        format!("pool_keyhash_{i}"),
                        format!("Pool Key Hash: {}", hex::encode(&v.pool_keyhash)),
                    ),
                    TreeItem::new_leaf(
                        format!("retirement_epoch_{i}"),
                        format!("Retirement Epoch: {}", v.epoch),
                    ),
                ];
                TreeItem::new(
                    format!("pool_retirement_{i}"),
                    "Pool Retirement",
                    retirement_children,
                )
                .expect("Failed to create pool retirement node")
            }
            Some(Certificate::GenesisKeyDelegation(v)) => {
                let genesis_children = vec![
                    TreeItem::new_leaf(
                        format!("genesis_hash_{i}"),
                        format!("Genesis Hash: {}", hex::encode(&v.genesis_hash)),
                    ),
                    TreeItem::new_leaf(
                        format!("genesis_delegate_hash_{i}"),
                        format!("Delegate Hash: {}", hex::encode(&v.genesis_delegate_hash)),
                    ),
                    TreeItem::new_leaf(
                        format!("vrf_keyhash_{i}"),
                        format!("VRF Key Hash: {}", hex::encode(&v.vrf_keyhash)),
                    ),
                ];
                TreeItem::new(
                    format!("genesis_key_delegation_{i}"),
                    "Genesis Key Delegation",
                    genesis_children,
                )
                .expect("Failed to create genesis key delegation node")
            }
            Some(Certificate::MirCert(v)) => {
                let mut mir_children = vec![TreeItem::new_leaf(
                    format!("mir_source_{i}"),
                    format!("Source: {:?}", v.from),
                )];
                if !v.to.is_empty() {
                    mir_children.push(
                        TreeItem::new(
                            format!("mir_targets_{i}"),
                            "Targets",
                            v.to.iter()
                                .enumerate()
                                .map(|(j, target)| {
                                    let mut target_children = target
                                        .stake_credential
                                        .as_ref()
                                        .map(map_cert_stake_credential)
                                        .unwrap_or_default();
                                    target_children.push(TreeItem::new_leaf(
                                        format!("delta_coin_{i}_{j}"),
                                        format!("Delta Coin: {}", target.delta_coin),
                                    ));
                                    TreeItem::new(
                                        format!("mir_target_{i}_{j}"),
                                        format!("Target {j}"),
                                        target_children,
                                    )
                                    .expect("Failed to create MIR target node")
                                })
                                .collect(),
                        )
                        .expect("Failed to create MIR targets node"),
                    );
                }
                mir_children.push(TreeItem::new_leaf(
                    format!("other_pot_{i}"),
                    format!("Other Pot: {}", v.other_pot),
                ));
                TreeItem::new(
                    format!("mir_cert_{i}"),
                    "Move Instantaneous Reward",
                    mir_children,
                )
                .expect("Failed to create MIR certificate node")
            }
            Some(Certificate::RegCert(v)) => {
                let mut reg_children = v
                    .stake_credential
                    .as_ref()
                    .map(map_cert_stake_credential)
                    .unwrap_or_default();
                reg_children.push(TreeItem::new_leaf(
                    format!("coin_{i}"),
                    format!("Coin: {}", v.coin),
                ));
                TreeItem::new(format!("reg_cert_{i}"), "Registration", reg_children)
                    .expect("Failed to create registration certificate node")
            }
            Some(Certificate::UnregCert(v)) => {
                let mut unreg_children = v
                    .stake_credential
                    .as_ref()
                    .map(map_cert_stake_credential)
                    .unwrap_or_default();
                unreg_children.push(TreeItem::new_leaf(
                    format!("coin_{i}"),
                    format!("Coin: {}", v.coin),
                ));
                TreeItem::new(format!("unreg_cert_{i}"), "Unregistration", unreg_children)
                    .expect("Failed to create unregistration certificate node")
            }
            Some(Certificate::StakeVoteDelegCert(v)) => {
                let mut stake_vote_children = v
                    .stake_credential
                    .as_ref()
                    .map(map_cert_stake_credential)
                    .unwrap_or_default();
                stake_vote_children.push(TreeItem::new_leaf(
                    format!("pool_keyhash_{i}"),
                    format!("Pool Key Hash: {}", hex::encode(&v.pool_keyhash)),
                ));
                stake_vote_children.extend(map_drep(&v.drep, i));
                TreeItem::new(
                    format!("stake_vote_deleg_cert_{i}"),
                    "Stake and Vote Delegation",
                    stake_vote_children,
                )
                .expect("Failed to create stake and vote delegation certificate node")
            }
            Some(Certificate::StakeRegDelegCert(v)) => {
                let mut stake_reg_children = v
                    .stake_credential
                    .as_ref()
                    .map(map_cert_stake_credential)
                    .unwrap_or_default();
                stake_reg_children.push(TreeItem::new_leaf(
                    format!("pool_keyhash_{i}"),
                    format!("Pool Key Hash: {}", hex::encode(&v.pool_keyhash)),
                ));
                stake_reg_children.push(TreeItem::new_leaf(
                    format!("coin_{i}"),
                    format!("Coin: {}", v.coin),
                ));
                TreeItem::new(
                    format!("stake_reg_deleg_cert_{i}"),
                    "Stake Registration and Delegation",
                    stake_reg_children,
                )
                .expect("Failed to create stake registration and delegation certificate node")
            }
            Some(Certificate::VoteRegDelegCert(v)) => {
                let mut vote_reg_children = v
                    .stake_credential
                    .as_ref()
                    .map(map_cert_stake_credential)
                    .unwrap_or_default();
                vote_reg_children.extend(map_drep(&v.drep, i));
                vote_reg_children.push(TreeItem::new_leaf(
                    format!("coin_{i}"),
                    format!("Coin: {}", v.coin),
                ));
                TreeItem::new(
                    format!("vote_reg_deleg_cert_{i}"),
                    "Vote Registration and Delegation",
                    vote_reg_children,
                )
                .expect("Failed to create vote registration and delegation certificate node")
            }
            Some(Certificate::StakeVoteRegDelegCert(v)) => {
                let mut stake_vote_reg_children = v
                    .stake_credential
                    .as_ref()
                    .map(map_cert_stake_credential)
                    .unwrap_or_default();
                stake_vote_reg_children.push(TreeItem::new_leaf(
                    format!("pool_keyhash_{i}"),
                    format!("Pool Key Hash: {}", hex::encode(&v.pool_keyhash)),
                ));
                stake_vote_reg_children.extend(map_drep(&v.drep, i));
                stake_vote_reg_children.push(TreeItem::new_leaf(
                    format!("coin_{i}"),
                    format!("Coin: {}", v.coin),
                ));
                TreeItem::new(
                    format!("stake_vote_reg_deleg_cert_{i}"),
                    "Stake and Vote Registration and Delegation",
                    stake_vote_reg_children,
                )
                .expect(
                    "Failed to create stake and vote registration and delegation certificate node",
                )
            }
            Some(Certificate::AuthCommitteeHotCert(v)) => {
                let mut auth_committee_children = v
                    .committee_cold_credential
                    .as_ref()
                    .map(map_cert_stake_credential)
                    .unwrap_or_default();
                if let Some(hot_cred) = &v.committee_hot_credential {
                    auth_committee_children.extend(map_cert_stake_credential(hot_cred));
                }
                TreeItem::new(
                    format!("auth_committee_hot_cert_{i}"),
                    "Authorize Committee Hot Key",
                    auth_committee_children,
                )
                .expect("Failed to create authorize committee hot key certificate node")
            }
            Some(Certificate::ResignCommitteeColdCert(v)) => {
                let mut resign_committee_children = v
                    .committee_cold_credential
                    .as_ref()
                    .map(map_cert_stake_credential)
                    .unwrap_or_default();
                if let Some(anchor) = &v.anchor {
                    resign_committee_children.push(TreeItem::new_leaf(
                        format!("anchor_url_{i}"),
                        format!("Anchor URL: {}", anchor.url),
                    ));
                    resign_committee_children.push(TreeItem::new_leaf(
                        format!("anchor_hash_{i}"),
                        format!("Anchor Content Hash: {}", hex::encode(&anchor.content_hash)),
                    ));
                }
                TreeItem::new(
                    format!("resign_committee_cold_cert_{i}"),
                    "Resign Committee Cold Key",
                    resign_committee_children,
                )
                .expect("Failed to create resign committee cold key certificate node")
            }
            Some(Certificate::RegDrepCert(v)) => {
                let mut reg_drep_children = v
                    .drep_credential
                    .as_ref()
                    .map(map_cert_stake_credential)
                    .unwrap_or_default();
                reg_drep_children.push(TreeItem::new_leaf(
                    format!("coin_{i}"),
                    format!("Coin: {}", v.coin),
                ));
                if let Some(anchor) = &v.anchor {
                    reg_drep_children.push(TreeItem::new_leaf(
                        format!("anchor_url_{i}"),
                        format!("Anchor URL: {}", anchor.url),
                    ));
                    reg_drep_children.push(TreeItem::new_leaf(
                        format!("anchor_hash_{i}"),
                        format!("Anchor Content Hash: {}", hex::encode(&anchor.content_hash)),
                    ));
                }
                TreeItem::new(
                    format!("reg_drep_cert_{i}"),
                    "Register DRep",
                    reg_drep_children,
                )
                .expect("Failed to create register DRep certificate node")
            }
            Some(Certificate::UnregDrepCert(v)) => {
                let mut unreg_drep_children = v
                    .drep_credential
                    .as_ref()
                    .map(map_cert_stake_credential)
                    .unwrap_or_default();
                unreg_drep_children.push(TreeItem::new_leaf(
                    format!("coin_{i}"),
                    format!("Coin: {}", v.coin),
                ));
                TreeItem::new(
                    format!("unreg_drep_cert_{i}"),
                    "Unregister DRep",
                    unreg_drep_children,
                )
                .expect("Failed to create unregister DRep certificate node")
            }
            Some(Certificate::UpdateDrepCert(v)) => {
                let mut update_drep_children = v
                    .drep_credential
                    .as_ref()
                    .map(map_cert_stake_credential)
                    .unwrap_or_default();
                if let Some(anchor) = &v.anchor {
                    update_drep_children.push(TreeItem::new_leaf(
                        format!("anchor_url_{i}"),
                        format!("Anchor URL: {}", anchor.url),
                    ));
                    update_drep_children.push(TreeItem::new_leaf(
                        format!("anchor_hash_{i}"),
                        format!("Anchor Content Hash: {}", hex::encode(&anchor.content_hash)),
                    ));
                }
                TreeItem::new(
                    format!("update_drep_cert_{i}"),
                    "Update DRep",
                    update_drep_children,
                )
                .expect("Failed to create update DRep certificate node")
            }
            None => TreeItem::new_leaf(
                format!("unknown_cert_{i}"),
                "Unknown Certificate".to_string(),
            ),
        })
        .collect::<Vec<_>>();

    TreeItem::new("certificates".to_string(), "Certificates", certs_children)
        .expect("Failed to create certificates node")
}

fn map_redeemer<'a>(redeemer: &Option<Redeemer>, index: &str) -> Vec<TreeItem<'a, String>> {
    match redeemer {
        Some(redeemer) => {
            let purpose_str = match RedeemerPurpose::try_from(redeemer.purpose) {
                Ok(purpose) => format!("{purpose:?}"),
                Err(_) => format!("Unknown ({})", redeemer.purpose),
            };

            let mut children = vec![
                TreeItem::new_leaf(
                    format!("redeemer_purpose_{index}"),
                    format!("Purpose: {purpose_str}"),
                ),
                TreeItem::new_leaf(
                    format!("redeemer_index_{index}"),
                    format!("Index: {}", redeemer.index),
                ),
                TreeItem::new_leaf(
                    format!("redeemer_cbor_{index}"),
                    format!("Original CBOR: {}", hex::encode(&redeemer.original_cbor)),
                ),
            ];
            if let Some(ex_units) = &redeemer.ex_units {
                children.push(
                    TreeItem::new(
                        format!("redeemer_ex_units_{index}"),
                        "Execution Units",
                        vec![
                            TreeItem::new_leaf(
                                format!("ex_units_steps_{index}"),
                                format!("Steps: {}", ex_units.steps),
                            ),
                            TreeItem::new_leaf(
                                format!("ex_units_memory_{index}"),
                                format!("Memory: {}", ex_units.memory),
                            ),
                        ],
                    )
                    .expect("Failed to create ex_units node"),
                );
            }
            if let Some(payload) = &redeemer.payload {
                children.extend(map_plutus_data(payload, &format!("redeemer_{index}")));
            }
            vec![TreeItem::new(
                format!("redeemer_{index}"),
                "Redeemer".to_string(),
                children,
            )
            .expect("Failed to create redeemer node")]
        }
        None => vec![TreeItem::new_leaf(
            format!("redeemer_{index}"),
            "Redeemer: None".to_string(),
        )],
    }
}

fn map_plutus_data<'a>(plutus_data: &PlutusData, index: &str) -> Vec<TreeItem<'a, String>> {
    match &plutus_data.plutus_data {
        Some(plutus_data::PlutusData::Constr(constr)) => {
            let mut children = vec![
                TreeItem::new_leaf(
                    format!("constr_tag_{index}"),
                    format!("Tag: {}", constr.tag),
                ),
                TreeItem::new_leaf(
                    format!("constr_any_{index}"),
                    format!("Any Constructor: {}", constr.any_constructor),
                ),
            ];
            if !constr.fields.is_empty() {
                children.push(
                    TreeItem::new(
                        format!("constr_fields_{index}"),
                        "Fields",
                        constr
                            .fields
                            .iter()
                            .enumerate()
                            .flat_map(|(j, field)| map_plutus_data(field, &format!("{index}_{j}")))
                            .collect(),
                    )
                    .expect("Failed to create constr fields node"),
                );
            }
            vec![TreeItem::new(
                format!("plutus_constr_{index}"),
                "Constr".to_string(),
                children,
            )
            .expect("Failed to create constr node")]
        }
        Some(plutus_data::PlutusData::Map(map)) => {
            let children = map
                .pairs
                .iter()
                .enumerate()
                .map(|(j, pair)| {
                    let mut pair_children = vec![];
                    if let Some(key) = &pair.key {
                        pair_children.extend(map_plutus_data(key, &format!("{index}_{j}_key")));
                    } else {
                        pair_children.push(TreeItem::new_leaf(
                            format!("{index}_{j}_key"),
                            "Key: None".to_string(),
                        ));
                    }
                    if let Some(value) = &pair.value {
                        pair_children.extend(map_plutus_data(value, &format!("{index}_{j}_value")));
                    } else {
                        pair_children.push(TreeItem::new_leaf(
                            format!("{index}_{j}_value"),
                            "Value: None".to_string(),
                        ));
                    }
                    TreeItem::new(
                        format!("map_pair_{index}_{j}"),
                        format!("Pair {j}"),
                        pair_children,
                    )
                    .expect("Failed to create map pair node")
                })
                .collect();
            vec![
                TreeItem::new(format!("plutus_map_{index}"), "Map".to_string(), children)
                    .expect("Failed to create map node"),
            ]
        }
        Some(plutus_data::PlutusData::BigInt(big_int)) => {
            let value = match &big_int.big_int {
                Some(big_int::BigInt::Int(i)) => format!("Int: {i}"),
                Some(big_int::BigInt::BigUInt(bytes)) => format!("BigUInt: {}", hex::encode(bytes)),
                Some(big_int::BigInt::BigNInt(bytes)) => format!("BigNInt: {}", hex::encode(bytes)),
                None => "BigInt: None".to_string(),
            };
            vec![TreeItem::new_leaf(format!("plutus_bigint_{index}"), value)]
        }
        Some(plutus_data::PlutusData::BoundedBytes(bytes)) => {
            vec![TreeItem::new_leaf(
                format!("plutus_bytes_{index}"),
                format!("Bounded Bytes: {}", hex::encode(bytes)),
            )]
        }
        Some(plutus_data::PlutusData::Array(array)) => {
            let children = array
                .items
                .iter()
                .enumerate()
                .flat_map(|(j, item)| map_plutus_data(item, &format!("{index}_{j}")))
                .collect();
            vec![TreeItem::new(
                format!("plutus_array_{index}"),
                "Array".to_string(),
                children,
            )
            .expect("Failed to create array node")]
        }
        None => vec![TreeItem::new_leaf(
            format!("plutus_none_{index}"),
            "PlutusData: None".to_string(),
        )],
    }
}

fn map_datum<'a>(datum: &Option<Datum>, index: &str) -> Vec<TreeItem<'a, String>> {
    match datum {
        Some(datum) => {
            let mut children = vec![
                TreeItem::new_leaf(
                    format!("datum_hash_{index}"),
                    format!("Datum Hash: {}", hex::encode(&datum.hash)),
                ),
                TreeItem::new_leaf(
                    format!("original_cbor_{index}"),
                    format!("Original CBOR: {}", hex::encode(&datum.original_cbor)),
                ),
            ];
            if let Some(payload) = &datum.payload {
                children.extend(map_plutus_data(payload, &format!("datum_{index}")));
            }
            vec![
                TreeItem::new(format!("datum_{index}"), "Datum".to_string(), children)
                    .expect("Failed to create datum node"),
            ]
        }
        None => vec![TreeItem::new_leaf(
            format!("datum_{index}"),
            "Datum: None".to_string(),
        )],
    }
}

fn map_script<'a>(script: &Option<Script>, index: &str) -> Vec<TreeItem<'a, String>> {
    match script {
        Some(script) => {
            let (label, children) = match &script.script {
                Some(script::Script::Native(native)) => {
                    let native_children = map_native_script(native, &format!("native_{index}"));
                    ("Native Script".to_string(), native_children)
                }
                Some(script::Script::PlutusV1(bytes)) => (
                    "Plutus V1 Script".to_string(),
                    vec![TreeItem::new_leaf(
                        format!("plutus_v1_{index}"),
                        format!("Script: {}", hex::encode(bytes)),
                    )],
                ),
                Some(script::Script::PlutusV2(bytes)) => (
                    "Plutus V2 Script".to_string(),
                    vec![TreeItem::new_leaf(
                        format!("plutus_v2_{index}"),
                        format!("Script: {}", hex::encode(bytes)),
                    )],
                ),
                Some(script::Script::PlutusV3(bytes)) => (
                    "Plutus V3 Script".to_string(),
                    vec![TreeItem::new_leaf(
                        format!("plutus_v3_{index}"),
                        format!("Script: {}", hex::encode(bytes)),
                    )],
                ),
                None => ("Script: None".to_string(), vec![]),
            };
            vec![TreeItem::new(format!("script_{index}"), label, children)
                .expect("Failed to create script node")]
        }
        None => vec![TreeItem::new_leaf(
            format!("script_{index}"),
            "Script: None".to_string(),
        )],
    }
}

fn map_native_script<'a>(native: &NativeScript, index: &str) -> Vec<TreeItem<'a, String>> {
    match &native.native_script {
        Some(native_script::NativeScript::ScriptPubkey(bytes)) => {
            vec![TreeItem::new_leaf(
                format!("script_pubkey_{index}"),
                format!("Pubkey: {}", hex::encode(bytes)),
            )]
        }
        Some(native_script::NativeScript::ScriptAll(list)) => {
            let children = list
                .items
                .iter()
                .enumerate()
                .flat_map(|(j, item)| map_native_script(item, &format!("{index}_{j}")))
                .collect();
            vec![
                TreeItem::new(format!("script_all_{index}"), "All".to_string(), children)
                    .expect("Failed to create script all node"),
            ]
        }
        Some(native_script::NativeScript::ScriptAny(list)) => {
            let children = list
                .items
                .iter()
                .enumerate()
                .flat_map(|(j, item)| map_native_script(item, &format!("{index}_{j}")))
                .collect();
            vec![
                TreeItem::new(format!("script_any_{index}"), "Any".to_string(), children)
                    .expect("Failed to create script any node"),
            ]
        }
        Some(native_script::NativeScript::ScriptNOfK(n_of_k)) => {
            let mut children = vec![TreeItem::new_leaf(
                format!("n_of_k_{index}"),
                format!("K: {}", n_of_k.k),
            )];
            children.extend(
                n_of_k
                    .scripts
                    .iter()
                    .enumerate()
                    .flat_map(|(j, item)| map_native_script(item, &format!("{index}_{j}"))),
            );
            vec![TreeItem::new(
                format!("script_n_of_k_{index}"),
                "N of K".to_string(),
                children,
            )
            .expect("Failed to create script n of k node")]
        }
        Some(native_script::NativeScript::InvalidBefore(slot)) => {
            vec![TreeItem::new_leaf(
                format!("invalid_before_{index}"),
                format!("Invalid Before: {slot}"),
            )]
        }
        Some(native_script::NativeScript::InvalidHereafter(slot)) => {
            vec![TreeItem::new_leaf(
                format!("invalid_hereafter_{index}"),
                format!("Invalid Hereafter: {slot}"),
            )]
        }
        None => vec![TreeItem::new_leaf(
            format!("native_script_{index}"),
            "Native Script: None".to_string(),
        )],
    }
}

fn map_vkey_witness<'a>(vkey_witness: &VKeyWitness, index: &str) -> Vec<TreeItem<'a, String>> {
    vec![TreeItem::new(
        format!("vkey_witness_{index}"),
        "VKey Witness".to_string(),
        vec![
            TreeItem::new_leaf(
                format!("vkey_{index}"),
                format!("VKey: {}", hex::encode(&vkey_witness.vkey)),
            ),
            TreeItem::new_leaf(
                format!("signature_{index}"),
                format!("Signature: {}", hex::encode(&vkey_witness.signature)),
            ),
        ],
    )
    .expect("Failed to create vkey witness node")]
}

fn map_metadatum<'a>(metadatum: &Metadatum, index: &str) -> Vec<TreeItem<'a, String>> {
    match &metadatum.metadatum {
        Some(metadatum::Metadatum::Int(i)) => {
            vec![TreeItem::new_leaf(
                format!("metadatum_int_{index}"),
                format!("Int: {i}"),
            )]
        }
        Some(metadatum::Metadatum::Bytes(bytes)) => {
            vec![TreeItem::new_leaf(
                format!("metadatum_bytes_{index}"),
                format!("Bytes: {}", hex::encode(bytes)),
            )]
        }
        Some(metadatum::Metadatum::Text(text)) => {
            vec![TreeItem::new_leaf(
                format!("metadatum_text_{index}"),
                format!("Text: {text}"),
            )]
        }
        Some(metadatum::Metadatum::Array(array)) => {
            let children = array
                .items
                .iter()
                .enumerate()
                .flat_map(|(j, item)| map_metadatum(item, &format!("{index}_{j}")))
                .collect();
            vec![TreeItem::new(
                format!("metadatum_array_{index}"),
                "Array".to_string(),
                children,
            )
            .expect("Failed to create metadatum array node")]
        }
        Some(metadatum::Metadatum::Map(map)) => {
            let children = map
                .pairs
                .iter()
                .enumerate()
                .map(|(j, pair)| {
                    let mut pair_children = vec![];
                    if let Some(key) = &pair.key {
                        pair_children.extend(map_metadatum(key, &format!("{index}_{j}_key")));
                    } else {
                        pair_children.push(TreeItem::new_leaf(
                            format!("{index}_{j}_key"),
                            "Key: None".to_string(),
                        ));
                    }
                    if let Some(value) = &pair.value {
                        pair_children.extend(map_metadatum(value, &format!("{index}_{j}_value")));
                    } else {
                        pair_children.push(TreeItem::new_leaf(
                            format!("{index}_{j}_value"),
                            "Value: None".to_string(),
                        ));
                    }
                    TreeItem::new(
                        format!("metadatum_pair_{index}_{j}"),
                        format!("Pair {j}"),
                        pair_children,
                    )
                    .expect("Failed to create metadatum pair node")
                })
                .collect();
            vec![TreeItem::new(
                format!("metadatum_map_{index}"),
                "Map".to_string(),
                children,
            )
            .expect("Failed to create metadatum map node")]
        }
        None => vec![TreeItem::new_leaf(
            format!("metadatum_none_{index}"),
            "Metadatum: None".to_string(),
        )],
    }
}

fn map_withdrawal<'a>(withdrawal: &Withdrawal, index: &str) -> Vec<TreeItem<'a, String>> {
    let mut children = vec![
        TreeItem::new_leaf(
            format!("withdrawal_account_{index}"),
            format!(
                "Reward Account: {}",
                hex::encode(&withdrawal.reward_account)
            ),
        ),
        TreeItem::new_leaf(
            format!("withdrawal_coin_{index}"),
            format!("Coin: {}", withdrawal.coin),
        ),
    ];
    children.extend(map_redeemer(&withdrawal.redeemer, index));
    children
}

fn map_witness_set<'a>(
    witness_set: &Option<WitnessSet>,
    index: usize,
) -> Vec<TreeItem<'a, String>> {
    match witness_set {
        Some(witness_set) => {
            let mut children = vec![];
            if !witness_set.vkeywitness.is_empty() {
                children.push(
                    TreeItem::new(
                        format!("vkey_witnesses_{index}"),
                        "VKey Witnesses",
                        witness_set
                            .vkeywitness
                            .iter()
                            .enumerate()
                            .flat_map(|(j, vkey)| {
                                map_vkey_witness(vkey, &format!("{index}_vkeywitness_{j}"))
                            })
                            .collect(),
                    )
                    .expect("Failed to create vkey witnesses node"),
                );
            }
            if !witness_set.script.is_empty() {
                children.push(
                    TreeItem::new(
                        format!("scripts_{index}"),
                        "Scripts",
                        witness_set
                            .script
                            .iter()
                            .enumerate()
                            .flat_map(|(j, script)| {
                                map_script(&Some(script.clone()), &format!("{index}_script_{j}"))
                            })
                            .collect(),
                    )
                    .expect("Failed to create scripts node"),
                );
            }
            if !witness_set.plutus_datums.is_empty() {
                children.push(
                    TreeItem::new(
                        format!("plutus_datums_{index}"),
                        "Plutus Datums",
                        witness_set
                            .plutus_datums
                            .iter()
                            .enumerate()
                            .flat_map(|(j, datum)| map_plutus_data(datum, &format!("{index}_{j}")))
                            .collect(),
                    )
                    .expect("Failed to create plutus datums node"),
                );
            }
            vec![TreeItem::new(
                format!("witness_set_{index}"),
                "Witness Set".to_string(),
                children,
            )
            .expect("Failed to create witness set node")]
        }
        None => vec![TreeItem::new_leaf(
            format!("witness_set_{index}"),
            "Witness Set: None".to_string(),
        )],
    }
}

fn map_aux_data<'a>(aux_data: &Option<AuxData>, index: usize) -> Vec<TreeItem<'a, String>> {
    let mut children = vec![];

    if let Some(aux_data) = aux_data {
        if !aux_data.metadata.is_empty() {
            children.push(
                TreeItem::new(
                    format!("metadata_{index}"),
                    "Metadata",
                    aux_data
                        .metadata
                        .iter()
                        .enumerate()
                        .map(|(j, meta)| {
                            let mut meta_children = vec![TreeItem::new_leaf(
                                format!("metadata_label_{index}_{j}"),
                                format!("Label: {}", meta.label),
                            )];
                            if let Some(value) = &meta.value {
                                meta_children.extend(map_metadatum(value, &format!("{index}_{j}")));
                            }
                            TreeItem::new(
                                format!("metadata_{index}_{j}"),
                                format!("Metadata {j}"),
                                meta_children,
                            )
                            .expect("Failed to create metadata node")
                        })
                        .collect(),
                )
                .expect("Failed to create metadata node"),
            );
        }

        if !aux_data.scripts.is_empty() {
            children.push(
                TreeItem::new(
                    format!("aux_scripts_{index}"),
                    "Scripts",
                    aux_data
                        .scripts
                        .iter()
                        .enumerate()
                        .flat_map(|(j, script)| {
                            map_script(&Some(script.clone()), &format!("{index}_{j}"))
                        })
                        .collect(),
                )
                .expect("Failed to create aux scripts node"),
            );
        }
    };

    if !children.is_empty() {
        return vec![TreeItem::new(
            format!("aux_data_{index}"),
            "Auxiliary Data".to_string(),
            children,
        )
        .expect("Failed to create aux data node")];
    }

    vec![]
}

fn map_tx_validity<'a>(validity: &Option<TxValidity>, index: usize) -> Vec<TreeItem<'a, String>> {
    match validity {
        Some(validity) => {
            vec![TreeItem::new(
                format!("validity_{index}"),
                "Validity".to_string(),
                vec![
                    TreeItem::new_leaf(
                        format!("validity_start_{index}"),
                        format!("Start: {}", validity.start),
                    ),
                    TreeItem::new_leaf(
                        format!("validity_ttl_{index}"),
                        format!("TTL: {}", validity.ttl),
                    ),
                ],
            )
            .expect("Failed to create validity node")]
        }
        None => vec![TreeItem::new_leaf(
            format!("validity_{index}"),
            "Validity: None".to_string(),
        )],
    }
}

fn map_tx_input<'a>(input: &TxInput, index: &str, tx_hash: &str) -> Vec<TreeItem<'a, String>> {
    let mut children = vec![
        TreeItem::new_leaf(
            format!("input_hash_{index}"),
            format!("Hash: {}", hex::encode(&input.tx_hash)),
        ),
        TreeItem::new_leaf(
            format!("input_index_{index}"),
            format!("Index: {}", input.output_index),
        ),
    ];
    if let Some(as_output) = &input.as_output {
        children.extend([TreeItem::new(
            format!("input_as_output_{tx_hash}_{index}"),
            "Output Spent",
            vec![map_tx_output(
                as_output,
                input.output_index as usize,
                tx_hash,
            )],
        )
        .expect("Failed to create as_output input node")]);
    }
    children.extend(map_redeemer(&input.redeemer, index));
    vec![TreeItem::new(
        format!("input_{tx_hash}_{index}"),
        format!("{}#{}", hex::encode(&input.tx_hash), input.output_index),
        children,
    )
    .expect("Failed to create input node")]
}

fn map_tx_output<'a>(output: &TxOutput, index: usize, tx_hash: &str) -> TreeItem<'a, String> {
    let address = Address::from_bytes(&output.address)
        .map_or("decoded fail".to_string(), |addr| addr.to_string());
    let mut children = vec![
        TreeItem::new_leaf(
            format!("output_{tx_hash}_{index}_address"),
            format!("Address: {address}"),
        ),
        TreeItem::new_leaf(
            format!("output_{tx_hash}_{index}_coin"),
            format!("Coin: {}", output.coin),
        ),
    ];
    if !output.assets.is_empty() {
        children.push(
            TreeItem::new(
                format!("output_{tx_hash}_{index}_assets"),
                "Assets",
                output
                    .assets
                    .iter()
                    .enumerate()
                    .map(|(i, m)| {
                        let policy_id = hex::encode(&m.policy_id);
                        let mut asset_children = m
                            .assets
                            .iter()
                            .enumerate()
                            .map(|(j, asset)| {
                                let name = String::try_from(asset.name.to_vec())
                                    .unwrap_or(" - ".to_string());
                                TreeItem::new(
                                    format!("output_asset_{policy_id}_{i}_{j}"),
                                    format!("Asset: {name}"),
                                    vec![
                                        TreeItem::new_leaf(
                                            format!("output_asset_mintcoin_{policy_id}_{i}_{j}"),
                                            format!("Mint Coin: {}", asset.mint_coin),
                                        ),
                                        TreeItem::new_leaf(
                                            format!("output_asset_outputcoin_{policy_id}_{i}_{j}"),
                                            format!("Output Coin: {}", asset.output_coin),
                                        ),
                                    ],
                                )
                                .expect("Failed to create asset node")
                            })
                            .collect::<Vec<_>>();

                        asset_children.extend(map_redeemer(&m.redeemer, &format!("output_{i}")));

                        TreeItem::new(
                            format!("output_policy_{policy_id}_{i}"),
                            format!("Policy: {policy_id}"),
                            asset_children,
                        )
                        .expect("Failed to create output policy node")
                    })
                    .collect(),
            )
            .expect("Failed to create assets node"),
        );
    }
    children.extend(map_datum(&output.datum, &index.to_string()));
    children.extend(map_script(&output.script, &index.to_string()));
    TreeItem::new(
        format!("output_{tx_hash}_{index}"),
        format!("{tx_hash}#{index}"),
        children,
    )
    .expect("Failed to create output node")
}
