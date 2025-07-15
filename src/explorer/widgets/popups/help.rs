use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Stylize},
    text::Line,
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget},
};

use super::centered_rect;

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
        let popup_area = centered_rect(60, 35, area);

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
            Line::default(),
            Line::from("Account"),
            Line::from("  i   : Add a temp account address"),
        ])
        .block(
            Block::bordered()
                .title(" Help | press ESC to go back ")
                .padding(ratatui::widgets::Padding::horizontal(1))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Color::Green)
                .bg(Color::Black),
        );

        Clear.render(popup_area, buf);
        help.render(popup_area, buf);
    }
}
