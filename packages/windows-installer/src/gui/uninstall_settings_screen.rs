use super::app::Message;
use super::settings_screen::Settings;
use super::shared::{self, Header};
use super::theme;
use crate::gui::screens::Screen;
use iced::widget::{checkbox, column, container, text};
use iced::{Element, Fill};

pub fn view<'a>(settings: &'a Settings, header: Header) -> Element<'a, Message> {
    let remove_associated_files = checkbox(settings.remove_associated_files)
        .label("Remove associated files")
        .on_toggle(Message::RemoveAssociatedFilesChanged)
        .size(18)
        .text_size(16);
    let remove_associated_files = if settings.has_associated_files {
        remove_associated_files
    } else {
        checkbox(false)
            .label("Remove associated files")
            .size(18)
            .text_size(16)
    };

    shared::screen_with_header(
        column![
            text("Uninstallation settings").size(18),
            container(column![
                text("Set uninstallation settings:"),
                remove_associated_files,
            ])
            .width(Fill)
            .height(Fill)
            .padding(12)
            .style(theme::field_group),
        ]
        .spacing(12),
        shared::footer(vec![
            theme::footer_button("Previous", Message::Previous),
            theme::footer_button("Next", Message::Next(Screen::UninstallSettings)),
            theme::footer_button("Cancel", Message::Cancel),
        ]),
        header,
    )
}
