use iced::widget::{button, container, text};
use iced::{Element, Length};

use crate::ui::messages::Message;
use crate::ui::theme::DraculaTheme;

pub fn copy_button<'a>(content: String) -> Element<'a, Message> {
    button(
        container(text("Copy").size(10))
            .padding(4)
            .width(Length::Shrink)
    )
    .on_press(Message::CopyToClipboard(content, true))
    .style(DraculaTheme::copy_button_style())
    .into()
}

