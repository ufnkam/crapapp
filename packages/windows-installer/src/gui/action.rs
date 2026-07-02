use iced::Element;
use iced::widget::{button, column, radio, text};

use super::app::Message;
use super::screens::Process;
use super::shared;

pub fn view(selected: Process) -> Element<'static, Message> {
    shared::screen(
        column![
            text("Choose option").size(18),
            radio(
                "Install",
                Process::Installation,
                Some(selected),
                Message::SelectProcess
            )
            .size(18)
            .spacing(10)
            .text_size(16),
            radio(
                "Reinstall",
                Process::Reinstallation,
                Some(selected),
                Message::SelectProcess,
            )
            .size(18)
            .spacing(10)
            .text_size(16),
            radio(
                "Uninstall",
                Process::Uninstallation,
                Some(selected),
                Message::SelectProcess,
            )
            .size(18)
            .spacing(10)
            .text_size(16),
        ]
        .spacing(10),
        shared::footer(vec![
            button(text("Next")).on_press(Message::Next).into(),
            button(text("Cancel")).on_press(Message::Cancel).into(),
        ]),
    )
}
