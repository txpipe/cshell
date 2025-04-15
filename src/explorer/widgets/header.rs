use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    widgets::{Block, Padding, Paragraph, Tabs, Widget},
};

use crate::explorer::tabs::SelectedTab;

#[derive(Clone)]
pub struct Header {
    pub selected_tab: SelectedTab,
}
impl Header {
    pub fn new(selected_tab: SelectedTab) -> Self {
        Self { selected_tab }
    }
}
impl Widget for Header {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let [title_area, provider_area] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(30)]).areas(area);

        let titles = ["Accounts", "Blocks", "Txs"];
        let selected_tab_index = match self.selected_tab {
            SelectedTab::Accounts(_) => 0,
            SelectedTab::Blocks(_) => 1,
            SelectedTab::Transactions(_) => 2,
        };
        Tabs::new(titles)
            .highlight_style((Color::Green, Modifier::BOLD))
            .select(selected_tab_index)
            .padding(" ", " ")
            .divider("|")
            .block(
                Block::bordered()
                    .padding(Padding::vertical(1))
                    .title(" Navigation ")
                    .border_style(Style::new().dark_gray()),
            )
            .render(title_area, buf);

        let name = Paragraph::new(" CShell Explorer ")
            .centered()
            .block(Block::bordered().border_style(Style::new().blue()))
            .style(Style::default().fg(ratatui::style::Color::Blue).bold());
        name.render(provider_area, buf);
    }
}
