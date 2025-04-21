use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Margin, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::Text,
    widgets::{
        Block, Cell, HighlightSpacing, Row, Scrollbar, ScrollbarState, StatefulWidget, Table,
        TableState,
    },
};

use crate::explorer::{App, ChainBlock};

#[derive(Default)]
pub struct BlocksTabState {
    pub scroll_state: ScrollbarState,
    pub table_state: TableState,
}

#[derive(Clone)]
pub struct BlocksTab {
    pub blocks: Vec<ChainBlock>,
}

impl From<&App> for BlocksTab {
    fn from(value: &App) -> Self {
        Self {
            blocks: value.chain.blocks.clone(),
        }
    }
}

impl StatefulWidget for BlocksTab {
    type State = BlocksTabState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State)
    where
        Self: Sized,
    {
        let header = ["Slot", "Hash", "Number", "Tx Count"]
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(Style::default().fg(Color::Green).bold())
            .height(1);

        let rows = self.blocks.iter().enumerate().map(|(i, block)| {
            let color = match i % 2 {
                0 => Color::Black,
                _ => Color::Reset,
            };
            Row::new(vec![
                format!("\n{}\n", block.slot),
                format!("\n{}\n", hex::encode(&block.hash)),
                format!("\n{}\n", block.number),
                format!("\n{}\n", block.tx_count),
            ])
            .style(Style::new().fg(Color::White).bg(color))
            .height(3)
        });
        let bar = " █ ";
        let table = Table::new(
            rows,
            [
                Constraint::Length(12),
                Constraint::Fill(1),
                Constraint::Length(12),
                Constraint::Length(10),
            ],
        )
        .header(header)
        .row_highlight_style(Modifier::BOLD)
        // .column_highlight_style((Color::LightGreen, Modifier::BOLD))
        // .cell_highlight_style((Color::LightGreen, Modifier::BOLD))
        .highlight_symbol(Text::from(vec!["".into(), bar.into(), "".into()]))
        .highlight_spacing(HighlightSpacing::Always)
        .block(Block::bordered());
        StatefulWidget::render(table, area, buf, &mut state.table_state);

        StatefulWidget::render(
            Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight),
            area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
            buf,
            &mut state.scroll_state,
        );
    }
}
