use std::collections::HashMap;
use std::sync::Arc;

use comfy_table::Table;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{
    Block, HighlightSpacing, List, ListItem, ListState, Padding, Paragraph, StatefulWidget, Widget,
};

use crate::explorer::{App, ExplorerContext};
use crate::types::DetailedBalance;
use crate::utils::clip;

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

pub fn balance_to_lines(balance: &DetailedBalance) -> Vec<Line> {
    let mut lines = vec![];
    if !balance.is_empty() {
        lines.push(Line::from("UTxOs"));
        lines.push(Line::from("====="));
    } else {
        lines.push(Line::from("No UTXOs found for this wallet"));
    }
    for utxo in balance {
        lines.push(Line::from(""));
        lines.push(Line::from(format!(
            "* {}#{}",
            hex::encode(&utxo.tx),
            utxo.tx_index
        )));
        lines.push(format!("  * Lovelace: {}", utxo.coin).into());

        if let Some(datum) = &utxo.datum {
            lines.push(format!("  * Datum: {}", hex::encode(datum.hash.clone())).into());
        }

        if !utxo.assets.is_empty() {
            lines.push("".into());
            lines.push("  * Assets:".into());

            let mut table = Table::new();
            table.set_header(vec!["Policy", "Asset", "Output Coin"]);

            for entry in &utxo.assets {
                for asset in &entry.assets {
                    table.add_row(vec![
                        hex::encode(&entry.policy_id),
                        hex::encode(&asset.name),
                        asset.output_coin.clone(),
                    ]);
                }
            }
            lines.push(format!("{}", table).into());
        }
    }

    lines
}

impl StatefulWidget for AccountsTab {
    type State = ListState;
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

        StatefulWidget::render(list, accounts_area, buf, state);

        // Handle details area:
        if let Some(i) = state.selected() {
            let wallet =
                self.context.store.wallets()[i % self.context.store.wallets().len()].clone();
            let key = wallet
                .address(self.context.provider.is_testnet())
                .to_string();

            let mut details = vec![
                Line::styled(
                    format!("{} wallet", wallet.name),
                    (Color::White, Modifier::UNDERLINED),
                ),
                Line::styled(key.clone(), Color::White),
                Line::from(""),
            ];

            if let Some(balance) = self.balances.get(&key) {
                details.extend(balance_to_lines(balance));
            }

            Paragraph::new(details)
                .block(
                    Block::bordered()
                        .title(" Details ")
                        .padding(Padding::horizontal(1)),
                )
                .render(details_area, buf);
        } else {
            Paragraph::new("Select a wallet to show its balance")
                .block(Block::bordered().title(" Details "))
                .render(details_area, buf);
        };
    }
}
