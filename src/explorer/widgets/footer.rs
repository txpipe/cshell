use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{Paragraph, Widget},
};

#[derive(Clone)]
pub struct Footer {}
impl Footer {
    pub fn new() -> Self {
        Self {}
    }
}
impl Widget for Footer {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        Paragraph::new("Press ? for help")
            .centered()
            .render(area, buf)
    }
}
