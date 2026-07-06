use super::app::Message;
use super::shared::{self, Header};
use super::theme;
use crate::windows_installer::InstallerConfig;
use crate::windows_installer::config::ADD_TO_PATH_VARIABLE;
use crate::windows_installer::gui::screens::Screen;
use iced::widget::{checkbox, column, container, row, text, text_input};
use iced::{Alignment, Element, Fill};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default)]
pub struct Settings {
    pub install_path: String,
    pub install_path_modifiable: bool,
    pub add_to_path: bool,
    pub has_associated_files: bool,
    pub remove_associated_files: bool,
    pub accepted_eulas: Vec<bool>,
}

impl Settings {
    const DEV_APP_NAME: &'static str = "windows-installer";

    pub fn from_config(config: Option<&InstallerConfig>) -> Self {
        let Some(config) = config else {
            let mut install_path = directories::UserDirs::new()
                .map(|dirs| dirs.home_dir().to_path_buf())
                .unwrap_or_else(|| {
                    if cfg!(windows) {
                        PathBuf::from("C:/Users")
                    } else {
                        PathBuf::from("/Users")
                    }
                });
            install_path.push(Self::DEV_APP_NAME);

            return Self {
                install_path: install_path.display().to_string(),
                install_path_modifiable: true,
                add_to_path: true,
                has_associated_files: false,
                remove_associated_files: false,
                accepted_eulas: Vec::new(),
            };
        };

        let app_name = config.install_app_name();
        let publisher = config.publisher_name();
        let mut default_install_path = directories::UserDirs::new()
            .map(|dirs| dirs.home_dir().to_path_buf())
            .unwrap_or_else(|| {
                if cfg!(windows) {
                    PathBuf::from("C:/Users")
                } else {
                    PathBuf::from("/Users")
                }
            });
        if let Some(publisher) = publisher {
            default_install_path.push(publisher);
        }
        default_install_path.push(app_name);
        let default_install_path = default_install_path.display().to_string();

        let install_path_modifiable = config
            .required_variables
            .iter()
            .any(|variable| variable == "INSTALLPATH");
        let install_path = if install_path_modifiable {
            default_install_path
        } else {
            config
                .payload
                .iter()
                .find(|entry| entry.executable)
                .or_else(|| config.payload.first())
                .and_then(|entry| {
                    Path::new(&entry.destination)
                        .components()
                        .collect::<PathBuf>()
                        .parent()
                        .filter(|parent| !parent.as_os_str().is_empty())
                        .map(|parent| parent.display().to_string())
                })
                .unwrap_or(default_install_path)
        };

        Self {
            install_path,
            install_path_modifiable,
            add_to_path: Self::add_to_path_enabled_by_default(config),
            has_associated_files: !config.associated_files.is_empty(),
            remove_associated_files: false,
            accepted_eulas: vec![false; config.eulas.len()],
        }
    }

    pub fn selected_install_path(
        &self,
        selected_path: PathBuf,
        config: Option<&InstallerConfig>,
    ) -> String {
        let app_name = Self::install_app_name(config);
        let publisher = config.and_then(InstallerConfig::publisher_name);
        let install_path = if Self::path_has_install_suffix(&selected_path, app_name, publisher)
            || (publisher.is_none() && Self::path_ends_with(&selected_path, app_name))
        {
            selected_path
        } else if publisher.is_some_and(|publisher| Self::path_ends_with(&selected_path, publisher))
        {
            selected_path.join(app_name)
        } else {
            let mut install_path = selected_path;
            if let Some(publisher) = publisher {
                install_path.push(publisher);
            }
            install_path.push(app_name);
            install_path
        };

        install_path.display().to_string()
    }

    pub fn browse_directory(&self, config: Option<&InstallerConfig>) -> Option<PathBuf> {
        if self.install_path.is_empty() {
            return None;
        }

        let install_path = Path::new(&self.install_path);
        let app_name = Self::install_app_name(config);
        let publisher = config.and_then(InstallerConfig::publisher_name);
        if Self::path_has_install_suffix(install_path, app_name, publisher) {
            install_path
                .parent()
                .and_then(Path::parent)
                .map(PathBuf::from)
        } else if Self::path_ends_with(install_path, app_name) {
            install_path.parent().map(PathBuf::from)
        } else {
            Some(install_path.to_path_buf())
        }
    }

    fn install_app_name(config: Option<&InstallerConfig>) -> &str {
        config
            .map(InstallerConfig::install_app_name)
            .unwrap_or(Self::DEV_APP_NAME)
    }

    fn add_to_path_enabled_by_default(config: &InstallerConfig) -> bool {
        config
            .required_variables
            .iter()
            .any(|variable| variable == ADD_TO_PATH_VARIABLE)
            || config.payload.iter().any(|entry| entry.executable)
    }

    fn path_has_install_suffix(path: &Path, app_name: &str, publisher: Option<&str>) -> bool {
        if !Self::path_ends_with(path, app_name) {
            return false;
        }

        publisher
            .and_then(|publisher| {
                path.parent()
                    .map(|parent| Self::path_ends_with(parent, publisher))
            })
            .unwrap_or(false)
    }

    fn path_ends_with(path: &Path, segment: &str) -> bool {
        path.file_name()
            .and_then(|file_name| file_name.to_str())
            .is_some_and(|file_name| file_name == segment)
    }
}

pub fn view<'a>(settings: &'a Settings, header: Header) -> Element<'a, Message> {
    let path_input = text_input("Installation path", &settings.install_path)
        .width(Fill)
        .size(16)
        .padding([8, 10]);
    let path_input = if settings.install_path_modifiable {
        path_input.on_input(Message::InstallPathChanged)
    } else {
        path_input
    };
    let browse_button = if settings.install_path_modifiable {
        theme::button("Browse").on_press(Message::BrowseInstallPath)
    } else {
        theme::button("Browse")
    };

    shared::screen_with_header(
        column![
            text("Installation settings").size(18),
            container(
                column![
                    text("Select installation path:"),
                    row![path_input, browse_button]
                        .spacing(8)
                        .align_y(Alignment::Center)
                ]
                .spacing(8)
            )
            .padding(12)
            .width(Fill)
            .style(theme::field_group),
            container(column![
                text("Set installation settings:"),
                checkbox(settings.add_to_path)
                    .label("Set PATH")
                    .on_toggle(Message::AddToPathChanged)
                    .size(18)
                    .text_size(16),
            ])
            .width(Fill)
            .height(Fill)
            .padding(12)
            .style(theme::field_group),
        ]
        .spacing(12),
        shared::footer(vec![
            theme::footer_button("Previous", Message::Previous),
            theme::footer_button("Next", Message::Next(Screen::Settings)),
            theme::footer_button("Cancel", Message::Cancel),
        ]),
        header,
    )
}
