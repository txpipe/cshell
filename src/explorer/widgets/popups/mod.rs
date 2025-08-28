use ratatui::layout::{Constraint, Flex, Layout, Rect};

pub mod help;
pub mod new_address;

fn centered_rect(length_x: u16, length_y: u16, area: Rect) -> Rect {
    let horizontal = Layout::horizontal([Constraint::Length(length_x)]).flex(Flex::Center);
    let vertical = Layout::vertical([Constraint::Length(length_y)]).flex(Flex::Center);

    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);

    area
}
