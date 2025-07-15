use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Text};
use ratatui::widgets::{
    Block, Cell, HighlightSpacing, List, ListItem, ListState, Padding, Paragraph, Row,
    StatefulWidget, Table, TableState, Widget,
};

use crate::explorer::{ExplorerContext, ExplorerWallet};
use crate::utils::clip;

#[derive(Default)]
pub struct AccountsTabState {
    list_state: ListState,
    table_state: TableState,
    focus_on_table: bool,
}
impl AccountsTabState {
    pub fn handle_key(&mut self, key: &KeyEvent) {
        match (key.code, key.modifiers) {
            (KeyCode::Char('l') | KeyCode::Right, _) => {
                if self.list_state.selected().is_some() {
                    self.focus_on_table = true;
                    self.table_state.select_next();
                }
            }
            (KeyCode::Char('h') | KeyCode::Left, _) => {
                if self.focus_on_table {
                    self.focus_on_table = false;
                    self.table_state.select(None);
                }
            }
            (KeyCode::Char('j') | KeyCode::Down, _) => {
                if self.focus_on_table {
                    self.table_state.select_next()
                } else {
                    self.list_state.select_next()
                }
            }
            (KeyCode::Char('k') | KeyCode::Up, _) => {
                if self.focus_on_table {
                    self.table_state.select_previous()
                } else {
                    self.list_state.select_previous()
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone)]
pub struct AccountsTab {
    pub context: Arc<ExplorerContext>,
}
impl AccountsTab {
    pub fn new(context: Arc<ExplorerContext>) -> Self {
        Self { context }
    }
}

impl StatefulWidget for AccountsTab {
    type State = AccountsTabState;
    fn render(
        self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
        state: &mut Self::State,
    ) where
        Self: Sized,
    {
        let [accounts_area, details_area] =
            Layout::horizontal([Constraint::Length(30), Constraint::Fill(1)]).areas(area);
        let [summary_area, utxos_area] =
            Layout::vertical([Constraint::Length(7), Constraint::Fill(1)]).areas(details_area);

        let block = Block::bordered().title(Line::raw(" Accounts ").centered());

        let guard = tokio::task::block_in_place(|| self.context.wallets.blocking_read());
        let wallets: Vec<(String, ExplorerWallet)> = guard
            .iter()
            .map(|(address, wallet)| (address.to_string(), wallet.clone()))
            .collect();

        let items: Vec<ListItem> = wallets
            .iter()
            .map(|(address, wallet)| {
                ListItem::new(vec![
                    Line::styled(wallet.name.to_string(), Color::Gray),
                    Line::styled(clip(address, 20), Color::DarkGray),
                ])
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::new().fg(Color::Green).add_modifier(Modifier::BOLD))
            .highlight_symbol("> ")
            .highlight_spacing(HighlightSpacing::Always);

        StatefulWidget::render(list, accounts_area, buf, &mut state.list_state);

        // Handle details area:
        if let Some(i) = state.list_state.selected() {
            let index = i % wallets.len();
            let (address, wallet) = &wallets[index];

            let mut details = vec![
                Line::styled(
                    format!("{} wallet", wallet.name),
                    (Color::White, Modifier::UNDERLINED),
                ),
                Line::styled(format!("Address: {}", &address), Color::White),
            ];

            let coin: u64 = wallet
                .balance
                .iter()
                .map(|utxo| utxo.coin.parse::<u64>().unwrap())
                .sum();
            details.push(Line::styled(
                format!("Balance: {} Lovelace", coin),
                Color::White,
            ));

            Block::bordered()
                .title(" Details ")
                .padding(Padding::horizontal(1))
                .render(details_area, buf);
            Paragraph::new(details.clone())
                .block(
                    Block::bordered()
                        .title(" Details ")
                        .padding(Padding::uniform(1)),
                )
                .render(summary_area, buf);

            // UTXOs table
            let header = ["Transaction", "Index", "Coin", "Assets", "Datum"]
                .into_iter()
                .map(Cell::from)
                .collect::<Row>()
                .style(Style::default().fg(Color::Green).bold())
                .height(1);

            let rows = wallet.balance.iter().map(|utxo| {
                Row::new(vec![
                    format!("\n{}\n", hex::encode(&utxo.tx)),
                    format!("\n{}\n", utxo.tx_index),
                    format!("\n{}\n", utxo.coin),
                    format!("\n{}\n", utxo.assets.len()),
                    format!(
                        "\n{}\n",
                        match &utxo.datum {
                            Some(datum) => clip(hex::encode(&datum.hash), 8),
                            None => "[Empty]".to_string(),
                        }
                    ),
                ])
                .style(Style::new().fg(Color::White))
                .height(3)
            });
            let bar = " █ ";
            let table = Table::new(
                rows,
                [
                    Constraint::Length(70),
                    Constraint::Length(6),
                    Constraint::Length(20),
                    Constraint::Length(8),
                    Constraint::Fill(1),
                ],
            )
            .header(header)
            .row_highlight_style(Modifier::BOLD)
            .highlight_symbol(Text::from(vec!["".into(), bar.into(), "".into()]))
            .highlight_spacing(HighlightSpacing::Always)
            .block(Block::bordered().border_style(if state.focus_on_table {
                Color::Green
            } else {
                Color::White
            }));
            StatefulWidget::render(table, utxos_area, buf, &mut state.table_state);
        } else {
            Paragraph::new("Select a wallet to show its balance")
                .block(Block::bordered().title(" Details "))
                .render(details_area, buf);
        };
    }
}
