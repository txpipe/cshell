use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::Line,
    widgets::{Block, Padding, Paragraph, Tabs, Widget},
};

use crate::{
    explorer::{event::ConnectionState, App, SelectedTab},
    provider::types::Provider,
};

#[derive(Clone)]
pub struct Header {
    pub selected_tab: SelectedTab,
    pub tip: Option<u64>,
    pub provider: Provider,
    pub app_state: ConnectionState,
}
impl From<&App> for Header {
    fn from(value: &App) -> Self {
        Self {
            selected_tab: value.selected_tab.clone(),
            tip: value.chain.tip,
            provider: value.context.provider.clone(),
            app_state: value.app_state.clone(),
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

        let tip = self.tip.map(|tip| tip.to_string()).unwrap_or("-".into());
        let text = vec![
            Line::from(format!(" Provider: {} ", self.provider.name())),
            Line::from(format!(" Status..: {} ", self.app_state)),
            Line::from(format!(" Tip.....: {} ", tip)),
        ];

        let (color, style) = match self.app_state {
            ConnectionState::Connected => (Color::Blue, Style::new().blue()),
            ConnectionState::Retrying => (Color::Yellow, Style::new().yellow()),
            ConnectionState::Disconnected => (Color::Red, Style::new().red()),
        };

        let name = Paragraph::new(text)
            .block(Block::bordered().border_style(style).title(" Connection "))
            .style(Style::default().fg(color));

        name.render(provider_area, buf);
    }
}
