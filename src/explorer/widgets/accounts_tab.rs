use std::sync::Arc;

use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{
    Block, HighlightSpacing, List, ListItem, ListState, Padding, Paragraph, StatefulWidget, Widget,
};

use crate::explorer::ExplorerContext;
use crate::utils::clip;

#[derive(Clone)]
pub struct AccountsTab {
    pub context: Arc<ExplorerContext>,
    pub list_state: ListState,
}

impl AccountsTab {
    pub fn new(context: Arc<ExplorerContext>) -> Self {
        Self {
            context,
            list_state: ListState::default(),
        }
    }

    pub fn select_next(&mut self) {
        self.list_state.select_next();
    }

    pub fn select_previous(&mut self) {
        self.list_state.select_previous();
    }

    pub fn select_first(&mut self) {
        self.list_state.select_first();
    }

    pub fn select_last(&mut self) {
        self.list_state.select_last();
    }
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
        if let Some(i) = self.list_state.selected() {
            let wallet =
                self.context.store.wallets()[i % self.context.store.wallets().len()].clone();
            Paragraph::new(vec![
                Line::styled(
                    format!("{} wallet", wallet.name),
                    (Color::White, Modifier::UNDERLINED),
                ),
                Line::styled(wallet.address(true).to_string(), Color::White),
            ])
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
