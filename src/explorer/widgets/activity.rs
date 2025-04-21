use chrono::{DateTime, Utc};
use ratatui::{
    style::{Color, Style, Stylize},
    widgets::{Block, Sparkline, Widget},
};
use std::collections::VecDeque;

use crate::explorer::{App, ChainBlock};

fn get_last_slots(data: &[ChainBlock], size: usize) -> Vec<u64> {
    let mut result = vec![0; size];
    if data.is_empty() {
        return result;
    }

    let start_index = if data.len() > size {
        data.len() - size
    } else {
        0
    };

    let last_200_slots = &data[start_index..];
    let max_slot = last_200_slots.last().unwrap().slot;
    let min_slot = if max_slot > (size as u64) {
        max_slot - (size as u64)
    } else {
        0
    };

    for item in last_200_slots {
        if item.slot <= max_slot && item.slot > min_slot {
            let index = (max_slot - item.slot) as usize;
            result[index] = item.tx_count as u64 + 1;
        }
    }
    result.reverse();
    result
}

#[derive(Clone, Default)]
pub struct ActivityMonitor {
    pub blocks: Vec<ChainBlock>,
    pub last_block_seen: Option<DateTime<Utc>>,
}

impl From<&App> for ActivityMonitor {
    fn from(value: &App) -> Self {
        Self {
            blocks: value.chain.blocks.clone(),
            last_block_seen: value.chain.last_block_seen,
        }
    }
}

impl Widget for ActivityMonitor {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let size = area.width as usize - 2;
        let mut points: VecDeque<u64> = get_last_slots(&self.blocks, size).into();
        let (title, color) = match self.last_block_seen {
            Some(dt) => {
                let seconds = (Utc::now() - dt).num_seconds();
                for _ in 0..seconds {
                    points.push_back(0);
                    points.pop_front();
                }

                (
                    format!(" Activity | Updated {} seconds ago", seconds),
                    match seconds {
                        i64::MIN..=20 => Color::Green,
                        21..=30 => Color::Yellow,
                        _ => Color::Red,
                    },
                )
            }
            None => (" Activity ".to_string(), Color::Green),
        };

        let sparkline = Sparkline::default()
            .block(
                Block::bordered()
                    .border_style(Style::new().dark_gray())
                    .title(title),
            )
            .data(&points)
            .style(Style::default().fg(color));
        sparkline.render(area, buf);
    }
}
