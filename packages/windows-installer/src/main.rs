#![cfg_attr(all(windows, feature = "gui"), windows_subsystem = "windows")]

fn main() -> Result<(), String> {
    windows_installer::installer::run(&[], &[], &[])
}
