use iced::widget::{text};
use iced::{Element};
use crate::ui::theme::DraculaTheme;
use crate::ui::messages::Message;

pub fn git_branch_text(branch: &str) -> Element<Message> {
    text(format!("git({})", branch))
        .size(14)
        .style(DraculaTheme::YELLOW)
        .into()
} 