use iced::widget::{container, scrollable};
use iced::{Element, Length, Command};
use iced::widget::scrollable::{Properties, RelativeOffset};
use once_cell::sync::Lazy;

use crate::app::Message;

// Use Lazy for static initialization
static SCROLL_ID: Lazy<scrollable::Id> = Lazy::new(|| scrollable::Id::new("terminal-scroll"));

pub fn scrollable_container<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    container(
        scrollable(
            container(content)
                .width(Length::Fill)
                .height(Length::Shrink)
        )
        .height(Length::Fill)
        .id(SCROLL_ID.clone())
        .direction(scrollable::Direction::Vertical(Properties::default()))
        .on_scroll(|_| {
            Message::ScrollToBottom
        })
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

// Add this function to get the scroll command
pub fn scroll_to_bottom() -> Command<Message> {
    scrollable::snap_to(SCROLL_ID.clone(), RelativeOffset::END)
} 