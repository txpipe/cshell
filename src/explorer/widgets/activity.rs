use chrono::{DateTime, Utc};
use ratatui::{
    style::{Color, Style, Stylize},
    widgets::{Block, Sparkline, Widget},
};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use crate::explorer::{App, ChainBlock};

fn get_last_slots(data: Rc<RefCell<VecDeque<ChainBlock>>>, size: usize) -> Vec<u64> {
    let data = data.borrow();
    let mut result = vec![0; size];

    if data.is_empty() {
        return result;
    }

    let last_blocks: Vec<_> = data.iter().take(size).collect();

    let max_slot = last_blocks.first().unwrap().slot;
    let min_slot = max_slot.saturating_sub(size as u64);

    for item in last_blocks {
        if item.slot <= max_slot && item.slot > min_slot {
            let index = (max_slot - item.slot) as usize;
            result[size - 1 - index] = item.tx_count as u64 + 1;
        }
    }

    result
}

#[derive(Clone, Default)]
pub struct ActivityMonitor {
    blocks: Rc<RefCell<VecDeque<ChainBlock>>>,
    last_block_seen: Option<DateTime<Utc>>,
}
impl From<&App> for ActivityMonitor {
    fn from(value: &App) -> Self {
        Self {
            blocks: Rc::clone(&value.chain.blocks),
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
        let mut points: VecDeque<u64> = get_last_slots(self.blocks, size).into();
        let (title, color) = match self.last_block_seen {
            Some(dt) => {
                let seconds = (Utc::now() - dt).num_seconds();
                for _ in 0..seconds {
                    points.push_back(0);
                    points.pop_front();
                }

                (
                    format!("Chain Activity | Updated {seconds} seconds ago"),
                    match seconds {
                        i64::MIN..=20 => Color::Green,
                        21..=30 => Color::Yellow,
                        _ => Color::Red,
                    },
                )
            }
            None => ("Chain Activity ".to_string(), Color::Green),
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
