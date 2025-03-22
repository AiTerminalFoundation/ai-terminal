use iced::widget::{container, text};
use iced::{Element, Length};

use crate::ui::theme::DraculaTheme;

pub fn drag_handle<'a>() -> Element<'a, crate::app::Message> {
    container(text(""))
        .width(Length::Fixed(2.0))
        .height(Length::Fill)
        .style(DraculaTheme::drag_handle_style())
        .into()
} 