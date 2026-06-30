use serde::Deserialize;

pub const UNINSTALLER_EXE: &str = "uninstall.exe";
pub const ADD_TO_PATH_VARIABLE: &str = "ADD_TO_PATH";

#[derive(Clone, Debug)]
pub struct PayloadEntry {
    pub destination: String,
    pub executable: bool,
    pub bytes: &'static [u8],
}

#[derive(Clone, Debug)]
pub struct IconEntry {
    pub binary: String,
}

#[derive(Clone, Debug)]
pub struct InstallerConfig {
    pub app_name: String,
    pub app_version: String,
    pub publisher: Option<String>,
    pub required_variables: Vec<String>,
    pub uninstaller_bytes: &'static [u8],
    pub payload: Vec<PayloadEntry>,
    pub icons: Vec<IconEntry>,
}

#[derive(Debug, Deserialize)]
struct EmbeddedConfig {
    app_name: String,
    app_version: String,
    publisher: Option<String>,
    variables: Vec<String>,
    payload: Vec<EmbeddedPayloadEntry>,
    icons: Vec<IconEntryConfig>,
}

#[derive(Debug, Deserialize)]
struct EmbeddedPayloadEntry {
    destination: String,
    executable: bool,
    offset: usize,
    len: usize,
}

#[derive(Debug, Deserialize)]
struct IconEntryConfig {
    binary: String,
}

pub fn installer_config(
    config: &'static [u8],
    payload: &'static [u8],
    uninstaller: &'static [u8],
) -> Result<InstallerConfig, String> {
    let config = serde_json::from_slice::<EmbeddedConfig>(config)
        .map_err(|error| format!("failed to read installer config: {error}"))?;

    let payload = config
        .payload
        .into_iter()
        .map(|entry| {
            let end = entry
                .offset
                .checked_add(entry.len)
                .ok_or_else(|| format!("payload offset overflow for {}", entry.destination))?;
            let bytes = payload
                .get(entry.offset..end)
                .ok_or_else(|| format!("payload bytes out of range for {}", entry.destination))?;

            Ok(PayloadEntry {
                destination: entry.destination,
                executable: entry.executable,
                bytes,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(InstallerConfig {
        app_name: config.app_name,
        app_version: config.app_version,
        publisher: config.publisher,
        required_variables: config.variables,
        uninstaller_bytes: uninstaller,
        payload,
        icons: config
            .icons
            .into_iter()
            .map(|icon| IconEntry {
                binary: icon.binary,
            })
            .collect(),
    })
}
