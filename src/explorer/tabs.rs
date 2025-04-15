use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{StatefulWidget, Widget},
};
use strum::Display;

use super::widgets::{
    accounts_tab::AccountsTab, blocks_tab::BlocksTab, transactions_tab::TransactionsTab,
};

#[derive(Clone, Display)]
pub enum SelectedTab {
    #[strum(to_string = "Accounts")]
    Accounts(AccountsTab),
    #[strum(to_string = "Blocks")]
    Blocks(BlocksTab),
    #[strum(to_string = "Txs")]
    Transactions(TransactionsTab),
}

impl SelectedTab {
    pub fn previous(&self) -> Self {
        match self {
            Self::Accounts(tab) => Self::Transactions(TransactionsTab::new(tab.context.clone())),
            Self::Blocks(tab) => Self::Accounts(AccountsTab::new(tab.context.clone())),
            Self::Transactions(tab) => Self::Blocks(BlocksTab::new(tab.context.clone())),
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Accounts(tab) => Self::Blocks(BlocksTab::new(tab.context.clone())),
            Self::Blocks(tab) => Self::Transactions(TransactionsTab::new(tab.context.clone())),
            Self::Transactions(tab) => Self::Accounts(AccountsTab::new(tab.context.clone())),
        }
    }
}

impl Widget for SelectedTab {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // in a real app these might be separate widgets
        match self {
            Self::Accounts(mut accounts_tab) => {
                accounts_tab
                    .clone()
                    .render(area, buf, &mut accounts_tab.list_state)
            }
            Self::Blocks(blocks_tab) => blocks_tab.render(area, buf),
            Self::Transactions(transactions_tab) => transactions_tab.render(area, buf),
        }
    }
}
