pub const UNINSTALLER_EXE: &str = "uninstall.exe";
pub const ADD_TO_PATH_VARIABLE: &str = "ADD_TO_PATH";

#[derive(Clone, Copy, Debug)]
pub struct PayloadEntry {
    pub destination: &'static str,
    pub executable: bool,
    pub bytes: &'static [u8],
}

#[derive(Clone, Copy, Debug)]
pub struct InstallerConfig {
    pub app_name: &'static str,
    pub app_version: &'static str,
    pub publisher: Option<&'static str>,
    pub app_display_icon: Option<&'static str>,
    pub required_variables: &'static [&'static str],
    pub path_entries: &'static [&'static str],
    pub uninstall_entries: &'static [&'static str],
    pub uninstaller_bytes: &'static [u8],
    pub payload: &'static [PayloadEntry],
}
