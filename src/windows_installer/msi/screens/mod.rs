//! Screen definitions for the MSI user interface.
//!
//! The package writer owns MSI tables; each screen owns its own identity and
//! presentation metadata. Controls and navigation are progressively kept next
//! to their screen instead of accumulating in `builder.rs`.

mod complete;
mod install_dir;
mod license;
mod progress;
mod ready;
mod welcome;

use crate::windows_installer::{Eula, InstallerConfig};
use msi::Value;

pub(super) struct DialogSpec {
    pub id: String,
    pub first_control: String,
    pub default_control: String,
    pub cancel_control: String,
    pub title: String,
    pub modeless: bool,
}

pub(super) fn dialogs(config: &InstallerConfig, eulas: &[Eula]) -> Vec<DialogSpec> {
    let mut dialogs = vec![
        welcome::dialog(config),
        install_dir::dialog(config),
        ready::dialog(config),
        progress::dialog(config),
        complete::dialog(config),
    ];
    dialogs.extend(eulas.iter().enumerate().map(license::dialog));
    dialogs
}

/// Owns the MSI `Dialog` table rows for every installer window.
pub(super) fn dialog_rows(config: &InstallerConfig, eulas: &[Eula]) -> Vec<Vec<Value>> {
    dialogs(config, eulas)
        .into_iter()
        .map(|screen| {
            let attributes = if screen.modeless { 1 } else { 3 };
            vec![
                Value::from(screen.id),
                Value::Int(50),
                Value::Int(50),
                Value::Int(520),
                Value::Int(360),
                Value::Int(attributes),
                Value::from(screen.title),
                Value::from(screen.first_control),
                Value::from(screen.default_control),
                Value::from(screen.cancel_control),
            ]
        })
        .collect()
}

pub(super) fn display_name(config: &InstallerConfig) -> &str {
    config.display_name.as_deref().unwrap_or(&config.app_name)
}
