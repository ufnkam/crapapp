use iced::Element;
use iced::widget::{button, checkbox, column, row, text, text_input};

use super::app::Message;
use super::shared;

#[derive(Debug, Default)]
pub struct Settings {
    pub install_path: String,
    pub add_to_path: bool,
}

pub fn view(settings: &Settings) -> Element<'_, Message> {
    shared::screen(
        column![
            text_input("Where to install", &settings.install_path)
                .on_input(Message::InstallPathChanged),
            text("Settings"),
            row![
                checkbox(settings.add_to_path).on_toggle(Message::AddToPathChanged),
                text("Set PATH"),
            ]
            .spacing(8),
        ]
        .spacing(12),
        shared::footer(vec![
            button(text("Previous")).on_press(Message::Cancel).into(),
            button(text("Next")).on_press(Message::Next).into(),
            button(text("Cancel")).on_press(Message::Cancel).into(),
        ]),
    )
}
