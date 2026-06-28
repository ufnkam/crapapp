use std::collections::HashMap;

use crate::config::{InstallerConfig, UNINSTALLER_EXE};
use crate::install::{install as install_quiet, install_plan, prune_install_root};
use crate::uninstall::{UninstallOptions, uninstall as uninstall_quiet};

pub fn install<F>(
    config: &InstallerConfig,
    variables: &HashMap<String, String>,
    mut confirm: F,
) -> Result<(), String>
where
    F: FnMut(&str) -> Result<bool, String>,
{
    crate::install::validate_variables(config, variables)?;
    let plan = install_plan(config, variables)?;

    println!("Installing {} {}", config.app_name, config.app_version);
    println!("Install directory: {}", plan.install_root.display());

    if plan.existing.registry_exists {
        return Err(format!(
            "{} is already registered as installed. Uninstall it from Windows Apps & Features before running setup again.",
            config.app_name
        ));
    }

    if plan.existing.path_exists {
        println!("Existing installation detected.");
        println!("Directory already exists: {}", plan.install_root.display());

        if !confirm("Remove existing files and replace this installation?")? {
            println!("Installation cancelled.");
            return Ok(());
        }

        println!(
            "Removing existing files, keeping {} and install directory.",
            UNINSTALLER_EXE
        );
        prune_install_root(&plan.install_root, &plan.uninstaller_path)?;
    }

    println!("Writing payload files...");
    let report = install_quiet(config, variables, &plan)?;

    if report.path_updated {
        println!("Updated user PATH.");
    }

    println!(
        "Installed {} files. Estimated size: {} KB.",
        report.files, report.estimated_size_kb
    );
    Ok(())
}

pub fn uninstall(config: &InstallerConfig, options: UninstallOptions) -> Result<(), String> {
    println!("Uninstalling {}", config.app_name);
    let report = uninstall_quiet(config, options)?;

    if report.path_removed {
        println!("Updated user PATH.");
    }

    println!("Removed {} files.", report.files);
    println!("Uninstalled {}", config.app_name);
    Ok(())
}
