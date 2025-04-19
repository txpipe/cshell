use std::collections::HashMap;
use std::sync::Arc;

use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Text};
use ratatui::widgets::{
    Block, Cell, HighlightSpacing, List, ListItem, ListState, Padding, Paragraph, Row,
    StatefulWidget, Table, TableState, Widget,
};

use crate::explorer::{App, ExplorerContext};
use crate::types::DetailedBalance;
use crate::utils::clip;

#[derive(Default)]
pub struct AccountsTabState {
    pub list_state: ListState,
    pub table_state: TableState,
    pub focus_on_table: bool,
}

#[derive(Clone)]
pub struct AccountsTab {
    pub context: Arc<ExplorerContext>,
    pub balances: HashMap<String, DetailedBalance>,
}

impl From<&App> for AccountsTab {
    fn from(value: &App) -> Self {
        Self {
            context: value.context.clone(),
            balances: value.chain.balances.clone(),
        }
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
            Layout::vertical([Constraint::Length(6), Constraint::Fill(1)]).areas(details_area);

        let block = Block::bordered().title(Line::raw(" Accounts ").centered());

        let items: Vec<ListItem> = self
            .context
            .store
            .wallets()
            .iter()
            .map(|wallet| {
                ListItem::new(vec![
                    Line::styled(wallet.name.to_string(), Color::Gray),
                    Line::styled(
                        clip(wallet.address(self.context.provider.is_testnet()), 20),
                        Color::DarkGray,
                    ),
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
            let wallet =
                self.context.store.wallets()[i % self.context.store.wallets().len()].clone();
            let key = wallet
                .address(self.context.provider.is_testnet())
                .to_string();

            let balance = self.balances.get(&key);
            let mut details = vec![
                Line::styled(
                    format!("{} wallet", wallet.name),
                    (Color::White, Modifier::UNDERLINED),
                ),
                Line::styled(format!("Address: {}", &key), Color::White),
            ];
            if let Some(balance) = balance {
                let coin: u64 = balance
                    .iter()
                    .map(|utxo| utxo.coin.parse::<u64>().unwrap())
                    .sum();
                details.push(Line::styled(
                    format!("Balance: {} Lovelace", coin),
                    Color::White,
                ));
            }

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
            let Some(balance) = balance else { return };
            let header = ["TRANSACTION", "INDEX", "COIN", "ASSETS", "DATUM"]
                .into_iter()
                .map(Cell::from)
                .collect::<Row>()
                .style(Style::default().fg(Color::Green).bold())
                .height(1);

            let rows = balance.iter().map(|utxo| {
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
                    Constraint::Length(12),
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
