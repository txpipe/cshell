use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Color,
    widgets::{Block, Paragraph, Widget},
};

#[derive(Clone)]
pub struct TransactionsTab {}
impl Widget for TransactionsTab {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        Paragraph::new("Transactions tab")
            .block(
                Block::bordered()
                    // .border_set(symbols::border::PROPORTIONAL_TALL)
                    .padding(ratatui::widgets::Padding::horizontal(1))
                    .border_style(Color::DarkGray),
            )
            .render(area, buf)
    }
}
