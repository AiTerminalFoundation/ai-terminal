use iced::widget::text;
use iced::{Element, Font};

use crate::ui::theme::DraculaTheme;

pub fn styled_text<'a>(content: &str, is_command: bool, command_failed: bool) -> Element<'a, crate::app::Message> {
    let style = if is_command {
        if command_failed {
            text(content)
                .font(Font::MONOSPACE)
                .size(13)  // Slightly larger for commands to make them stand out
                .style(DraculaTheme::error_command_text())
        } else {
            text(content)
                .font(Font::MONOSPACE)
                .size(13)  // Slightly larger for commands to make them stand out
                .style(DraculaTheme::command_text())
        }
    } else {
        text(content)
            .font(Font::MONOSPACE)
            .size(12)
            .style(DraculaTheme::output_text())
    };
    style.into()
} 