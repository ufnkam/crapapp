use crate::config::InstallerConfig;
use crate::install::{
    add_user_path_entries, estimated_size_kb, install_plan, prune_install_root, registry_entries,
    uninstall_entries,
};
use crate::registry::{remove_registry_key, write_registry_entries};
use crate::{add_to_path_requested, remove_created_directories, resolve_install_path};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::{env, fs, io};
use std::borrow::Cow;

pub fn install(
    config: &InstallerConfig,
    variables: &HashMap<String, String>,
) -> Result<(), String> {
    crate::install::validate_variables(config, variables)?;
    let plan = install_plan(config, variables)?;

    println!("Installing {} {}", config.app_name, config.app_version);
    println!("Install directory: {}", plan.install_root.display());

    if plan.existing.registry_exists {
        if !confirm("Existing installation detected. Continue the reinstallation?")? {
            println!("Aborting.");
            return Ok(());
        }

        uninstall(config)?;
    }

    if plan.existing.path_exists {
        println!("Directory already exists: {}", plan.install_root.display());
        println!("Cleaning existing directory",);
        prune_install_root(&plan.install_root, &plan.uninstaller_path)?;
    }

    let mut installed_paths = Vec::new();
    for (entry, path) in config.payload.iter().zip(plan.payload_paths.iter()) {
        let file_name = Path::new(&entry.destination)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        println!(
            "Extracting {} file to {}",
            file_name,
            path.to_str().unwrap()
        );
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }

        fs::write(path, entry.bytes)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;

        #[cfg(unix)]
        if entry.executable {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(path)
                .map_err(|error| format!("failed to read {} metadata: {error}", path.display()))?
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions)
                .map_err(|error| format!("failed to chmod {}: {error}", path.display()))?;
        }

        installed_paths.push(path.clone());
    }

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
        add_user_path_entries(config, variables, &plan.install_root)?;
        println!("Updated user PATH.");
    }

    write_registry_entries(registry_entries(
        config,
        variables,
        &plan.install_root,
        &plan.uninstaller_path,
        estimated_size_kb,
    ))?;
    println!("Installed registry keys");

    println!(
        "Installed {} files. Total size: {} KB.",
        installed_paths.len(),
        estimated_size_kb
    );
    Ok(())
}

pub fn uninstall(config: &InstallerConfig) -> Result<(), String> {
    println!("Uninstalling {}", config.app_name);
    let current_exe =
        env::current_exe().map_err(|error| format!("failed to find uninstaller path: {error}"))?;
    let install_root = current_exe
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| "failed to find uninstaller directory".to_owned())?;

    for path in uninstall_entries(config)
        .into_iter()
        .rev()
        .map(|entry| resolve_install_path(Cow::from(entry), &install_root))
    {
        if path == current_exe {
            continue;
        }

        if path.exists() {
            println!("Removing {}", path.display());
            fs::remove_file(&path)
                .map_err(|error| format!("failed to remove {}: {error}", path.display()))?;
        }
    }

    remove_created_directories(config, &install_root);
    println!("Removing registry keys");
    remove_registry_key(config);

    println!("Updated user PATH.");
    println!("Uninstalled {}", config.app_name);
    Ok(())
}

fn confirm(question: &str) -> Result<bool, String> {
    print!("{question} [y/N] ");
    io::stdout()
        .flush()
        .map_err(|error| format!("failed to flush stdout: {error}"))?;

    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|error| format!("failed to read answer: {error}"))?;

    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES" | "Yes"))
}
