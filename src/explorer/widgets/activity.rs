use rand::{
    distributions::{Distribution, Uniform},
    rngs::ThreadRng,
};
use ratatui::{
    style::{Color, Style, Stylize},
    widgets::{Block, Sparkline, Widget},
};

#[derive(Clone)]
pub struct ActivityMonitor {
    distribution: Uniform<u64>,
    rng: ThreadRng,
}

impl ActivityMonitor {
    pub fn new() -> Self {
        Self {
            distribution: Uniform::new(0, 100),
            rng: rand::thread_rng(),
        }
    }
}

impl Iterator for ActivityMonitor {
    type Item = u64;
    fn next(&mut self) -> Option<u64> {
        Some(self.distribution.sample(&mut self.rng))
    }
}

impl Widget for ActivityMonitor {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let mut signal = ActivityMonitor::new();
        let data1 = signal.by_ref().take(200).collect::<Vec<u64>>();
        let sparkline = Sparkline::default()
            .block(
                Block::bordered()
                    .border_style(Style::new().dark_gray())
                    .title(" Activity "),
            )
            .data(&data1)
            .style(Style::default().fg(Color::Green));
        sparkline.render(area, buf);
    }
}
