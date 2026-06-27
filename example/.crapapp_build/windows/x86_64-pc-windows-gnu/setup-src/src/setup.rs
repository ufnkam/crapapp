use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use winreg::RegKey;
use winreg::enums::HKEY_CURRENT_USER;

struct PayloadEntry {
    destination: &'static str,
    #[allow(dead_code)]
    executable: bool,
    bytes: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/payload.rs"));

const APP_NAME: &str = "example";
const APP_VERSION: &str = "0.1.0";
const REQUIRED_VARIABLES: &[&str] = &["INSTALLPATH"];
const UNINSTALLER_BYTES: &[u8] = include_bytes!("/Users/kamilufnal/workspace/cargo-crapapp/example/.crapapp_build/windows/x86_64-pc-windows-gnu/setup-src/target/x86_64-pc-windows-gnu/release/uninstall.exe");
const PATH_ENTRIES: &[&str] = &["$INSTALLPATH"];
const UNINSTALLER_EXE: &str = "uninstall.exe";

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let variables = parse_args()?;
    validate_variables(&variables)?;
    install(&variables)
}

fn install(variables: &HashMap<String, String>) -> Result<(), String> {
    let mut installed_paths = Vec::new();

    for entry in PAYLOAD {
        let destination = resolve_variables(entry.destination, variables);
        let path = PathBuf::from(destination);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }

        fs::write(&path, entry.bytes)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;

        #[cfg(unix)]
        if entry.executable {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(&path)
                .map_err(|error| format!("failed to read {} metadata: {error}", path.display()))?
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions)
                .map_err(|error| format!("failed to chmod {}: {error}", path.display()))?;
        }

        installed_paths.push(path);
    }

    let install_root = install_root(variables, &installed_paths)?;
    fs::create_dir_all(&install_root)
        .map_err(|error| format!("failed to create {}: {error}", install_root.display()))?;

    let uninstaller_path = install_root.join(UNINSTALLER_EXE);
    copy_uninstaller(&uninstaller_path)?;

    add_user_path_entries(variables)?;
    register_uninstaller(&install_root, &uninstaller_path)?;

    println!("installed {} files", PAYLOAD.len());
    Ok(())
}

fn parse_args() -> Result<HashMap<String, String>, String> {
    let mut values = HashMap::new();
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--args" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--args requires KEY=value".to_owned())?;
                let (key, value) = value
                    .split_once('=')
                    .ok_or_else(|| format!("invalid --args value {value}, expected KEY=value"))?;

                values.insert(key.to_owned(), value.to_owned());
            }
            "--help" | "-h" => {
                print_help();
                process::exit(0);
            }
            _ => return Err(format!("unknown argument {arg}")),
        }
    }

    Ok(values)
}

fn validate_variables(values: &HashMap<String, String>) -> Result<(), String> {
    for required in REQUIRED_VARIABLES {
        if !values.contains_key(*required) {
            return Err(format!(
                "missing variable {required}. Pass it as --args {required}=<value>"
            ));
        }
    }

    for key in values.keys() {
        if !REQUIRED_VARIABLES.contains(&key.as_str()) {
            return Err(format!("unknown variable {key}"));
        }
    }

    Ok(())
}

fn resolve_variables(value: &str, variables: &HashMap<String, String>) -> String {
    let mut resolved = value.to_owned();

    for (key, replacement) in variables {
        resolved = resolved.replace(&format!("${key}"), replacement);
    }

    resolved
}

fn install_root(
    variables: &HashMap<String, String>,
    installed_paths: &[PathBuf],
) -> Result<PathBuf, String> {
    if let Some(install_path) = variables.get("INSTALLPATH") {
        return Ok(PathBuf::from(install_path));
    }

    if let Some(parent) = installed_paths.first().and_then(|path| path.parent()) {
        return Ok(parent.to_path_buf());
    }

    env::current_dir().map_err(|error| format!("failed to find current directory: {error}"))
}

fn copy_uninstaller(destination: &Path) -> Result<(), String> {
    fs::write(destination, UNINSTALLER_BYTES)
        .map_err(|error| format!("failed to write {}: {error}", destination.display()))
}

fn register_uninstaller(install_root: &PathBuf, uninstaller_path: &PathBuf) -> Result<(), String> {
    let install_location = install_root.display().to_string();
    let uninstall_string = format!("\"{}\"", uninstaller_path.display());

    write_registry_values(&install_location, &uninstall_string)
}

fn write_registry_values(install_location: &str, uninstall_string: &str) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(registry_key())
        .map_err(|error| format!("failed to create user uninstall registry key: {error}"))?;

    key.set_value("DisplayName", &APP_NAME)
        .map_err(|error| format!("failed to write DisplayName: {error}"))?;
    key.set_value("DisplayVersion", &APP_VERSION)
        .map_err(|error| format!("failed to write DisplayVersion: {error}"))?;
    key.set_value("InstallLocation", &install_location)
        .map_err(|error| format!("failed to write InstallLocation: {error}"))?;
    key.set_value("UninstallString", &uninstall_string)
        .map_err(|error| format!("failed to write UninstallString: {error}"))?;
    key.set_value("QuietUninstallString", &uninstall_string)
        .map_err(|error| format!("failed to write QuietUninstallString: {error}"))?;
    key.set_value("Publisher", &APP_NAME)
        .map_err(|error| format!("failed to write Publisher: {error}"))?;

    Ok(())
}

fn add_user_path_entries(variables: &HashMap<String, String>) -> Result<(), String> {
    let entries = PATH_ENTRIES
        .iter()
        .map(|entry| resolve_variables(entry, variables))
        .filter(|entry| !entry.is_empty())
        .collect::<Vec<_>>();

    if entries.is_empty() {
        return Ok(());
    }

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (environment, _) = hkcu
        .create_subkey("Environment")
        .map_err(|error| format!("failed to open user environment registry key: {error}"))?;
    let current_path = environment.get_value::<String, _>("Path").unwrap_or_default();
    let mut path_parts = current_path
        .split(';')
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    for entry in entries {
        if !path_parts
            .iter()
            .any(|part| part.eq_ignore_ascii_case(&entry))
        {
            path_parts.push(entry);
        }
    }

    environment
        .set_value("Path", &path_parts.join(";"))
        .map_err(|error| format!("failed to update user Path: {error}"))
}

fn registry_key() -> String {
    format!(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{APP_NAME}")
}

fn print_help() {
    eprintln!("Usage: setup.exe --args KEY=value [--args KEY=value ...]");

    if !REQUIRED_VARIABLES.is_empty() {
        eprintln!("Required variables:");

        for variable in REQUIRED_VARIABLES {
            eprintln!("  {variable}");
        }
    }
}
