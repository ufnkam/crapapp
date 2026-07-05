use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;

use iced::futures::channel::mpsc;
use iced::widget::{column, container, progress_bar, stack, text};
use iced::{Color, Element, Fill, Task};

use super::app::Message;
use super::screens::{Process, Screen};
use super::settings_screen::Settings;
use super::shared::{self, Header};
use super::theme;
use crate::config::{ADD_TO_PATH_VARIABLE, InstallerConfig};
use crate::install::{
    add_to_path_requested, add_user_path_entries, create_associated_files, estimated_size_kb,
    install_plan, prune_install_root, registry_entries, uninstall_entries, validate_variables,
};
use crate::registry::{remove_registry_key, write_registry_entries};
use crate::{
    remove_associated_files, remove_created_directories, remove_user_path_entries,
    resolve_install_path,
};

#[derive(Clone, Debug)]
pub struct State {
    pub progress: f32,
    pub step: Step,
    pub detail: String,
    pub running: bool,
    pub finished: bool,
    pub error: Option<String>,
}

impl State {
    pub fn idle(process: Process) -> Self {
        Self {
            progress: 0.0,
            step: Step::from_process(process),
            detail: "Waiting to start".to_owned(),
            running: false,
            finished: false,
            error: None,
        }
    }

    pub fn running(process: Process) -> Self {
        Self {
            progress: 0.0,
            step: Step::preparing(process),
            detail: "Calculating required steps".to_owned(),
            running: true,
            finished: false,
            error: None,
        }
    }

