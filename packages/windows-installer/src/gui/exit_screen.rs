use iced::Element;
use iced::widget::text;
use std::borrow::Cow;

use super::app::Message;
use super::shared::{self, Header};
use super::theme;

pub fn view(message: Cow<'_, str>, header: Header) -> Element<'static, Message> {
    shared::screen_with_header(
        text(message.to_string()),
        shared::footer(vec![theme::footer_button("Finish", Message::Finish)]),
        header,
    )
}
