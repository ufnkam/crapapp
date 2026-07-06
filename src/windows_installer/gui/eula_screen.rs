use super::app::Message;
use super::settings_screen::Settings;
use super::shared::{self, Header};
use super::theme;
use crate::windows_installer::InstallerConfig;
use crate::windows_installer::gui::screens::Screen;
use iced::widget::{checkbox, column, container, scrollable, text};
use iced::{Element, Fill};

pub fn view<'a>(
    settings: &'a Settings,
    config: &'a InstallerConfig,
    index: usize,
    header: Header,
) -> Element<'a, Message> {
    let Some(eula) = config.eulas.get(index) else {
        return shared::screen_with_header(
            text("License agreement is missing."),
            shared::footer(vec![
                theme::footer_button("Previous", Message::Previous),
                theme::footer_button("Cancel", Message::Cancel),
            ]),
            header,
        );
    };
    let accepted = settings.accepted_eulas.get(index).copied().unwrap_or(false);
    let next_button = if !eula.required || accepted {
        theme::footer_button("Next", Message::Next(Screen::Eula(index)))
    } else {
        theme::button("Next").into()
    };
    let position = format!("License agreement {} of {}", index + 1, config.eulas.len());

    shared::screen_with_header(
        column![
            text(position).size(18),
            container(
                column![
                    text(&eula.name).size(16),
                    scrollable(text(&eula.text).size(14))
                        .height(Fill)
                        .width(Fill),
                    checkbox(accepted)
                        .label(format!("I accept {}", eula.name))
                        .on_toggle(move |enabled| Message::EulaAcceptedChanged(index, enabled))
                        .size(18)
                        .text_size(16),
                ]
                .spacing(12),
            )
            .padding(12)
            .width(Fill)
            .height(Fill)
            .style(theme::field_group),
        ]
        .spacing(12),
        shared::footer(vec![
            theme::footer_button("Previous", Message::Previous),
            next_button,
            theme::footer_button("Cancel", Message::Cancel),
        ]),
        header,
    )
}
