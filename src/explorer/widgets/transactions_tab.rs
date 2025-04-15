use std::sync::Arc;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Color,
    widgets::{Block, Paragraph, Widget},
};

use crate::explorer::ExplorerContext;

#[derive(Clone)]
pub struct TransactionsTab {
    pub context: Arc<ExplorerContext>,
}
impl TransactionsTab {
    pub fn new(context: Arc<ExplorerContext>) -> Self {
        Self { context }
    }
}
impl Widget for TransactionsTab {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        Paragraph::new("Welcome to the transactions tabs example!")
            .block(
                Block::bordered()
                    // .border_set(symbols::border::PROPORTIONAL_TALL)
                    .padding(ratatui::widgets::Padding::horizontal(1))
                    .border_style(Color::DarkGray),
            )
            .render(area, buf)
    }
}
