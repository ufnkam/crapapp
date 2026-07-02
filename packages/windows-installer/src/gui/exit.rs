use iced::Element;
use iced::widget::{button, text};
use std::borrow::Cow;

use super::app::Message;
use super::shared;

pub fn view(message: Cow<'_, str>) -> Element<'static, Message> {
    shared::screen(
        text(message.to_string()),
        shared::footer(vec![button("Finish").on_press(Message::Finish).into()]),
    )
}
