use iced::Element;
use iced::widget::{button, column, progress_bar, text};

use super::app::Message;
use super::screens::Process;
use super::shared;

pub fn view(process: Process) -> Element<'static, Message> {
    shared::screen(
        column![
            progress_bar(0.0..=100.0, 45.0),
            text(format!("{} application", process.progress_label())),
        ]
        .spacing(12),
        shared::footer(vec![button(text("Next")).on_press(Message::Next).into()]),
    )
}
