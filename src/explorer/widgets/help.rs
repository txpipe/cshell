use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Stylize},
    text::Line,
    widgets::{Block, Clear, Paragraph, Widget},
};

#[derive(Clone)]
pub struct HelpPopup {}
impl HelpPopup {
    pub fn new() -> Self {
        Self {}
    }
}
impl Widget for HelpPopup {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let vertical = Layout::vertical([Constraint::Percentage(20)]).flex(Flex::Center);
        let horizontal = Layout::horizontal([Constraint::Percentage(60)]).flex(Flex::Center);
        let [area] = vertical.areas(area);
        let [area] = horizontal.areas(area);
        let help = Paragraph::new(vec![
            Line::from(""),
            Line::from("Tab | shift + Tab: Scroll through tabs"),
            Line::from("q | Esc : Quit CShell"),
            Line::from("? : Show this help"),
        ])
        .style((Color::Black, Modifier::BOLD))
        .block(
            Block::bordered()
                .title(Line::from("Help").style((Color::Black, Modifier::BOLD)))
                .padding(ratatui::widgets::Padding::horizontal(1))
                .border_style(Color::Yellow)
                .bg(Color::Yellow),
        );
        Clear.render(area, buf);
        help.render(area, buf);
    }
}
