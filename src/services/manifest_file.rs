use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

use crate::services::icons::IconMapping;

pub const MANIFEST_PATH: &str = "CRAP.toml";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrapManifest {
    pub build: Option<BuildConfig>,
    pub windows: Option<WindowsPlatform>,
    pub macos: Option<MacosPlatform>,
    pub linux: Option<LinuxPlatform>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildConfig {
    pub publisher: Option<String>,
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub features: Vec<String>,
}

impl CrapManifest {
    pub fn platforms(&self) -> Vec<PlatformConfig<'_>> {
        let mut platforms = Vec::new();

        if let Some(windows) = &self.windows {
            platforms.push(PlatformConfig::Windows(windows));
        }

        if let Some(macos) = &self.macos {
            platforms.push(PlatformConfig::Macos(macos));
        }

        if let Some(linux) = &self.linux {
            platforms.push(PlatformConfig::Linux(linux));
        }

        platforms
    }
}

pub enum PlatformConfig<'a> {
    Windows(&'a WindowsPlatform),
    Macos(&'a MacosPlatform),
    Linux(&'a LinuxPlatform),
}

pub trait PlatformManifest {
    fn name(&self) -> &'static str;
    fn bin_dir(&self) -> &str;
    fn install_path(&self) -> Option<&str>;
    fn variable_sources(&self) -> Vec<&str>;
    fn files(&self) -> &[FileMapping];
    fn icons(&self) -> &[IconMapping];
    fn targets(&self) -> Vec<&'static str>;
}

impl PlatformManifest for PlatformConfig<'_> {
    fn name(&self) -> &'static str {
        match self {
            PlatformConfig::Windows(platform) => platform.name(),
            PlatformConfig::Macos(platform) => platform.name(),
            PlatformConfig::Linux(platform) => platform.name(),
        }
    }

    fn bin_dir(&self) -> &str {
        match self {
            PlatformConfig::Windows(platform) => platform.bin_dir(),
            PlatformConfig::Macos(platform) => platform.bin_dir(),
            PlatformConfig::Linux(platform) => platform.bin_dir(),
        }
    }

    fn install_path(&self) -> Option<&str> {
        match self {
            PlatformConfig::Windows(platform) => platform.install_path(),
            PlatformConfig::Macos(platform) => platform.install_path(),
            PlatformConfig::Linux(platform) => platform.install_path(),
        }
    }

    fn variable_sources(&self) -> Vec<&str> {
        match self {
            PlatformConfig::Windows(platform) => platform.variable_sources(),
            PlatformConfig::Macos(platform) => platform.variable_sources(),
            PlatformConfig::Linux(platform) => platform.variable_sources(),
        }
    }

    fn files(&self) -> &[FileMapping] {
        match self {
            PlatformConfig::Windows(platform) => platform.files(),
            PlatformConfig::Macos(platform) => platform.files(),
            PlatformConfig::Linux(platform) => platform.files(),
        }
    }

    fn icons(&self) -> &[IconMapping] {
        match self {
            PlatformConfig::Windows(platform) => platform.icons(),
            PlatformConfig::Macos(platform) => platform.icons(),
            PlatformConfig::Linux(platform) => platform.icons(),
        }
    }

    fn targets(&self) -> Vec<&'static str> {
        match self {
            PlatformConfig::Windows(platform) => platform.targets(),
            PlatformConfig::Macos(platform) => platform.targets(),
            PlatformConfig::Linux(platform) => platform.targets(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowsPlatform {
    #[serde(default)]
    pub targets: Vec<WindowsTarget>,
    pub install_path: Option<String>,
    pub bin_dir: Option<String>,
    #[serde(default)]
    pub files: Vec<FileMapping>,
    #[serde(default)]
    pub icons: Vec<IconMapping>,
}

impl PlatformManifest for WindowsPlatform {
    fn name(&self) -> &'static str {
        "windows"
    }

    fn bin_dir(&self) -> &str {
        self.bin_dir.as_deref().unwrap_or("")
    }

    fn install_path(&self) -> Option<&str> {
        self.install_path.as_deref()
    }

    fn variable_sources(&self) -> Vec<&str> {
        self.install_path.iter().map(String::as_str).collect()
    }

    fn files(&self) -> &[FileMapping] {
        &self.files
    }

    fn icons(&self) -> &[IconMapping] {
        &self.icons
    }

    fn targets(&self) -> Vec<&'static str> {
        self.targets.iter().map(WindowsTarget::target).collect()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacosPlatform {
    #[serde(default)]
    pub targets: Vec<MacosTarget>,
    #[serde(default)]
    pub files: Vec<FileMapping>,
}

impl PlatformManifest for MacosPlatform {
    fn name(&self) -> &'static str {
        "macos"
    }

    fn bin_dir(&self) -> &str {
        "bin"
    }

    fn install_path(&self) -> Option<&str> {
        None
    }

    fn variable_sources(&self) -> Vec<&str> {
        Vec::new()
    }

    fn files(&self) -> &[FileMapping] {
        &self.files
    }

    fn icons(&self) -> &[IconMapping] {
        &[]
    }

    fn targets(&self) -> Vec<&'static str> {
        self.targets.iter().map(MacosTarget::target).collect()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinuxPlatform {
    #[serde(default)]
    pub targets: Vec<LinuxTarget>,
    #[serde(default)]
    pub files: Vec<FileMapping>,
}

impl PlatformManifest for LinuxPlatform {
    fn name(&self) -> &'static str {
        "linux"
    }

    fn bin_dir(&self) -> &str {
        "bin"
    }

    fn install_path(&self) -> Option<&str> {
        None
    }

    fn variable_sources(&self) -> Vec<&str> {
        Vec::new()
    }

    fn files(&self) -> &[FileMapping] {
        &self.files
    }

    fn icons(&self) -> &[IconMapping] {
        &[]
    }

    fn targets(&self) -> Vec<&'static str> {
        self.targets.iter().map(LinuxTarget::target).collect()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileMapping {
    pub source: String,
    pub destination: String,
}

#[derive(Debug, Clone, Deserialize)]
pub enum WindowsTarget {
    #[serde(rename = "x86_64-pc-windows-gnu")]
    X86_64PcWindowsGnu,
    #[serde(rename = "x86_64-pc-windows-msvc")]
    X86_64PcWindowsMsvc,
    #[serde(rename = "aarch64-pc-windows-gnullvm")]
    Aarch64PcWindowsGnullvm,
    #[serde(rename = "aarch64-pc-windows-msvc")]
    Aarch64PcWindowsMsvc,
}

impl WindowsTarget {
    pub fn target(&self) -> &'static str {
        match self {
            WindowsTarget::X86_64PcWindowsGnu => "x86_64-pc-windows-gnu",
            WindowsTarget::X86_64PcWindowsMsvc => "x86_64-pc-windows-msvc",
            WindowsTarget::Aarch64PcWindowsGnullvm => "aarch64-pc-windows-gnullvm",
            WindowsTarget::Aarch64PcWindowsMsvc => "aarch64-pc-windows-msvc",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub enum MacosTarget {
    #[serde(rename = "x86_64-apple-darwin")]
    X86_64AppleDarwin,
    #[serde(rename = "aarch64-apple-darwin")]
    Aarch64AppleDarwin,
}

impl MacosTarget {
    pub fn target(&self) -> &'static str {
        match self {
            MacosTarget::X86_64AppleDarwin => "x86_64-apple-darwin",
            MacosTarget::Aarch64AppleDarwin => "aarch64-apple-darwin",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub enum LinuxTarget {
    #[serde(rename = "x86_64-unknown-linux-gnu")]
    X86_64UnknownLinuxGnu,
    #[serde(rename = "x86_64-unknown-linux-musl")]
    X86_64UnknownLinuxMusl,
}

impl LinuxTarget {
    pub fn target(&self) -> &'static str {
        match self {
            LinuxTarget::X86_64UnknownLinuxGnu => "x86_64-unknown-linux-gnu",
            LinuxTarget::X86_64UnknownLinuxMusl => "x86_64-unknown-linux-musl",
        }
    }
}

impl CrapManifest {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read manifest at {}", path.display()))?;

        toml::from_str(&contents)
            .with_context(|| format!("failed to parse manifest at {}", path.display()))
    }
}
