use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::Line,
    widgets::{Block, Padding, Paragraph, Tabs, Widget},
};

use crate::{
    explorer::{App, SelectedTab},
    provider::types::Provider,
};

#[derive(Clone)]
pub struct Header {
    pub selected_tab: SelectedTab,
    pub tip: Option<u64>,
    pub provider: Provider,
    pub disconnected: bool,
}
impl From<&App> for Header {
    fn from(value: &App) -> Self {
        Self {
            selected_tab: value.selected_tab.clone(),
            tip: value.chain.tip,
            provider: value.context.provider.clone(),
            disconnected: value.disconnected,
        }
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

        let name = if self.disconnected {
            Paragraph::new(vec![
                Line::from(" CShell Explorer ".to_string()),
                Line::from(" Disconnected ").style(Style::new().bold()),
                Line::from(format!(" Provider: {} ", self.provider.name())),
            ])
            .centered()
            .block(
                Block::bordered()
                    .border_style(Style::new().red())
                    .title(" Provider "),
            )
            .style(Style::default().fg(ratatui::style::Color::Red))
        } else {
            Paragraph::new(match self.tip {
                Some(tip) => vec![
                    Line::from(" CShell Explorer ".to_string()),
                    Line::from(format!(" Tip: {} ", tip)),
                    Line::from(format!(" Provider: {} ", self.provider.name())),
                ],
                None => vec![Line::from(" CShell Explorer ".to_string())],
            })
            .centered()
            .block(
                Block::bordered()
                    .border_style(Style::new().blue())
                    .title(" Provider "),
            )
            .style(Style::default().fg(ratatui::style::Color::Blue))
        };

        name.render(provider_area, buf);
    }
}
