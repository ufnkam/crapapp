use iced::{Element, Task, window};

use super::screens::{Process, Screen};
use super::settings::Settings;
use super::{action, exit, process, settings};

pub fn run() -> iced::Result {
    iced::application(Installer::default, Installer::update, Installer::view)
        .title(window_title)
        .window(window::Settings {
            icon: installer_icon(),
            ..window::Settings::default()
        })
        .run()
}

#[derive(Debug, Clone)]
pub enum Message {
    SelectProcess(Process),
    InstallPathChanged(String),
    AddToPathChanged(bool),
    Next,
    Cancel,
    Finish,
}

struct Installer {
    screen: Screen,
    process: Process,
    settings: Settings,
}

impl Default for Installer {
    fn default() -> Self {
        Self {
            screen: Screen::Action,
            process: Process::Installation,
            settings: Settings::default(),
        }
    }
}

impl Installer {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectProcess(process) => {
                self.process = process;
            }
            Message::InstallPathChanged(path) => {
                self.settings.install_path = path;
            }
            Message::AddToPathChanged(enabled) => {
                self.settings.add_to_path = enabled;
            }
            Message::Next => {
                self.next();
            }
            Message::Cancel => {
                self.screen = Screen::Exit("Installation has been canceled.".to_string());
            }
            Message::Finish => {
                std::process::exit(0);
            }
        }

        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        match &self.screen {
            Screen::Action => action::view(self.process),
            Screen::Settings => settings::view(&self.settings),
            Screen::Process(process) => process::view(*process),
            Screen::Exit(content) => exit::view(content.into()),
        }
    }

    fn next(&mut self) {
        self.screen = match self.screen {
            Screen::Action => match self.process {
                Process::Installation | Process::Reinstallation => Screen::Settings,
                Process::Uninstallation => Screen::Process(self.process),
            },
            Screen::Settings => Screen::Process(self.process),
            Screen::Process(_) => {
                Screen::Exit("Installation has been finished successfully".to_string())
            }
            Screen::Exit(_) => unreachable!(),
        };
    }
}

fn window_title(_: &Installer) -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_stem()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "Installer".to_owned())
}

fn installer_icon() -> Option<window::Icon> {
    window::icon::from_rgba(
        include_bytes!("../../assets/install.rgba").to_vec(),
        512,
        512,
    )
    .ok()
}
