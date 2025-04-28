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
use utxorpc::spec::cardano;

use crate::explorer::{App, ChainBlock};

#[derive(Default)]
pub struct TransactionsTabState {
    pub scroll_state: ScrollbarState,
    pub table_state: TableState,
}

#[derive(Clone)]
pub struct TransactionsTab {
    pub blocks: Vec<ChainBlock>,
}

impl From<&App> for TransactionsTab {
    fn from(value: &App) -> Self {
        Self {
            blocks: value.chain.blocks.clone(),
        }
    }
}

struct TxView {
    hash: String,
    slot: u64,
    certs: usize,
    assets: usize,
    amount_ada: u64,
    datum: bool,
}
impl TxView {
    pub fn new(slot: u64, body: &cardano::BlockBody) -> Vec<Self> {
        body.tx
            .iter()
            .map(|tx| Self {
                hash: hex::encode(&tx.hash),
                slot,
                certs: tx.certificates.len(),
                assets: tx.outputs.iter().map(|o| o.assets.len()).sum(),
                amount_ada: tx.outputs.iter().map(|o| o.coin).sum(),
                datum: tx.outputs.iter().any(|o| match &o.datum {
                    Some(datum) => !datum.hash.is_empty(),
                    None => false,
                }),
            })
            .collect()
    }
}

impl StatefulWidget for TransactionsTab {
    type State = TransactionsTabState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State)
    where
        Self: Sized,
    {
        let header = ["Hash", "Slot", "Certs", "Assets", "Total Coin", "Datum"]
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(Style::default().fg(Color::Green).bold())
            .height(1);

        let txs = self.blocks.iter().flat_map(|chain_block| {
            if let Some(body) = &chain_block.body {
                return TxView::new(chain_block.slot, body);
            }
            Default::default()
        });

        let rows = txs.enumerate().map(|(i, tx)| {
            let color = match i % 2 {
                0 => Color::Black,
                _ => Color::Reset,
            };
            Row::new(vec![
                format!("\n{}\n", tx.hash),
                format!("\n{}\n", tx.slot),
                format!("\n{}\n", tx.certs),
                format!("\n{}\n", tx.assets),
                format!("\n{}\n", tx.amount_ada),
                format!("\n{}\n", if tx.datum { "yes" } else { "no" }),
            ])
            .style(Style::new().fg(Color::White).bg(color))
            .height(3)
        });

        let bar = " █ ";
        let table = Table::new(
            rows,
            [
                Constraint::Fill(1),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(12),
            ],
        )
        .header(header)
        .row_highlight_style(Modifier::BOLD)
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
