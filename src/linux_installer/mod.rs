#![allow(dead_code)]

use std::path::{Component, Path, PathBuf};

use anyhow::bail;

pub mod aur;
pub mod deb;
pub mod desktop_entry;
pub mod linux_aur_bundler;
pub mod linux_deb_bundler;
pub mod linux_rpm_bundler;
pub mod rpm;

pub fn install_relative_path(path: &str) -> anyhow::Result<String> {
    if path.trim().is_empty() {
        bail!("Linux install path must not be empty");
    }

    let mut relative = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(part) => relative.push(part),
            Component::ParentDir | Component::Prefix(_) => {
                bail!("Linux install path {path} must not contain parent or prefix components");
            }
        }
    }

    if relative.as_os_str().is_empty() {
        bail!("Linux install path {path} must not resolve to package root");
    }

    Ok(relative.display().to_string())
}
