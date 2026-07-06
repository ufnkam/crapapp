use iced::widget::{
    Button, button as iced_button, container, progress_bar as iced_progress_bar, text,
};
use iced::{Background, Border, Color, Element, Shadow, Theme, border};

use super::app::Message;

fn windows_blue() -> Color {
    Color::from_rgb8(0x00, 0x78, 0xD4)
}

fn windows_blue_hover() -> Color {
    Color::from_rgb8(0x10, 0x6E, 0xB9)
}

fn windows_blue_pressed() -> Color {
    Color::from_rgb8(0x00, 0x5A, 0x9E)
}

pub fn button<'a>(label: impl text::IntoFragment<'a>) -> Button<'a, Message> {
    iced_button(text(label))
        .padding([8, 14])
        .style(rounded_button)
}

pub fn rounded_button(_theme: &Theme, status: iced_button::Status) -> iced_button::Style {
    let base = iced_button::Style {
        background: Some(Background::Color(windows_blue())),
        text_color: Color::WHITE,
        border: Border {
            radius: border::Radius::from(8.0),
            width: 1.0,
            color: windows_blue_pressed(),
        },
        shadow: Shadow::default(),
        snap: true,
    };

    match status {
        iced_button::Status::Active => base,
        iced_button::Status::Hovered => iced_button::Style {
            background: Some(Background::Color(windows_blue_hover())),
            border: Border {
                color: windows_blue_hover(),
                ..base.border
            },
            ..base
        },
        iced_button::Status::Pressed => iced_button::Style {
            background: Some(Background::Color(windows_blue_pressed())),
            border: Border {
                color: windows_blue_pressed(),
                ..base.border
            },
            ..base
        },
        iced_button::Status::Disabled => iced_button::Style {
            background: base
                .background
                .map(|background| background.scale_alpha(0.5)),
            text_color: base.text_color.scale_alpha(0.5),
            border: Border {
                color: Color {
                    a: 0.5,
                    ..base.border.color
                },
                ..base.border
            },
            ..base
        },
    }
}

pub fn progress_bar(theme: &Theme) -> iced_progress_bar::Style {
    let palette = theme.extended_palette();

    iced_progress_bar::Style {
        background: Background::Color(palette.background.strong.color),
        bar: Background::Color(windows_blue()),
        border: border::rounded(2),
    }
}

pub fn footer_button<'a>(
    label: impl text::IntoFragment<'a>,
    message: Message,
) -> Element<'a, Message> {
    button(label).on_press(message).into()
}

pub fn field_group(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(Background::Color(palette.background.weakest.color)),
        text_color: Some(palette.background.weakest.text),
        border: Border {
            radius: border::Radius::from(8.0),
            width: 1.0,
            color: palette.background.weak.color,
        },
        shadow: Shadow::default(),
        snap: true,
    }
}
