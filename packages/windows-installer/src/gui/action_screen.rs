use iced::widget::{column, radio, row, text};
use iced::{Color, Element};

use super::app::Message;
use super::screens::{Process, Screen};
use super::shared::{self, Header};
use super::theme;

pub fn view(selected: Process, installed: bool, header: Header) -> Element<'static, Message> {
    shared::screen_with_header(
        column![
            text("Choose option installation").size(18),
            option("Install", Process::Installation, selected, !installed),
            option("Reinstall", Process::Reinstallation, selected, installed),
            option("Uninstall", Process::Uninstallation, selected, installed),
        ]
        .spacing(10),
        shared::footer(vec![
            theme::footer_button("Next", Message::Next(Screen::Action)),
            theme::footer_button("Cancel", Message::Cancel),
        ]),
        header,
    )
}

fn option(
    label: &'static str,
    process: Process,
    selected: Process,
    enabled: bool,
) -> Element<'static, Message> {
    if enabled {
        radio(label, process, Some(selected), Message::SelectProcess)
            .size(18)
            .spacing(10)
            .text_size(16)
            .into()
    } else {
        let disabled = Color::from_rgb8(150, 150, 150);

        row![
            text("○").size(18).color(disabled),
            text(label).color(disabled)
        ]
        .spacing(10)
        .into()
    }
}
