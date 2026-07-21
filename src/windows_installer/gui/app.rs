use super::screens::{Process, Screen};
use super::shared::Header;
use super::{
    action_screen, eula_screen, exit_screen, process_screen, settings_screen,
    uninstall_settings_screen,
};
use crate::windows_installer::InstallerConfig;
use crate::windows_installer::gui::settings_screen::Settings;
use iced::{Element, Size, Task, window};

#[derive(Clone, Debug)]
pub struct UiConfig {
    title: String,
    config: Option<InstallerConfig>,
}

impl UiConfig {
    pub fn from_embedded(
        entrypoint: EntryPoint,
        config: &'static [u8],
        payload: &'static [u8],
        uninstaller: &'static [u8],
    ) -> Result<Self, String> {
        if config.is_empty() {
            return Ok(Self {
                title: "windows-installer".to_owned(),
                config: None,
            });
        }

        let config = InstallerConfig::new(config, payload, uninstaller)?;
        let app_name = config
            .display_name
            .as_deref()
            .map(str::trim)
            .filter(|display_name| !display_name.is_empty())
            .or_else(|| {
                let app_name = config.app_name.trim();
                (!app_name.is_empty()).then_some(app_name)
            })
            .unwrap_or("windows-installer");
        let title = match entrypoint {
            EntryPoint::Installer => format!("{app_name} installer"),
            EntryPoint::Uninstaller => format!("{app_name} uninstaller"),
        };

        Ok(Self {
            title,
            config: Some(config),
        })
    }

    pub fn installer_config(&self) -> &Option<InstallerConfig> {
        &self.config
    }
}

pub fn run(config: UiConfig) -> iced::Result {
    iced::application(
        move || Installer::new(config.clone()),
        Installer::update,
        Installer::view,
    )
    .title(window_title)
    .window(window::Settings {
        size: Size::new(760.0, 520.0),
        min_size: Some(Size::new(620.0, 420.0)),
        position: window::Position::Centered,
        icon: window_icon(),
        ..window::Settings::default()
    })
    .run()
}

#[derive(Debug, Clone)]
pub enum Message {
    SelectProcess(Process),
    InstallPathChanged(String),
    BrowseInstallPath,
    AddToPathChanged(bool),
    RemoveAssociatedFilesChanged(bool),
    EulaAcceptedChanged(usize, bool),
    ProcessEvent(process_screen::Event),
    AdaptWindow(Option<window::Id>),
    WindowMonitorSize(window::Id, Option<Size>),
    Next(Screen),
    Cancel,
    Finish,
    Previous,
}

struct Installer {
    title: String,
    screen: Screen,
    process: Process,
    settings: Settings,
    process_state: process_screen::State,
    installed: bool,
    screen_stack: Vec<Screen>,
    ui_config: UiConfig,
}

impl Installer {
    fn new(config: UiConfig) -> (Self, Task<Message>) {
        let settings = Settings::from_config(config.installer_config().as_ref());
        let installed = config
            .installer_config()
            .as_ref()
            .and_then(|installer_config| {
                let variables = process_screen::variables(installer_config, &settings);
                crate::windows_installer::install::install_plan(installer_config, &variables).ok()
            })
            .is_some_and(|plan| plan.existing.registry_exists);
        let process = if installed {
            Process::Reinstallation
        } else {
            Process::Installation
        };

        (
            Self {
                title: config.title.clone(),
                screen: Screen::Action,
                process,
                settings,
                process_state: process_screen::State::idle(process),
                installed,
                screen_stack: vec![],
                ui_config: config,
            },
            window::latest().map(Message::AdaptWindow),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectProcess(process) => {
                if process_available(process, self.installed) {
                    self.process = process;
                }
            }
            Message::InstallPathChanged(path) => {
                self.settings.install_path = path;
            }
            Message::BrowseInstallPath => {
                if self.settings.install_path_modifiable {
                    let dialog = self
                        .settings
                        .browse_directory(self.ui_config.installer_config().as_ref())
                        .map_or_else(rfd::FileDialog::new, |path| {
                            rfd::FileDialog::new().set_directory(path)
                        });

                    if let Some(path) = dialog.pick_folder() {
                        self.settings.install_path = self.settings.selected_install_path(
                            path,
                            self.ui_config.installer_config().as_ref(),
                        );
                    }
                }
            }
            Message::AddToPathChanged(enabled) => {
                self.settings.add_to_path = enabled;
            }
            Message::RemoveAssociatedFilesChanged(enabled) => {
                self.settings.remove_associated_files = enabled;
            }
            Message::EulaAcceptedChanged(index, accepted) => {
                if let Some(eula) = self.settings.accepted_eulas.get_mut(index) {
                    *eula = accepted;
                }
            }
            Message::ProcessEvent(event) => {
                self.process_state.apply(event);
            }
            Message::AdaptWindow(Some(id)) => {
                return window::monitor_size(id)
                    .map(move |size| Message::WindowMonitorSize(id, size));
            }
            Message::AdaptWindow(None) => {}
            Message::WindowMonitorSize(id, Some(size)) => {
                return window::resize::<Message>(id, adaptive_window_size(size));
            }
            Message::WindowMonitorSize(_, None) => {}
            Message::Next(current_screen) => {
                return self.next_screen(current_screen);
            }
            Message::Cancel => {
                self.screen = Screen::Exit("Installation has been canceled.".to_string());
            }
            Message::Finish => {
                std::process::exit(0);
            }
            Message::Previous => {
                self.prev_screen();
            }
        }

        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let header = Header::from_config(
            self.header_title(),
            self.ui_config.installer_config().as_ref(),
        );

        match &self.screen {
            Screen::Action => action_screen::view(self.process, self.installed, header),
            Screen::Eula(index) => {
                if let Some(config) = self.ui_config.installer_config().as_ref() {
                    eula_screen::view(&self.settings, config, *index, header)
                } else {
                    settings_screen::view(&self.settings, header)
                }
            }
            Screen::Settings => settings_screen::view(&self.settings, header),
            Screen::UninstallSettings => uninstall_settings_screen::view(&self.settings, header),
            Screen::Process(process) => process_screen::view(*process, &self.process_state, header),
            Screen::Exit(content) => exit_screen::view(content.into(), header),
        }
    }