    pub fn apply(&mut self, event: Event) {
        match event {
            Event::Progress {
                progress,
                step,
                detail,
            } => {
                self.progress = progress.clamp(0.0, 100.0);
                self.step = step;
                self.detail = detail;
            }
            Event::Finished(Ok(message)) => {
                self.progress = 100.0;
                self.step = Step::Finished;
                self.detail = message;
                self.running = false;
                self.finished = true;
                self.error = None;
            }
            Event::Finished(Err(error)) => {
                self.step = Step::Failed;
                self.detail = error.clone();
                self.running = false;
                self.finished = true;
                self.error = Some(error);
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum Event {
    Progress {
        progress: f32,
        step: Step,
        detail: String,
    },
    Finished(Result<String, String>),
}

#[derive(Clone, Copy, Debug)]
pub enum Step {
    Installing,
    Uninstalling,
    Reinstalling,
    PreparingInstallation,
    PreparingUninstallation,
    PreparingReinstallation,
    CleaningExistingDirectory,
    ExtractingFiles,
    CreatingAssociatedFiles,
    UpdatingUserPath,
    SettingRegistryEntries,
    RemovingFiles,
    RemovingDirectories,
    RemovingAssociatedFiles,
    RemovingRegistryEntries,
    Finished,
    Failed,
}

impl Step {
    fn from_process(process: Process) -> Self {
        match process {
            Process::Installation => Self::Installing,
            Process::Uninstallation => Self::Uninstalling,
            Process::Reinstallation => Self::Reinstalling,
        }
    }

    fn preparing(process: Process) -> Self {
        match process {
            Process::Installation => Self::PreparingInstallation,
            Process::Uninstallation => Self::PreparingUninstallation,
            Process::Reinstallation => Self::PreparingReinstallation,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Installing => "Installing",
            Self::Uninstalling => "Uninstalling",
            Self::Reinstalling => "Reinstalling",
            Self::PreparingInstallation => "Preparing installation",
            Self::PreparingUninstallation => "Preparing uninstallation",
            Self::PreparingReinstallation => "Preparing reinstallation",
            Self::CleaningExistingDirectory => "Cleaning existing directory",
            Self::ExtractingFiles => "Extracting files",
            Self::CreatingAssociatedFiles => "Creating associated files",
            Self::UpdatingUserPath => "Updating user PATH",
            Self::SettingRegistryEntries => "Setting registry entries",
            Self::RemovingFiles => "Removing files",
            Self::RemovingDirectories => "Removing directories",
            Self::RemovingAssociatedFiles => "Removing associated files",
            Self::RemovingRegistryEntries => "Removing registry entries",
            Self::Finished => "Finished",
            Self::Failed => "Failed",
        }
    }
}

pub fn start(
    process: Process,
    config: Option<InstallerConfig>,
    settings: Settings,
) -> Task<Message> {
    let (mut sender, receiver) = mpsc::channel(100);

    thread::spawn(move || {
        let result = run(process, config, settings, |event| {
            let _ = sender.try_send(event);
        });
        let _ = sender.try_send(Event::Finished(result));
    });

    Task::run(receiver, Message::ProcessEvent)
}

pub fn view<'a>(process: Process, state: &'a State, header: Header) -> Element<'a, Message> {
    let footer = if state.finished && state.progress >= 100.0 && state.error.is_none() {
        shared::footer(vec![theme::footer_button(
            "Next",
            Message::Next(Screen::Process(process)),
        )])
    } else {
        shared::footer(vec![theme::footer_button("Cancel", Message::Cancel)])
    };

    shared::screen_with_header(
        column![
            stack([
                container(progress_bar(0.0..=100.0, state.progress).style(theme::progress_bar),)
                    .width(Fill)
                    .into(),
                container(text(format!("{:.0}%", state.progress)).color(Color::WHITE))
                    .center(Fill)
                    .into(),
            ]),
            text(state.step.as_str()).size(18),
            text(&state.detail),
        ]
        .spacing(12),
        footer,
        header,
    )
}

fn run(
    process: Process,
    config: Option<InstallerConfig>,
    settings: Settings,
    mut emit: impl FnMut(Event),
) -> Result<String, String> {
    let Some(config) = config else {
        emit.progress(100.0, Step::Finished, "No installer config was provided.");
        return Ok("No installer config was provided.".to_owned());
    };
    let variables = variables(&config, &settings);

    match process {
        Process::Installation => install(&config, &variables, ProgressRange::new(0.0, 100.0), emit),
        Process::Uninstallation => {
            uninstall(
                &config,
                None,
                settings.remove_associated_files,
                ProgressRange::new(0.0, 100.0),
                emit,
            )?;
            Ok(format!("Uninstalled {}.", display_name(&config)))
        }
        Process::Reinstallation => {
            let plan = install_plan(&config, &variables)?;
            uninstall(
                &config,
                Some(plan.install_root),
                false,
                ProgressRange::new(0.0, 45.0),
                &mut emit,
            )?;
            install(&config, &variables, ProgressRange::new(45.0, 100.0), emit)
        }
    }
}

fn install(
    config: &InstallerConfig,
    variables: &HashMap<String, String>,
    range: ProgressRange,
    mut emit: impl FnMut(Event),
) -> Result<String, String> {
    emit.progress(
        range.at(0.0),
        Step::PreparingInstallation,
        "Calculating required steps",
    );

    validate_variables(config, variables)?;
    let plan = install_plan(config, variables)?;
    emit.progress(
        range.at(0.02),
        Step::PreparingInstallation,
        format!("Install directory: {}", plan.install_root.display()),
    );

    if plan.existing.path_exists {
        emit.progress(
            range.at(0.04),
            Step::CleaningExistingDirectory,
            plan.install_root.display().to_string(),
        );
        prune_install_root(&plan.install_root, &plan.uninstaller_path)?;
    }

    let payload_bytes = config
        .payload
        .iter()
        .map(|entry| entry.bytes.len() as f32)
        .sum::<f32>()
        .max(1.0);
    let mut extracted_bytes = 0.0;
    let mut installed_paths = Vec::new();

    for (entry, path) in config.payload.iter().zip(plan.payload_paths.iter()) {
        let file_name = Path::new(&entry.destination)
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .unwrap_or(&entry.destination);
        let detail = format!("Extracting {} file to {}", file_name, path.display());

        emit.progress(
            range.at(0.06 + 0.78 * (extracted_bytes / payload_bytes)),
            Step::ExtractingFiles,
            detail,
        );
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }

        fs::write(path, entry.bytes)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;

        extracted_bytes += entry.bytes.len() as f32;
        installed_paths.push(path.clone());
        emit.progress(
            range.at(0.06 + 0.78 * (extracted_bytes / payload_bytes)),
            Step::ExtractingFiles,
            format!("Extracted {} file to {}", file_name, path.display()),
        );
    }

    emit.progress(
        range.at(0.86),
        Step::ExtractingFiles,
        format!(
            "Extracting {} file to {}",
            "uninstall.exe",
            plan.uninstaller_path.display()
        ),
    );
    fs::create_dir_all(&plan.install_root)
        .map_err(|error| format!("failed to create {}: {error}", plan.install_root.display()))?;
    fs::write(&plan.uninstaller_path, config.uninstaller_bytes).map_err(|error| {
        format!(
            "failed to write {}: {error}",
            plan.uninstaller_path.display()
        )
    })?;

    let estimated_size_kb = estimated_size_kb(&installed_paths, &plan.uninstaller_path)?;
    let path_updated = add_to_path_requested(variables)?;

    if path_updated {
        emit.progress(
            range.at(0.92),
            Step::UpdatingUserPath,
            "Setting PATH entries",
        );
        add_user_path_entries(config, variables, &plan.install_root)?;
    }

    if !config.associated_files.is_empty() {
        emit.progress(
            range.at(0.94),
            Step::CreatingAssociatedFiles,
            "Creating associated files and directories",
        );
        create_associated_files(config, variables, &plan.install_root)?;
    }

    emit.progress(
        range.at(0.96),
        Step::SettingRegistryEntries,
        "Writing application uninstall metadata",
    );
    write_registry_entries(registry_entries(
        config,
        variables,
        &plan.install_root,
        &plan.uninstaller_path,
        estimated_size_kb,
    ))?;

    let message = format!(
        "Installed {} to {}.",
        display_name(config),
        plan.install_root.display()
    );
    emit.progress(range.end, Step::Finished, message.clone());

    Ok(message)
}

fn uninstall(
    config: &InstallerConfig,
    install_root: Option<PathBuf>,
    remove_associated: bool,
    range: ProgressRange,
    mut emit: impl FnMut(Event),
) -> Result<(), String> {
    emit.progress(
        range.at(0.0),
        Step::PreparingUninstallation,
        "Calculating required steps",
    );
    let current_exe =
        env::current_exe().map_err(|error| format!("failed to find uninstaller path: {error}"))?;
    let install_root = install_root
        .or_else(|| current_exe.parent().map(PathBuf::from))
        .ok_or_else(|| "failed to find uninstaller directory".to_owned())?;
    let remove_paths = uninstall_entries(config)
        .into_iter()
        .rev()
        .map(|entry| resolve_install_path(Cow::from(entry), &install_root))
        .collect::<Vec<_>>();
    let total_bytes = remove_paths
        .iter()
        .filter_map(|path| fs::metadata(path).ok())
        .map(|metadata| metadata.len() as f32)
        .sum::<f32>()
        .max(1.0);
    let mut removed_bytes = 0.0;

    emit.progress(
        range.at(0.02),
        Step::PreparingUninstallation,
        format!("Install directory: {}", install_root.display()),
    );

    for path in remove_paths {
        if path == current_exe {
            continue;
        }

        let bytes = fs::metadata(&path)
            .map(|metadata| metadata.len() as f32)
            .unwrap_or(0.0);
        if path.exists() {
            emit.progress(
                range.at(0.05 + 0.70 * (removed_bytes / total_bytes)),
                Step::RemovingFiles,
                format!("Removing {}", path.display()),
            );
            fs::remove_file(&path)
                .map_err(|error| format!("failed to remove {}: {error}", path.display()))?;
        }
        removed_bytes += bytes;
    }

    if remove_associated {
        emit.progress(
            range.at(0.78),
            Step::RemovingAssociatedFiles,
            "Removing associated files and directories",
        );
        remove_associated_files(config, &install_root)?;
    }

    emit.progress(
        range.at(0.82),
        Step::RemovingDirectories,
        "Removing empty created directories",
    );
    remove_created_directories(config, &install_root);

    emit.progress(
        range.at(0.86),
        Step::UpdatingUserPath,
        "Removing PATH entries",
    );
    remove_user_path_entries(config, &install_root);

    emit.progress(
        range.at(0.94),
        Step::RemovingRegistryEntries,
        "Removing application uninstall metadata",
    );
    remove_registry_key(config);

    emit.progress(
        range.end,
        Step::Finished,
        format!("Uninstalled {}.", display_name(config)),
    );

    Ok(())
}

fn variables(config: &InstallerConfig, settings: &Settings) -> HashMap<String, String> {
    let mut variables = HashMap::new();

    if config
        .required_variables
        .iter()
        .any(|variable| variable == "INSTALLPATH")
    {
        variables.insert("INSTALLPATH".to_owned(), settings.install_path.clone());
    }

    variables.insert(
        ADD_TO_PATH_VARIABLE.to_owned(),
        if settings.add_to_path { "1" } else { "0" }.to_owned(),
    );

    variables
}

fn display_name(config: &InstallerConfig) -> &str {
    config
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|display_name| !display_name.is_empty())
        .unwrap_or(&config.app_name)
}

#[derive(Clone, Copy)]
struct ProgressRange {
    start: f32,
    end: f32,
}

impl ProgressRange {
    fn new(start: f32, end: f32) -> Self {
        Self { start, end }
    }

    fn at(self, fraction: f32) -> f32 {
        self.start + (self.end - self.start) * fraction.clamp(0.0, 1.0)
    }
}

trait EmitProgress {
    fn progress(&mut self, progress: f32, step: Step, detail: impl Into<String>);
}

impl<T> EmitProgress for T
where
    T: FnMut(Event),
{
    fn progress(&mut self, progress: f32, step: Step, detail: impl Into<String>) {
        self(Event::Progress {
            progress,
            step,
            detail: detail.into(),
        });
    }
}
