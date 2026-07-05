use iced::widget::{column, container, image, row, rule, text};
use iced::{Alignment, Element, Fill, Length};

use super::app::Message;
use super::assets::{INSTALL_ICON_RGBA, INSTALL_ICON_SIZE};
use crate::InstallerConfig;
use crate::config::DisplayIcon;

#[derive(Clone, Debug)]
pub struct Header {
    title: Option<String>,
    icon: Option<DisplayIcon>,
}

impl Header {
    pub fn from_config(title: Option<String>, config: Option<&InstallerConfig>) -> Self {
        Self {
            title,
            icon: config.and_then(|config| config.display_icon_rgba.clone()),
        }
    }
}

pub fn screen_with_header<'a>(
    content: impl Into<Element<'a, Message>>,
    controls: impl Into<Element<'a, Message>>,
    header: Header,
) -> Element<'a, Message> {
    container(
        column![
            header_view(header),
            rule::horizontal(1),
            container(content)
                .height(Length::Fill)
                .width(Length::Fill)
                .padding([12, 16]),
            rule::horizontal(1),
            container(controls)
                .height(56)
                .width(Length::Fill)
                .padding([10, 16]),
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

pub fn header_view(header: Header) -> Element<'static, Message> {
    let icon = header.icon.map_or_else(
        || image::Handle::from_rgba(INSTALL_ICON_SIZE, INSTALL_ICON_SIZE, INSTALL_ICON_RGBA),
        |icon| image::Handle::from_rgba(icon.width, icon.height, icon.rgba),
    );
    let mut header_row = row![image(icon).width(64).height(64)];

    if let Some(title) = header.title {
        header_row = header_row.push(text(title).size(20));
    }

    header_row
        .spacing(12)
        .height(80)
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
