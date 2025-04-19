use chrono::{DateTime, Utc};
use ratatui::{
    style::{Color, Style, Stylize},
    widgets::{Block, Sparkline, Widget},
};
use std::collections::VecDeque;

use crate::explorer::{App, ChainBlock};

fn get_last_slots(data: &[ChainBlock]) -> Vec<u64> {
    let mut result = vec![0; 200];
    if data.is_empty() {
        return result;
    }

    let start_index = if data.len() > 200 {
        data.len() - 200
    } else {
        0
    };

    let last_200_slots = &data[start_index..];
    let max_slot = last_200_slots.last().unwrap().slot;

    for item in last_200_slots {
        if item.slot <= max_slot && item.slot > max_slot - 200 {
            let index = (max_slot - item.slot) as usize;
            result[index] = item.tx_count as u64 + 1;
        }
    }
    result
}

#[derive(Clone, Default)]
pub struct ActivityMonitor {
    pub points: VecDeque<u64>,
    pub last_block_seen: Option<DateTime<Utc>>,
}

impl From<&App> for ActivityMonitor {
    fn from(value: &App) -> Self {
        Self {
            points: get_last_slots(&value.chain.blocks).into(),
            last_block_seen: value.chain.last_block_seen,
        }
    }
}

impl Widget for ActivityMonitor {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let title = match self.last_block_seen {
            Some(dt) => {
                let seconds = (Utc::now() - dt).num_seconds();
                format!(" Activity | Updated {} seconds ago", seconds)
            }
            None => " Activity ".to_string(),
        };

        let sparkline = Sparkline::default()
            .block(
                Block::bordered()
                    .border_style(Style::new().dark_gray())
                    .title(title),
            )
            .data(&self.points)
            .style(Style::default().fg(Color::Green));
        sparkline.render(area, buf);
    }
}
