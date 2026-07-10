use serde::{Deserialize, Serialize};

pub const UNINSTALLER_EXE: &str = "uninstall.exe";
pub const ADD_TO_PATH_VARIABLE: &str = "ADD_TO_PATH";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PayloadEntry {
    #[serde(skip_serializing, default)]
    source: Option<String>,
    pub destination: String,
    pub executable: bool,
    #[serde(default)]
    offset: usize,
    #[serde(default)]
    len: usize,
    #[serde(skip, default)]
    pub bytes: &'static [u8],
}

impl PayloadEntry {
    pub fn source(&self) -> Result<&str, String> {
        self.source
            .as_deref()
            .ok_or_else(|| format!("payload source is missing for {}", self.destination))
    }

    pub fn with_range(mut self, offset: usize, len: usize) -> Self {
        self.offset = offset;
        self.len = len;
        self
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AssociatedFile {
    pub path: String,
    pub kind: AssociatedFileKind,
    #[serde(default)]
    pub eula_report: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Shortcut {
    pub target: String,
    pub name: String,
    pub directory: Option<String>,
    pub icon: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssociatedFileKind {
    File,
    Directory,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Eula {
    pub name: String,
    pub text: String,
    #[serde(default = "default_true")]
    pub required: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DisplayIcon {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InstallerConfig {
    pub app_name: String,
    pub app_version: String,
    pub display_name: Option<String>,
    pub publisher: Option<String>,
    #[serde(rename = "variables")]
    pub required_variables: Vec<String>,
    #[serde(skip_serializing, default)]
    pub uninstaller_source: String,
    #[serde(skip, default)]
    pub uninstaller_bytes: &'static [u8],
    pub payload: Vec<PayloadEntry>,
    pub display_icon: Option<String>,
    pub display_icon_rgba: Option<DisplayIcon>,
    #[serde(default)]
    pub associated_files: Vec<AssociatedFile>,
    #[serde(default)]
    pub shortcuts: Vec<Shortcut>,
    #[serde(default)]
    pub eulas: Vec<Eula>,
}

fn default_true() -> bool {
    true
}

impl Default for InstallerConfig {
    fn default() -> Self {
        Self {
            app_name: "template-app".to_owned(),
            app_version: "0.0.0".to_owned(),
            display_name: None,
            publisher: None,
            required_variables: Vec::new(),
            uninstaller_source: String::new(),
            uninstaller_bytes: &[],
            payload: Vec::new(),
            display_icon: None,
            display_icon_rgba: None,
            associated_files: Vec::new(),
            shortcuts: Vec::new(),
            eulas: Vec::new(),
        }
    }
}

impl InstallerConfig {
    pub fn install_app_name(&self) -> &str {
        let app_name = self.app_name.trim();

        if app_name.is_empty() {
            "windows-installer"
        } else {
            app_name
        }
    }

    pub fn publisher_name(&self) -> Option<&str> {
        self.publisher
            .as_deref()
            .map(str::trim)
            .filter(|publisher| !publisher.is_empty())
    }

    pub fn new(
        config: &'static [u8],
        payload: &'static [u8],
        uninstaller: &'static [u8],
    ) -> Result<Self, String> {
        let mut config = serde_json::from_slice::<Self>(config)
            .map_err(|error| format!("failed to read installer config: {error}"))?;

        for entry in &mut config.payload {
            let end = entry
                .offset
                .checked_add(entry.len)
                .ok_or_else(|| format!("payload offset overflow for {}", entry.destination))?;
            entry.bytes = payload
                .get(entry.offset..end)
                .ok_or_else(|| format!("payload bytes out of range for {}", entry.destination))?;
        }

        config.uninstaller_bytes = uninstaller;

        Ok(config)
    }
}
