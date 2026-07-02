#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Screen {
    Action,
    Settings,
    Process(Process),
    Exit(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Process {
    Installation,
    Uninstallation,
    Reinstallation,
}

impl Process {
    pub fn progress_label(self) -> &'static str {
        match self {
            Self::Installation => "Installing",
            Self::Uninstallation => "Uninstalling",
            Self::Reinstallation => "Reinstalling",
        }
    }
}
