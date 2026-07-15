use crate::bundlers::WindowsInstallerKind;
use crate::platform_manifests::WindowsPlatformManifest;
use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer, Serialize};
use std::path::Path;

pub const MANIFEST_PATH: &str = "CRAP.toml";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrapManifest {
    pub build: Option<BuildConfig>,
    pub windows: Option<WindowsPlatformManifest>,
    pub macos: Option<MacosPlatform>,
    pub linux: Option<LinuxPlatform>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildConfig {
    pub publisher: Option<String>,
    pub display_name: Option<String>,
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
    Windows(&'a WindowsPlatformManifest),
    Macos(&'a MacosPlatform),
    Linux(&'a LinuxPlatform),
}

pub trait PlatformManifest {
    fn name(&self) -> &'static str;
    fn bin_dir(&self) -> &str;
    fn install_path(&self) -> Option<&str>;
    fn variable_sources(&self) -> Vec<&str>;
    fn files(&self) -> &[FileMapping];
    fn display_icon(&self) -> Option<&str>;
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

    fn display_icon(&self) -> Option<&str> {
        match self {
            PlatformConfig::Windows(platform) => platform.display_icon(),
            PlatformConfig::Macos(platform) => platform.display_icon(),
            PlatformConfig::Linux(platform) => platform.display_icon(),
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

impl PlatformManifest for WindowsPlatformManifest {
    fn name(&self) -> &'static str {
        "windows"
    }

    fn bin_dir(&self) -> &str {
        self.bin_dir()
    }

    fn install_path(&self) -> Option<&str> {
        self.install_path()
    }

    fn variable_sources(&self) -> Vec<&str> {
        self.variable_sources()
    }

    fn files(&self) -> &[FileMapping] {
        &self.files
    }

    fn display_icon(&self) -> Option<&str> {
        self.display_icon()
    }

    fn targets(&self) -> Vec<&'static str> {
        self.targets.iter().map(WindowsTarget::target).collect()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssociatedFile {
    pub path: String,
    pub kind: AssociatedFileKind,
    #[serde(default)]
    pub eula_report: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssociatedFileKind {
    File,
    Directory,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ShortcutMapping {
    pub binary: String,
    pub name: String,
    pub directory: Option<String>,
    pub icon: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum EulaFile {
    Path(String),
    Options {
        path: String,
        #[serde(default = "default_true")]
        required: bool,
    },
}

impl EulaFile {
    pub fn path(&self) -> &str {
        match self {
            Self::Path(path) => path,
            Self::Options { path, .. } => path,
        }
    }

    pub fn required(&self) -> bool {
        match self {
            Self::Path(_) => true,
            Self::Options { required, .. } => *required,
        }
    }
}

fn default_true() -> bool {
    true
}

pub(crate) fn deserialize_windows_installers<'de, D>(
    deserializer: D,
) -> Result<Vec<WindowsInstallerKind>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum WindowsInstallerList {
        One(WindowsInstallerKind),
        Many(Vec<WindowsInstallerKind>),
    }

    Ok(
        match Option::<WindowsInstallerList>::deserialize(deserializer)? {
            Some(WindowsInstallerList::One(installer)) => vec![installer],
            Some(WindowsInstallerList::Many(installers)) => installers,
            None => Vec::new(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::CrapManifest;
    use crate::bundlers::WindowsInstallerKind;

    fn parse_manifest(source: &str) -> CrapManifest {
        toml::from_str(source).expect("manifest should parse")
    }

    #[test]
    fn windows_installer_defaults_to_cli() {
        let manifest = parse_manifest(
            r#"
            [windows]
            targets = ["x86_64-pc-windows-gnu"]
            "#,
        );

        let windows = manifest.windows.expect("windows platform should exist");
        assert_eq!(windows.installers(), vec![WindowsInstallerKind::Cli]);
    }

    #[test]
    fn windows_installer_accepts_single_value() {
        let manifest = parse_manifest(
            r#"
            [windows]
            targets = ["x86_64-pc-windows-gnu"]
            installer = "gui"
            "#,
        );

        let windows = manifest.windows.expect("windows platform should exist");
        assert_eq!(windows.installers(), vec![WindowsInstallerKind::Gui]);
    }

    #[test]
    fn windows_installer_accepts_list() {
        let manifest = parse_manifest(
            r#"
            [windows]
            targets = ["x86_64-pc-windows-gnu"]
            installer = ["cli", "gui"]
            "#,
        );

        let windows = manifest.windows.expect("windows platform should exist");
        assert_eq!(
            windows.installers(),
            vec![WindowsInstallerKind::Cli, WindowsInstallerKind::Gui]
        );
    }

    #[test]
    fn windows_installer_deduplicates_list_without_reordering() {
        let manifest = parse_manifest(
            r#"
            [windows]
            targets = ["x86_64-pc-windows-gnu"]
            installer = ["gui", "cli", "gui"]
            "#,
        );

        let windows = manifest.windows.expect("windows platform should exist");
        assert_eq!(
            windows.installers(),
            vec![WindowsInstallerKind::Gui, WindowsInstallerKind::Cli]
        );
    }

    #[test]
    fn eula_required_false_parses() {
        let manifest = parse_manifest(
            r#"
            [windows]
            targets = ["x86_64-pc-windows-gnu"]
            eulas = [
                { path = "EULA.txt", required = false },
            ]
            "#,
        );

        let windows = manifest.windows.expect("windows platform should exist");
        assert!(!windows.eulas[0].required());
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

    fn display_icon(&self) -> Option<&str> {
        None
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

    fn display_icon(&self) -> Option<&str> {
        None
    }

    fn targets(&self) -> Vec<&'static str> {
        self.targets.iter().map(LinuxTarget::target).collect()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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
