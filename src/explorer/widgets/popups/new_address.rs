use std::{str::FromStr, sync::Arc};

use crossterm::event::{KeyCode, KeyEvent};
use pallas::ledger::addresses::Address;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::Line,
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget},
};

use crate::{
    explorer::ExplorerContext,
    utils::{clip, Name},
};

use super::centered_rect;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum InputField {
    #[default]
    Name,
    Address,
}

#[derive(Clone)]
pub struct NewViewAddress {
    pub context: Arc<ExplorerContext>,
    pub name_input: String,
    pub address_input: String,
    pub focused: InputField,

    pub name_error: Option<String>,
    pub address_error: Option<String>,
    pub success_message: Option<String>,
}
impl NewViewAddress {
    pub fn new(context: Arc<ExplorerContext>) -> Self {
        Self {
            context,
            name_input: Default::default(),
            address_input: Default::default(),
            focused: Default::default(),

            name_error: None,
            address_error: None,
            success_message: None,
        }
    }

    pub async fn handle_key(&mut self, key: &KeyEvent) {
        match key.code {
            KeyCode::Tab | KeyCode::BackTab => {
                self.focused = match self.focused {
                    InputField::Name => InputField::Address,
                    InputField::Address => InputField::Name,
                };
            }
            KeyCode::Char(c) => match self.focused {
                InputField::Name => {
                    self.name_error = None;
                    self.name_input.push(c)
                }
                InputField::Address => {
                    self.address_error = None;
                    self.address_input.push(c)
                }
            },
            KeyCode::Backspace => match self.focused {
                InputField::Name => {
                    self.name_input.pop();
                }
                InputField::Address => {
                    self.address_input.pop();
                }
            },
            KeyCode::Enter => {
                match (
                    Name::try_from(self.name_input.as_str()),
                    Address::from_str(&self.address_input),
                ) {
                    (Ok(name), Ok(address)) => {
                        self.context.insert_wallet(address, name).await;

                        self.name_error = None;
                        self.address_error = None;
                        self.success_message = Some(format!(
                            "✔ Added address: {}...",
                            clip(&self.address_input, 20),
                        ));

                        self.name_input.clear();
                        self.address_input.clear();
                        self.focused = InputField::Name;
                    }
                    (Ok(_), Err(_)) => {
                        self.name_error = None;
                        self.address_error = Some("Invalid bech32 address".into());
                        self.success_message = None;
                        self.focused = InputField::Address;
                    }
                    (Err(_), Ok(_)) => {
                        self.name_error = Some("Type a valid name".into());
                        self.address_error = None;
                        self.success_message = None;
                        self.focused = InputField::Name;
                    }
                    (Err(_), Err(_)) => {
                        self.name_error = Some("Type a valid name".into());
                        self.address_error = Some("Invalid bech32 address".into());
                        self.success_message = None;
                    }
                }
            }
            _ => {}
        }
    }
}

impl Widget for NewViewAddress {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup_area = centered_rect(60, 16, area);

        Clear.render(popup_area, buf);

        let block = Block::default()
            .title(" Add New Address | press ESC to go back ")
            .on_black()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Color::Green)
            .bg(Color::Black);
        block.render(popup_area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([
                Constraint::Length(3), // Name input
                Constraint::Length(3), // Address input
                Constraint::Length(3), // Errors / success message
                Constraint::Length(1), // Instructions
            ])
            .split(popup_area);

        let name_border = if self.name_error.is_some() {
            Style::default().fg(Color::Red)
        } else if self.focused == InputField::Name {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        };
        let name_text = if self.focused == InputField::Name {
            format!("{}│", self.name_input)
        } else {
            self.name_input.clone()
        };
        let name = Paragraph::new(name_text).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Name")
                .border_style(name_border),
        );
        name.render(chunks[0], buf);

        let address_border = if self.address_error.is_some() {
            Style::default().fg(Color::Red)
        } else if self.focused == InputField::Address {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        };
        let address_text = if self.focused == InputField::Address {
            format!("{}│", self.address_input)
        } else {
            self.address_input.clone()
        };
        let address = Paragraph::new(address_text).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Address")
                .border_style(address_border),
        );
        address.render(chunks[1], buf);

        let mut msg_lines = Vec::new();

        if let Some(err) = &self.name_error {
            msg_lines.push(Line::from(err.as_str()));
        }
        if let Some(err) = &self.address_error {
            msg_lines.push(Line::from(err.as_str()));
        }
        if let Some(success) = &self.success_message {
            msg_lines.push(Line::from(success.as_str()));
        }

        let color = if self.success_message.is_some() {
            Color::Green
        } else {
            Color::Red
        };

        let message_paragraph = Paragraph::new(msg_lines)
            .style(Style::default().fg(color))
            .alignment(Alignment::Left);

        message_paragraph.render(chunks[2], buf);

        let instruction = Paragraph::new("Press Enter to Add")
            .style(
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::ITALIC),
            )
            .alignment(Alignment::Center);

        instruction.render(chunks[3], buf);
    }
}
