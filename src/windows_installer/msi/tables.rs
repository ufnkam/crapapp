use msi::Value;

pub(super) struct InstallExecuteSequence;

impl InstallExecuteSequence {
    pub(super) fn rows() -> Vec<Vec<Value>> {
        [
            ("AppSearch", 50),
            ("CostInitialize", 800),
            ("FileCost", 900),
            ("CostFinalize", 1000),
            ("InstallValidate", 1400),
            ("InstallInitialize", 1500),
            ("ProcessComponents", 1600),
            ("UnpublishFeatures", 1800),
            ("RemoveRegistryValues", 2600),
            ("RemoveShortcuts", 3200),
            ("RemoveFiles", 3500),
            ("CreateFolders", 3700),
            ("InstallFiles", 4000),
            ("CreateShortcuts", 4500),
            ("WriteRegistryValues", 5000),
            ("WriteEnvironmentStrings", 5200),
            ("RegisterUser", 6000),
            ("RegisterProduct", 6100),
            ("PublishFeatures", 6300),
            ("PublishProduct", 6400),
            ("InstallFinalize", 6600),
        ]
        .into_iter()
        .map(|(action, sequence)| vec![Value::from(action), Value::Null, Value::Int(sequence)])
        .collect()
    }
}

pub(super) struct InstallUiSequence;

impl InstallUiSequence {
    pub(super) fn rows() -> Vec<Vec<Value>> {
        [
            ("AppSearch", None, 50),
            ("CostInitialize", None, 800),
            ("FileCost", None, 900),
            ("CostFinalize", None, 1000),
            ("WelcomeDlg", Some("NOT Installed"), 1290),
            ("ProgressDlg", Some("NOT Installed"), 1295),
            ("ExecuteAction", None, 1300),
            ("ExitDlg", Some("NOT Installed"), 6600),
        ]
        .into_iter()
        .map(|(action, condition, sequence)| {
            vec![
                Value::from(action),
                condition.map(Value::from).unwrap_or(Value::Null),
                Value::Int(sequence),
            ]
        })
        .collect()
    }
}

pub(super) struct ActionText;

impl ActionText {
    pub(super) fn rows() -> Vec<Vec<Value>> {
        [
            ("InstallFiles", "Installing application files"),
            ("CreateShortcuts", "Creating shortcuts"),
            ("WriteRegistryValues", "Writing application settings"),
            ("WriteEnvironmentStrings", "Updating environment variables"),
            ("RegisterProduct", "Registering application"),
            ("PublishProduct", "Finishing installation"),
            ("RemoveFiles", "Removing application files"),
            ("RemoveRegistryValues", "Removing application settings"),
        ]
        .into_iter()
        .map(|(action, description)| {
            vec![Value::from(action), Value::from(description), Value::Null]
        })
        .collect()
    }
}

pub(super) struct TextStyle;

impl TextStyle {
    pub(super) fn rows() -> Vec<Vec<Value>> {
        [
            ("DefaultFont", "Segoe UI", 10, Some(0), None),
            ("TitleFont", "Segoe UI", 16, Some(0), Some(1)),
            ("HeadingFont", "Segoe UI", 11, Some(0), Some(1)),
        ]
        .into_iter()
        .map(|(style, face, size, color, bits)| {
            vec![
                Value::from(style),
                Value::from(face),
                Value::Int(size),
                color.map(Value::Int).unwrap_or(Value::Null),
                bits.map(Value::Int).unwrap_or(Value::Null),
            ]
        })
        .collect()
    }
}
