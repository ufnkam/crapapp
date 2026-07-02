use iced::widget::{column, container, image, row, rule};
use iced::{Alignment, Element, Fill, Length};

use super::app::Message;

pub fn screen<'a>(
    content: impl Into<Element<'a, Message>>,
    controls: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(
        column![
            header(),
            rule::horizontal(1),
            container(content)
                .height(Length::Fill)
                .width(Length::Fill)
                .padding([12, 16]),
            rule::horizontal(1),
            container(controls)
                .height(44)
                .width(Length::Fill)
                .padding([8, 16]),
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

pub fn header<'a>() -> Element<'a, Message> {
    row![
        image(image::Handle::from_rgba(
            256,
            256,
            include_bytes!("../../assets/install.rgba").as_slice()
        ))
        .width(36)
        .height(36)
    ]
    .height(52)
    .align_y(Alignment::Center)
    .padding([0, 16])
    .into()
}

pub fn footer(elements: Vec<Element<Message>>) -> Element<Message> {
    let mut buttons_row = row![].spacing(8).align_y(Alignment::Center);
    for elem in elements {
        buttons_row = buttons_row.push(elem);
    }

    container(buttons_row)
        .width(Fill)
        .align_x(iced::Right)
        .center_y(Fill)
        .into()
}
