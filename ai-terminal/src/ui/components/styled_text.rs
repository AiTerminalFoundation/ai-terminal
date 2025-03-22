use iced::widget::{text, row};
use iced::{Element, Font, Color};

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

// Create styled text for git branch info
pub fn git_branch_text<'a>(branch_name: &str) -> Element<'a, crate::app::Message> {
    row![
        text("git")
            .font(Font::MONOSPACE)
            .size(12)
            .style(DraculaTheme::YELLOW),
        text("(")
            .font(Font::MONOSPACE)
            .size(12)
            .style(DraculaTheme::output_text()),
        text(branch_name)
            .font(Font::MONOSPACE)
            .size(12)
            .style(DraculaTheme::GREEN),
        text(")")
            .font(Font::MONOSPACE)
            .size(12)
            .style(DraculaTheme::output_text())
    ]
    .spacing(0)
    .into()
} 