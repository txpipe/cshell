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
        let vertical = Layout::vertical([Constraint::Percentage(30)]).flex(Flex::Center);
        let horizontal = Layout::horizontal([Constraint::Percentage(60)]).flex(Flex::Center);
        let [area] = vertical.areas(area);
        let [area] = horizontal.areas(area);
        let help = Paragraph::new(vec![
            Line::default(),
            Line::from("  q   : Quit CShell"),
            Line::from("  esc : Go back or close popup"),
            Line::from("  ?   : Show this help"),
            Line::default(),
            Line::from("Navigation"),
            Line::from("  Tab | shift + Tab     : Scroll through tabs"),
            Line::from("  j | \u{1F883}         : Scroll down"),
            Line::from("  k | \u{1F881}         : Scroll up"),
            Line::from("  j | \u{1F883} + Shift : Scroll to bottom"),
            Line::from("  k | \u{1F881} + Shift : Scroll to top"),
            Line::default(),
            Line::from("Search"),
            Line::from("  f | / : Focus on filter"),
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