    fn next_screen(&mut self, current_screen: Screen) -> Task<Message> {
        self.screen_stack.push(current_screen);
        self.screen = match self.screen {
            Screen::Action => match self.process {
                Process::Installation | Process::Reinstallation => {
                    if self.has_eulas() {
                        Screen::Eula(0)
                    } else {
                        Screen::Settings
                    }
                }
                Process::Uninstallation => Screen::UninstallSettings,
            },
            Screen::Eula(index) => {
                let next_index = index + 1;
                if self
                    .ui_config
                    .installer_config()
                    .as_ref()
                    .is_some_and(|config| next_index < config.eulas.len())
                {
                    Screen::Eula(next_index)
                } else {
                    Screen::Settings
                }
            }
            Screen::Settings => Screen::Process(self.process),
            Screen::UninstallSettings => Screen::Process(self.process),
            Screen::Process(_) => Screen::Exit(self.process_state.detail.clone()),
            Screen::Exit(_) => unreachable!(),
        };

        if let Screen::Process(process) = self.screen {
            self.process_state =
                crate::windows_installer::gui::process_screen::State::running(process);
            return crate::windows_installer::gui::process_screen::start(
                process,
                self.ui_config.installer_config().clone(),
                self.settings.clone(),
            );
        }

        Task::none()
    }

    fn prev_screen(&mut self) {
        self.screen = self.screen_stack.pop().unwrap();
    }

    fn has_eulas(&self) -> bool {
        self.ui_config
            .installer_config()
            .as_ref()
            .is_some_and(|config| !config.eulas.is_empty())
    }

    fn header_title(&self) -> Option<String> {
        let Some(config) = self.ui_config.installer_config().as_ref() else {
            return Some(format!("{} windows-installer", self.process.action_label()));
        };
        let display_name = display_name(config);
        let app_version = config.app_version.trim();

        Some(if app_version.is_empty() {
            format!("{} {display_name}", self.process.action_label())
        } else {
            format!(
                "{} {display_name} {app_version}",
                self.process.action_label()
            )
        })
    }
}

fn process_available(process: Process, installed: bool) -> bool {
    match (installed, process) {
        (true, Process::Reinstallation | Process::Uninstallation) => true,
        (false, Process::Installation) => true,
        _ => false,
    }
}

fn adaptive_window_size(monitor_size: Size) -> Size {
    Size::new(
        (monitor_size.width * 0.58).clamp(620.0, 760.0),
        (monitor_size.height * 0.62).clamp(420.0, 520.0),
    )
}

fn window_title(installer: &Installer) -> String {
    installer.title.clone()
}

fn display_name(config: &InstallerConfig) -> &str {
    config
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|display_name| !display_name.is_empty())
        .unwrap_or_else(|| {
            let app_name = config.app_name.trim();

            if app_name.is_empty() {
                "windows-installer"
            } else {
                app_name
            }
        })
}

fn window_icon() -> Option<window::Icon> {
    None
}

#[derive(Clone, Copy, Debug)]
pub enum EntryPoint {
    Installer,
    Uninstaller,
}
