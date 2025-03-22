use iced::widget::{container, scrollable};
use iced::{Element, Length, Command};
use iced::widget::scrollable::{Properties, RelativeOffset};
use once_cell::sync::Lazy;

use crate::app::Message;

// Use Lazy for static initialization
static SCROLL_ID: Lazy<scrollable::Id> = Lazy::new(|| scrollable::Id::new("terminal-scroll"));

pub fn scrollable_container<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    // Create custom properties for smoother scrolling
    let properties = Properties::new()
        .width(8.0)        // Slightly wider scrollbar
        .scroller_width(8.0)
        .margin(1.0);      // Add some margin for better appearance

    container(
        scrollable(
            container(content)
                .width(Length::Fill)
                .height(Length::Shrink)
        )
        .height(Length::Fill)
        .id(SCROLL_ID.clone())
        .direction(scrollable::Direction::Vertical(properties))
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

// Add this function to get the scroll command
pub fn scroll_to_bottom() -> Command<Message> {
    scrollable::snap_to(SCROLL_ID.clone(), RelativeOffset::END)
} 