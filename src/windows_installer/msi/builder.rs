use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow, bail};
use msi::{CodePage, Column, Insert, Language, Package, PackageType, Value};
use uuid::Uuid;

const DEFAULT_MANUFACTURER: &str = "unknown";
const COMPONENT_64BIT: i32 = 256;
const COMPONENT_REGISTRY_KEYPATH: i32 = 4;
const HKCU: i32 = 1;
const ADD_TO_PATH_PROPERTY: &str = "ADD_TO_PATH";
const DIALOG_WIDTH: i32 = 520;
#[cfg(test)]
const DIALOG_HEIGHT: i32 = 360;
const FOOTER_LINE_Y: i32 = 316;
const FOOTER_BUTTON_Y: i32 = 328;
const BUTTON_WIDTH: i32 = 70;
const BUTTON_HEIGHT: i32 = 22;
const BACK_BUTTON_X: i32 = 270;
const CANCEL_BUTTON_X: i32 = 348;
const NEXT_BUTTON_X: i32 = 426;

use crate::windows_installer::{AssociatedFileKind, Eula, InstallerConfig};

fn display_name(config: &InstallerConfig) -> &str {
    config.display_name.as_deref().unwrap_or(&config.app_name)
}

fn manufacturer(config: &InstallerConfig) -> &str {
    config.publisher_name().unwrap_or(DEFAULT_MANUFACTURER)
}

pub fn build(
    plan: &InstallerConfig,
    output: &Path,
    folder_picker_action: &[u8],
    display_icon_source: Option<&str>,
) -> anyhow::Result<()> {
    validate(plan)?;
    let display_name = display_name(plan);
    let manufacturer = manufacturer(plan);

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    if output.exists() {
        fs::remove_file(output)
            .with_context(|| format!("failed to remove {}", output.display()))?;
    }

    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(output)
        .with_context(|| format!("failed to create {}", output.display()))?;
    let mut package = Package::create(PackageType::Installer, file)
        .with_context(|| format!("failed to create MSI database {}", output.display()))?;
    package.set_database_codepage(CodePage::Windows1252);
    package
        .summary_info_mut()
        .set_codepage(CodePage::Windows1252);
    package.summary_info_mut().set_subject(display_name);
    package.summary_info_mut().set_author(manufacturer);
    package
        .summary_info_mut()
        .set_comments(format!("{display_name} installer package"));
    package
        .summary_info_mut()
        .set_keywords(&["Installer", "MSI", "Database"].map(ToString::to_string));
    package
        .summary_info_mut()
        .set_arch(msi_platform(&plan.bundle_target));
    package
        .summary_info_mut()
        .set_languages(&[Language::from_code(1033)]);
    package
        .summary_info_mut()
        .set_page_count(msi_page_count(&plan.bundle_target));
    package.summary_info_mut().set_word_count(10);
    package.summary_info_mut().set_doc_security(0);
    package
        .summary_info_mut()
        .set_creating_application("cargo-crapapp");
    package.summary_info_mut().set_uuid(package_code(plan));

    let mut directory_ids = DirectoryIds::default();
    let files = msi_files(plan, &mut directory_ids)?;
    collect_associated_directories(plan, &mut directory_ids)?;
    let shortcut_rows = shortcuts(plan, &files)?;
    let icons = icons(plan, &files, &shortcut_rows, display_icon_source)?;
    let eulas = plan.eulas.clone();
    let path_updates = path_updates(&files);
    let cabinet_stream = cabinet_stream_name(&plan.app_name);
    create_schema(&mut package)?;
    insert_rows(
        &mut package,
        plan,
        &cabinet_stream,
        &files,
        &directory_ids,
        &shortcut_rows,
        &icons,
        &eulas,
        &path_updates,
        folder_picker_action,
    )?;

    let cab = super::cabinet::build(&files)?;
    package
        .write_stream(&cabinet_stream)
        .context("failed to create MSI cabinet stream")?
        .write_all(&cab)
        .context("failed to write MSI cabinet stream")?;

    let file = package
        .into_inner()
        .context("failed to flush MSI database")?;
    drop(file);

    super::finalizer::finalize(output).context("failed to finalize MSI database table order")?;

    Ok(())
}

fn validate(plan: &InstallerConfig) -> anyhow::Result<()> {
    if plan.payload.is_empty() {
        bail!("msi package has no files to package");
    }

    Ok(())
}

fn cabinet_stream_name(package: &str) -> String {
    let mut name = package
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_owned();

    if name.is_empty() {
        name.push_str("app");
    }
    name.push_str(".cab");
    name
}

#[derive(Clone, Debug)]
pub(super) struct PackageFile {
    id: String,
    component: String,
    pub(super) source: PathBuf,
    pub(super) cabinet_name: String,
    directory: String,
    directory_root: String,
    install_path: PathBuf,
    file_name: String,
    size: i32,
}

#[derive(Clone, Debug)]
struct ShortcutComponent {
    id: String,
    directory: String,
    directory_name: String,
    name: String,
    component: String,
    registry: String,
    target_file: String,
    icon: Option<String>,
}

type IconStream = (String, PathBuf);

fn msi_files(
    plan: &InstallerConfig,
    directories: &mut DirectoryIds,
) -> anyhow::Result<Vec<PackageFile>> {
    let mut output = Vec::new();

    for (index, file) in plan.payload.iter().enumerate() {
        let relative = install_relative_path(&file.destination)?;
        let file_name = relative
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "MSI destination {} must have a UTF-8 file name",
                    file.destination
                )
            })?;
        let file_name = msi_filename(file_name);
        let parent = relative.parent().unwrap_or_else(|| Path::new(""));
        let directory = directories.id_for("INSTALLFOLDER", parent)?;
        let source = PathBuf::from(file.source().map_err(anyhow::Error::msg)?);
        let size = fs::metadata(&source)
            .with_context(|| format!("failed to read metadata for {}", source.display()))?
            .len();

        if size > i32::MAX as u64 {
            bail!("MSI payload file {} is too large", source.display());
        }

        output.push(PackageFile {
            id: format!("File{}", index + 1),
            component: format!("Component{}", index + 1),
            source,
            cabinet_name: format!("File{}", index + 1),
            directory,
            directory_root: "INSTALLFOLDER".to_owned(),
            install_path: relative,
            file_name,
            size: size as i32,
        });
    }

    Ok(output)
}

fn collect_associated_directories(
    plan: &InstallerConfig,
    directories: &mut DirectoryIds,
) -> anyhow::Result<()> {
    for file in plan
        .associated_files
        .iter()
        .filter(|file| matches!(file.kind, AssociatedFileKind::Directory))
    {
        let associated = associated_path(&file.path)?;
        directories.id_for(&associated.root, &associated.relative)?;
    }

    Ok(())
}

fn install_relative_path(destination: &str) -> anyhow::Result<PathBuf> {
    let destination = destination
        .replace("$INSTALLPATH", "")
        .replace("%INSTALLPATH%", "");
    if destination.contains('$') || destination.contains('%') {
        bail!("MSI payload destination {destination} contains unsupported variables");
    }

    let mut relative = PathBuf::new();
    for component in Path::new(&destination).components() {
        match component {
            std::path::Component::RootDir | std::path::Component::CurDir => {}
            std::path::Component::Normal(part) => relative.push(part),
            std::path::Component::ParentDir | std::path::Component::Prefix(_) => {
                bail!(
                    "MSI payload destination {destination} must not contain parent or prefix components"
                );
            }
        }
    }

    if relative.as_os_str().is_empty() {
        bail!("MSI payload destination {destination} must not resolve to install root");
    }

    Ok(relative)
}

struct AssociatedPath {
    root: String,
    relative: PathBuf,
}

fn associated_path(path: &str) -> anyhow::Result<AssociatedPath> {
    let (root, value) = if let Some(path) = path.strip_prefix("$INSTALLPATH") {
        ("INSTALLFOLDER", path)
    } else if let Some(path) = path.strip_prefix("$HOMEPATH/Documents") {
        ("PersonalFolder", path)
    } else if let Some(path) = path.strip_prefix("$HOMEPATH") {
        ("UserProfileFolder", path)
    } else {
        bail!("MSI associated file path {path} must start with $INSTALLPATH or $HOMEPATH");
    };

    Ok(AssociatedPath {
        root: root.to_owned(),
        relative: install_relative_path(value)?,
    })
}

#[derive(Default)]
struct DirectoryIds {
    next: usize,
    ids: BTreeMap<(String, PathBuf), String>,
}

impl DirectoryIds {
    fn id_for(&mut self, root: &str, path: &Path) -> anyhow::Result<String> {
        if path.as_os_str().is_empty() {
            return Ok(root.to_owned());
        }

        let key = (root.to_owned(), path.to_path_buf());
        if let Some(id) = self.ids.get(&key) {
            return Ok(id.clone());
        }

        if let Some(parent) = path.parent() {
            self.id_for(root, parent)?;
        }

        self.next += 1;
        let id = format!("Dir{}", self.next);
        self.ids.insert(key, id.clone());
        Ok(id)
    }

    fn id_for_existing(&self, root: &str, path: &Path) -> anyhow::Result<String> {
        if path.as_os_str().is_empty() {
            return Ok(root.to_owned());
        }

        self.ids
            .get(&(root.to_owned(), path.to_path_buf()))
            .cloned()
            .ok_or_else(|| {
                anyhow!(
                    "MSI directory {} under {root} was not declared",
                    path.display()
                )
            })
    }
}

fn associated_directory_component(index: usize) -> String {
    format!("AssociatedDirectory{}", index + 1)
}

fn associated_directory_registry(index: usize) -> String {
    format!("RegistryAssociatedDirectory{}", index + 1)
}

fn path_update_id(index: usize) -> String {
    format!("PathEntry{}", index + 1)
}

fn path_update_component(index: usize) -> String {
    format!("PathEntry{}", index + 1)
}

fn path_update_registry(index: usize) -> String {
    format!("PathRegistry{}", index + 1)
}

fn create_schema<W: Read + Write + Seek>(package: &mut Package<W>) -> anyhow::Result<()> {
    create_table(
        package,
        "Property",
        vec![
            Column::build("Property").primary_key().id_string(72),
            Column::build("Value").nullable().formatted_string(0),
        ],
    )?;
    create_table(
        package,
        "Directory",
        vec![
            Column::build("Directory").primary_key().id_string(72),
            Column::build("Directory_Parent").nullable().id_string(72),
            Column::build("DefaultDir").string(255),
        ],
    )?;
    create_table(
        package,
        "AppSearch",
        vec![
            Column::build("Property").primary_key().id_string(72),
            Column::build("Signature_").primary_key().id_string(72),
        ],
    )?;
    create_table(
        package,
        "RegLocator",
        vec![
            Column::build("Signature_").primary_key().id_string(72),
            Column::build("Root").int16(),
            Column::build("Key").formatted_string(255),
            Column::build("Name").nullable().formatted_string(255),
            Column::build("Type").nullable().int16(),
        ],
    )?;
    create_table(
        package,
        "Signature",
        vec![
            Column::build("Signature").primary_key().id_string(72),
            Column::build("FileName").text_string(255),
            Column::build("MinVersion").nullable().text_string(20),
            Column::build("MaxVersion").nullable().text_string(20),
            Column::build("MinSize").nullable().int32(),
            Column::build("MaxSize").nullable().int32(),
            Column::build("MinDate").nullable().int32(),
            Column::build("MaxDate").nullable().int32(),
            Column::build("Languages").nullable().text_string(255),
        ],
    )?;
    create_table(
        package,
        "Feature",
        vec![
            Column::build("Feature").primary_key().id_string(38),
            Column::build("Feature_Parent").nullable().id_string(38),
            Column::build("Title").nullable().text_string(64),
            Column::build("Description").nullable().text_string(255),
            Column::build("Display").nullable().int16(),
            Column::build("Level").int16(),
            Column::build("Directory_").nullable().id_string(72),
            Column::build("Attributes").int16(),
        ],
    )?;
    create_table(
        package,
        "Component",
        vec![
            Column::build("Component").primary_key().id_string(72),
            Column::build("ComponentId").nullable().string(38),
            Column::build("Directory_").id_string(72),
            Column::build("Attributes").int16(),
            Column::build("Condition").nullable().formatted_string(255),
            Column::build("KeyPath").nullable().id_string(72),
        ],
    )?;
    create_table(
        package,
        "FeatureComponents",
        vec![
            Column::build("Feature_").primary_key().id_string(38),
            Column::build("Component_").primary_key().id_string(72),
        ],
    )?;
    create_table(
        package,
        "File",
        vec![
            Column::build("File").primary_key().id_string(72),
            Column::build("Component_").id_string(72),
            Column::build("FileName").string(255),
            Column::build("FileSize").int32(),
            Column::build("Version").nullable().string(72),
            Column::build("Language").nullable().string(20),
            Column::build("Attributes").nullable().int16(),
            Column::build("Sequence").int16(),
        ],
    )?;
    create_table(
        package,
        "Media",
        vec![
            Column::build("DiskId").primary_key().int16(),
            Column::build("LastSequence").int16(),
            Column::build("DiskPrompt").nullable().string(64),
            Column::build("Cabinet").nullable().string(255),
            Column::build("VolumeLabel").nullable().string(32),
            Column::build("Source").nullable().string(72),
        ],
    )?;
    create_table(
        package,
        "CreateFolder",
        vec![
            Column::build("Directory_").primary_key().id_string(72),
            Column::build("Component_").primary_key().id_string(72),
        ],
    )?;
    create_table(
        package,
        "Shortcut",
        vec![
            Column::build("Shortcut").primary_key().id_string(72),
            Column::build("Directory_").id_string(72),
            Column::build("Name").string(128),
            Column::build("Component_").id_string(72),
            Column::build("Target").nullable().formatted_string(255),
            Column::build("Arguments").nullable().formatted_string(255),
            Column::build("Description").nullable().text_string(255),
            Column::build("Hotkey").nullable().int16(),
            Column::build("Icon_").nullable().id_string(72),
            Column::build("IconIndex").nullable().int16(),
            Column::build("ShowCmd").nullable().int16(),
            Column::build("WkDir").nullable().id_string(72),
        ],
    )?;
    create_table(
        package,
        "Icon",
        vec![
            Column::build("Name").primary_key().id_string(72),
            Column::build("Data").binary(),
        ],
    )?;
    create_table(
        package,
        "Registry",
        vec![
            Column::build("Registry").primary_key().id_string(72),
            Column::build("Root").int16(),
            Column::build("Key").formatted_string(255),
            Column::build("Name").nullable().formatted_string(255),
            Column::build("Value").nullable().formatted_string(0),
            Column::build("Component_").id_string(72),
        ],
    )?;
    create_table(
        package,
        "RemoveRegistry",
        vec![
            Column::build("RemoveRegistry").primary_key().id_string(72),
            Column::build("Root").int16(),
            Column::build("Key").formatted_string(255),
            Column::build("Name").nullable().formatted_string(255),
            Column::build("Component_").id_string(72),
        ],
    )?;
    create_table(
        package,
        "Environment",
        vec![
            Column::build("Environment").primary_key().id_string(72),
            Column::build("Name").formatted_string(255),
            Column::build("Value").nullable().formatted_string(255),
            Column::build("Component_").id_string(72),
        ],
    )?;
    create_table(
        package,
        "InstallExecuteSequence",
        vec![
            Column::build("Action").primary_key().id_string(72),
            Column::build("Condition").nullable().formatted_string(255),
            Column::build("Sequence").nullable().int16(),
        ],
    )?;
    create_table(
        package,
        "InstallUISequence",
        vec![
            Column::build("Action").primary_key().id_string(72),
            Column::build("Condition").nullable().formatted_string(255),
            Column::build("Sequence").nullable().int16(),
        ],
    )?;
    create_table(
        package,
        "TextStyle",
        vec![
            Column::build("TextStyle").primary_key().id_string(72),
            Column::build("FaceName").string(32),
            Column::build("Size").int16(),
            Column::build("Color").nullable().int32(),
            Column::build("StyleBits").nullable().int16(),
        ],
    )?;
    create_table(
        package,
        "Dialog",
        vec![
            Column::build("Dialog").primary_key().id_string(72),
            Column::build("HCentering").int16(),
            Column::build("VCentering").int16(),
            Column::build("Width").int16(),
            Column::build("Height").int16(),
            Column::build("Attributes").int32(),
            Column::build("Title").nullable().formatted_string(128),
            Column::build("Control_First").nullable().id_string(50),
            Column::build("Control_Default").nullable().id_string(50),
            Column::build("Control_Cancel").nullable().id_string(50),
        ],
    )?;
    create_table(
        package,
        "Control",
        vec![
            Column::build("Dialog_").primary_key().id_string(72),
            Column::build("Control").primary_key().id_string(50),
            Column::build("Type").id_string(20),
            Column::build("X").int16(),
            Column::build("Y").int16(),
            Column::build("Width").int16(),
            Column::build("Height").int16(),
            Column::build("Attributes").nullable().int32(),
            Column::build("Property").nullable().id_string(50),
            Column::build("Text").nullable().formatted_string(0),
            Column::build("Control_Next").nullable().id_string(50),
            Column::build("Help").nullable().formatted_string(50),
        ],
    )?;
    create_table(
        package,
        "CheckBox",
        vec![
            Column::build("Property").primary_key().id_string(72),
            Column::build("Value").formatted_string(64),
        ],
    )?;
    create_table(
        package,
        "CustomAction",
        vec![
            Column::build("Action").primary_key().id_string(72),
            Column::build("Type").int16(),
            Column::build("Source").nullable().id_string(72),
            Column::build("Target").nullable().formatted_string(0),
        ],
    )?;
    create_table(
        package,
        "Binary",
        vec![
            Column::build("Name").primary_key().id_string(72),
            Column::build("Data").binary(),
        ],
    )?;
    create_table(
        package,
        "EventMapping",
        vec![
            Column::build("Dialog_").primary_key().id_string(72),
            Column::build("Control_").primary_key().id_string(50),
            Column::build("Event").primary_key().id_string(50),
            Column::build("Attribute").primary_key().id_string(50),
        ],
    )?;
    create_table(
        package,
        "ActionText",
        vec![
            Column::build("Action").primary_key().id_string(72),
            Column::build("Description").nullable().text_string(64),
            Column::build("Template").nullable().text_string(128),
        ],
    )?;
    create_table(
        package,
        "ControlEvent",
        vec![
            Column::build("Dialog_").primary_key().id_string(72),
            Column::build("Control_").primary_key().id_string(50),
            Column::build("Event").primary_key().formatted_string(50),
            Column::build("Argument")
                .primary_key()
                .formatted_string(255),
            Column::build("Condition").nullable().formatted_string(255),
            Column::build("Ordering").nullable().int16(),
        ],
    )?;
    create_table(
        package,
        "ControlCondition",
        vec![
            Column::build("Dialog_").primary_key().id_string(72),
            Column::build("Control_").primary_key().id_string(50),
            Column::build("Action").primary_key().id_string(50),
            Column::build("Condition")
                .primary_key()
                .formatted_string(255),
        ],
    )?;

    Ok(())
}

fn create_table<W>(
    package: &mut Package<W>,
    table: &str,
    columns: Vec<Column>,
) -> anyhow::Result<()>
where
    W: Read + Write + Seek,
{
    package
        .create_table(table, columns)
        .with_context(|| format!("failed to create MSI table {table}"))
}

#[allow(clippy::too_many_arguments)]
fn insert_rows<W: Read + Write + Seek>(
    package: &mut Package<W>,
    plan: &InstallerConfig,
    cabinet_stream: &str,
    files: &[PackageFile],
    directory_ids: &DirectoryIds,
    shortcuts: &[ShortcutComponent],
    icons: &[IconStream],
    eulas: &[Eula],
    path_updates: &[String],
    folder_picker_action: &[u8],
) -> anyhow::Result<()> {
    let manufacturer = manufacturer(plan);
    let display_name = display_name(plan);

    let mut property_rows = vec![
        row([
            "ProductCode",
            &product_code(plan).braced().to_string().to_ascii_uppercase(),
        ]),
        row(["ProductName", display_name]),
        row(["ProductVersion", &msi_version(&plan.app_version)]),
        row(["ProductLanguage", "1033"]),
        row(["Manufacturer", manufacturer]),
        row([
            "UpgradeCode",
            &upgrade_code(plan).braced().to_string().to_ascii_uppercase(),
        ]),
        row(["ALLUSERS", "2"]),
        row(["MSIINSTALLPERUSER", "1"]),
        row(["ARPNOREPAIR", "1"]),
        row(["ARPNOMODIFY", "1"]),
        row(["DefaultUIFont", "DefaultFont"]),
        row([
            "SecureCustomProperties",
            "USERPROFILEDIR;INSTALLFOLDER;ADD_TO_PATH",
        ]),
    ];
    if !path_updates.is_empty() {
        property_rows.push(row([ADD_TO_PATH_PROPERTY, "1"]));
    }
    if let Some(product_icon) = product_icon(icons) {
        property_rows.push(row(["ARPPRODUCTICON", product_icon]));
    }
    insert(package, "Property", property_rows)?;
    insert(
        package,
        "AppSearch",
        vec![vec![
            Value::from("USERPROFILEDIR"),
            Value::from("UserProfileRegistry"),
        ]],
    )?;
    insert(
        package,
        "RegLocator",
        vec![vec![
            Value::from("UserProfileRegistry"),
            Value::Int(HKCU),
            Value::from("Volatile Environment"),
            Value::from("USERPROFILE"),
            Value::Int(2),
        ]],
    )?;
    insert(
        package,
        "Directory",
        directory_rows(plan, directory_ids, shortcuts)?,
    )?;
    insert(
        package,
        "Feature",
        vec![vec![
            Value::from("DefaultFeature"),
            Value::Null,
            Value::from(display_name),
            Value::from(display_name),
            Value::Int(1),
            Value::Int(1),
            Value::from("INSTALLFOLDER"),
            Value::Int(0),
        ]],
    )?;

    let component_attributes = component_attributes(&plan.bundle_target);
    let registry_component_attributes = component_attributes | COMPONENT_REGISTRY_KEYPATH;
    let mut component_rows: Vec<Vec<Value>> = files
        .iter()
        .map(|file| {
            vec![
                Value::from(file.component.as_str()),
                Value::from(
                    component_code(plan, file)
                        .braced()
                        .to_string()
                        .to_ascii_uppercase()
                        .as_str(),
                ),
                Value::from(file.directory.as_str()),
                Value::Int(component_attributes),
                Value::Null,
                Value::from(file.id.as_str()),
            ]
        })
        .collect();
    for (index, file) in plan
        .associated_files
        .iter()
        .filter(|file| matches!(file.kind, AssociatedFileKind::Directory))
        .enumerate()
    {
        let associated = associated_path(&file.path)?;
        let directory = directory_ids.id_for_existing(&associated.root, &associated.relative)?;
        component_rows.push(vec![
            Value::from(associated_directory_component(index).as_str()),
            Value::from(
                Uuid::new_v5(
                    &Uuid::NAMESPACE_URL,
                    format!(
                        "cargo-crapapp:msi:v2:directory-component:{}:{}:{}:{directory}",
                        plan.app_name, plan.app_version, plan.bundle_target
                    )
                    .as_bytes(),
                )
                .braced()
                .to_string()
                .to_ascii_uppercase()
                .as_str(),
            ),
            Value::from(directory.as_str()),
            Value::Int(registry_component_attributes),
            Value::Null,
            Value::from(associated_directory_registry(index).as_str()),
        ]);
    }
    component_rows.extend(shortcuts.iter().map(|shortcut| {
        vec![
            Value::from(shortcut.component.as_str()),
            Value::from(
                shortcut_component_code(plan, shortcut)
                    .braced()
                    .to_string()
                    .to_ascii_uppercase()
                    .as_str(),
            ),
            Value::from(shortcut.directory.as_str()),
            Value::Int(registry_component_attributes),
            Value::Null,
            Value::from(shortcut.registry.as_str()),
        ]
    }));
    component_rows.extend(path_updates.iter().enumerate().map(|(index, directory)| {
        vec![
            Value::from(path_update_component(index).as_str()),
            Value::from(
                path_component_code(plan, index)
                    .braced()
                    .to_string()
                    .to_ascii_uppercase()
                    .as_str(),
            ),
            Value::from(directory.as_str()),
            Value::Int(registry_component_attributes),
            Value::from(format!("{ADD_TO_PATH_PROPERTY}=\"1\"").as_str()),
            Value::from(path_update_registry(index).as_str()),
        ]
    }));
    insert(package, "Component", component_rows)?;

    let mut feature_component_rows: Vec<Vec<Value>> = files
        .iter()
        .map(|file| {
            vec![
                Value::from("DefaultFeature"),
                Value::from(file.component.as_str()),
            ]
        })
        .collect();
    feature_component_rows.extend(
        plan.associated_files
            .iter()
            .filter(|file| matches!(file.kind, AssociatedFileKind::Directory))
            .enumerate()
            .map(|(index, _)| {
                vec![
                    Value::from("DefaultFeature"),
                    Value::from(associated_directory_component(index).as_str()),
                ]
            }),
    );
    feature_component_rows.extend(shortcuts.iter().map(|shortcut| {
        vec![
            Value::from("DefaultFeature"),
            Value::from(shortcut.component.as_str()),
        ]
    }));
    feature_component_rows.extend(path_updates.iter().enumerate().map(|(index, _)| {
        vec![
            Value::from("DefaultFeature"),
            Value::from(path_update_component(index).as_str()),
        ]
    }));
    insert(package, "FeatureComponents", feature_component_rows)?;

    let file_rows = files
        .iter()
        .enumerate()
        .map(|(index, file)| {
            vec![
                Value::from(file.id.as_str()),
                Value::from(file.component.as_str()),
                Value::from(file.file_name.as_str()),
                Value::Int(file.size),
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Int(index as i32 + 1),
            ]
        })
        .collect();
    insert(package, "File", file_rows)?;
    let create_folder_rows = plan
        .associated_files
        .iter()
        .filter(|file| matches!(file.kind, AssociatedFileKind::Directory))
        .enumerate()
        .map(|(index, file)| {
            let associated = associated_path(&file.path)?;
            let directory =
                directory_ids.id_for_existing(&associated.root, &associated.relative)?;
            Ok(vec![
                Value::from(directory.as_str()),
                Value::from(associated_directory_component(index).as_str()),
            ])
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    if !create_folder_rows.is_empty() {
        insert(package, "CreateFolder", create_folder_rows)?;
    }
    if !shortcuts.is_empty() {
        insert(
            package,
            "Shortcut",
            shortcuts
                .iter()
                .map(|shortcut| {
                    vec![
                        Value::from(shortcut.id.as_str()),
                        Value::from(shortcut.directory.as_str()),
                        Value::from(shortcut.name.as_str()),
                        Value::from(shortcut.component.as_str()),
                        Value::from(format!("[#{}]", shortcut.target_file).as_str()),
                        Value::Null,
                        Value::from(display_name_from_msi_filename(&shortcut.name)),
                        Value::Null,
                        shortcut
                            .icon
                            .as_deref()
                            .map(Value::from)
                            .unwrap_or(Value::Null),
                        Value::Int(0),
                        Value::Null,
                        Value::Null,
                    ]
                })
                .collect(),
        )?;
    }
    if !icons.is_empty() {
        insert(
            package,
            "Icon",
            icons
                .iter()
                .map(|(id, _)| vec![Value::from(id.as_str()), Value::Binary])
                .collect(),
        )?;
        for (id, source) in icons {
            package
                .write_stream(&format!("Icon.{id}"))
                .with_context(|| format!("failed to create MSI icon stream {id}"))?
                .write_all(
                    &fs::read(source)
                        .with_context(|| format!("failed to read icon {}", source.display()))?,
                )
                .with_context(|| format!("failed to write MSI icon stream {id}"))?;
        }
    }
    let registry_rows = registry_rows(plan, files, directory_ids, shortcuts, path_updates)?;
    if !registry_rows.is_empty() {
        insert(package, "Registry", registry_rows)?;
        insert(
            package,
            "RemoveRegistry",
            remove_registry_rows(plan, files, shortcuts, path_updates)?,
        )?;
    }
    if !path_updates.is_empty() {
        insert(package, "Environment", environment_rows(path_updates))?;
    }
    let mut checkbox_rows = eulas
        .iter()
        .enumerate()
        .map(|(index, _)| vec![Value::from(eula_property(index)), Value::from("1")])
        .collect::<Vec<_>>();
    if !path_updates.is_empty() {
        checkbox_rows.push(vec![Value::from(ADD_TO_PATH_PROPERTY), Value::from("1")]);
    }
    if !checkbox_rows.is_empty() {
        insert(package, "CheckBox", checkbox_rows)?;
    }
    insert(
        package,
        "Binary",
        vec![vec![Value::from("PickInstallFolderAction"), Value::Binary]],
    )?;
    package
        .write_stream("Binary.PickInstallFolderAction")
        .context("failed to create MSI folder picker action stream")?
        .write_all(folder_picker_action)
        .context("failed to write MSI folder picker action stream")?;
    insert(
        package,
        "CustomAction",
        vec![vec![
            Value::from("PickInstallFolder"),
            Value::Int(1),
            Value::from("PickInstallFolderAction"),
            Value::from("PickInstallFolder"),
        ]],
    )?;
    insert(
        package,
        "EventMapping",
        vec![
            vec![
                Value::from("ProgressDlg"),
                Value::from("Progress"),
                Value::from("SetProgress"),
                Value::from("Progress"),
            ],
            vec![
                Value::from("ProgressDlg"),
                Value::from("ActionText"),
                Value::from("ActionText"),
                Value::from("Text"),
            ],
        ],
    )?;
    insert(package, "ActionText", super::tables::ActionText::rows())?;
    insert(
        package,
        "Media",
        vec![vec![
            Value::Int(1),
            Value::Int(files.len() as i32),
            Value::Null,
            Value::from(format!("#{cabinet_stream}").as_str()),
            Value::Null,
            Value::Null,
        ]],
    )?;
    insert(
        package,
        "InstallExecuteSequence",
        super::tables::InstallExecuteSequence::rows(),
    )?;
    insert(
        package,
        "InstallUISequence",
        super::tables::InstallUiSequence::rows(),
    )?;
    insert(package, "TextStyle", super::tables::TextStyle::rows())?;
    insert(package, "Dialog", super::screens::dialog_rows(plan, eulas))?;
    insert(package, "Control", control_rows(plan, eulas, path_updates))?;
    insert(package, "ControlEvent", control_event_rows(plan, eulas))?;
    let control_conditions = control_condition_rows(eulas);
    if !control_conditions.is_empty() {
        insert(package, "ControlCondition", control_conditions)?;
    }

    Ok(())
}

fn directory_rows(
    plan: &InstallerConfig,
    directory_ids: &DirectoryIds,
    shortcuts: &[ShortcutComponent],
) -> anyhow::Result<Vec<Vec<Value>>> {
    let mut parents = BTreeMap::<String, (String, String)>::new();
    parents.insert(
        "TARGETDIR".to_owned(),
        (String::new(), "SourceDir".to_owned()),
    );
    parents.insert(
        "USERPROFILEDIR".to_owned(),
        ("TARGETDIR".to_owned(), ".".to_owned()),
    );
    parents.insert(
        "System64Folder".to_owned(),
        ("TARGETDIR".to_owned(), ".".to_owned()),
    );
    if let Some(publisher) = plan.publisher_name() {
        parents.insert(
            "PUBLISHERFOLDER".to_owned(),
            ("USERPROFILEDIR".to_owned(), default_dir(publisher)),
        );
        parents.insert(
            "INSTALLFOLDER".to_owned(),
            (
                "PUBLISHERFOLDER".to_owned(),
                default_dir(plan.install_app_name()),
            ),
        );
    } else {
        parents.insert(
            "INSTALLFOLDER".to_owned(),
            (
                "USERPROFILEDIR".to_owned(),
                default_dir(plan.install_app_name()),
            ),
        );
    }
    parents.insert(
        "PersonalFolder".to_owned(),
        ("TARGETDIR".to_owned(), ".".to_owned()),
    );
    parents.insert(
        "ProgramMenuFolder".to_owned(),
        ("TARGETDIR".to_owned(), ".".to_owned()),
    );

    let directory_map = &directory_ids.ids;
    let mut directories = directory_map
        .iter()
        .map(|((root, path), id)| (root.clone(), path.clone(), id.clone()))
        .collect::<Vec<_>>();
    directories.sort_by(|left, right| left.1.cmp(&right.1).then(left.0.cmp(&right.0)));
    for (root, path, id) in directories {
        if id == root {
            continue;
        }
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .and_then(|parent| {
                directory_map
                    .get(&(root.clone(), parent.to_path_buf()))
                    .cloned()
            })
            .unwrap_or_else(|| root.clone());
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow::anyhow!("MSI directory {} must be UTF-8", path.display()))?;
        parents.insert(id, (parent, default_dir(name)));
    }
    for shortcut in shortcuts {
        parents
            .entry(shortcut.directory.clone())
            .or_insert_with(|| {
                (
                    "ProgramMenuFolder".to_owned(),
                    default_dir(&shortcut.directory_name),
                )
            });
    }

    let mut rows = Vec::new();
    for (directory, (parent, name)) in parents {
        rows.push(vec![
            Value::from(directory.as_str()),
            if parent.is_empty() {
                Value::Null
            } else {
                Value::from(parent.as_str())
            },
            Value::from(name.as_str()),
        ]);
    }

    Ok(rows)
}

fn default_dir(name: &str) -> String {
    msi_filename(name)
}

fn msi_filename(name: &str) -> String {
    let sanitized = name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "-");
    if is_short_filename(&sanitized) {
        return sanitized;
    }

    format!("{}|{}", short_filename(&sanitized), sanitized)
}

fn is_short_filename(name: &str) -> bool {
    let mut parts = name.split('.');
    let stem = parts.next().unwrap_or_default();
    let extension = parts.next();
    if parts.next().is_some() || stem.is_empty() || stem.len() > 8 {
        return false;
    }
    if extension.is_some_and(|extension| extension.is_empty() || extension.len() > 3) {
        return false;
    }

    name.chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '$' | '~'))
}

fn short_filename(name: &str) -> String {
    let (stem, extension) = name.rsplit_once('.').unwrap_or((name, ""));
    let mut short_stem = short_part(stem, 6);
    if short_stem.is_empty() {
        short_stem.push_str("FILE");
    }
    short_stem.push_str("~1");

    let short_extension = short_part(extension, 3);
    if short_extension.is_empty() {
        short_stem
    } else {
        format!("{short_stem}.{short_extension}")
    }
}

fn short_part(value: &str, max_len: usize) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(max_len)
        .map(|character| character.to_ascii_uppercase())
        .collect()
}

fn display_name_from_msi_filename(name: &str) -> &str {
    name.rsplit_once('|')
        .map(|(_, long_name)| long_name)
        .unwrap_or(name)
}

fn control_rows(
    plan: &InstallerConfig,
    eulas: &[Eula],
    path_updates: &[String],
) -> Vec<Vec<Value>> {
    let display_name = display_name(plan);
    let welcome_title = format!("Welcome to the {display_name} Setup Wizard");
    let welcome_body = format!(
        "\nThis wizard will install {} {} on your computer.",
        display_name, plan.app_version,
    );
    let install_dir_title = format!("Choose where to install {display_name}");
    let install_dir_body = "Choose the directory to where application will be installed.";
    let ready_title = format!("Ready to install {display_name}");
    let ready_body = "Click Install to begin installation.";
    let exit_title = format!("Completed the Setup Wizard for {display_name}");

    let mut rows = vec![
        title_control("WelcomeDlg", "Title", 24, 24, 472, 40, &welcome_title),
        text_control("WelcomeDlg", "Body", 24, 82, 472, 80, &welcome_body),
        line_control("WelcomeDlg"),
        footer_button("WelcomeDlg", "Cancel", CANCEL_BUTTON_X, "Cancel"),
        footer_button("WelcomeDlg", "Next", NEXT_BUTTON_X, "Next"),
        title_control(
            "InstallDirDlg",
            "Title",
            24,
            24,
            472,
            40,
            &install_dir_title,
        ),
        text_control("InstallDirDlg", "Body", 24, 72, 472, 32, install_dir_body),
        heading_control(
            "InstallDirDlg",
            "PathLabel",
            44,
            122,
            432,
            18,
            "Installation folder",
        ),
        edit_control(
            "InstallDirDlg",
            "PathEdit",
            44,
            150,
            344,
            18,
            "INSTALLFOLDER",
        ),
        button_control("InstallDirDlg", "Browse", 396, 148, 80, 22, "Browse..."),
        checkbox_control(
            "InstallDirDlg",
            "AddToPath",
            44,
            198,
            432,
            20,
            ADD_TO_PATH_PROPERTY,
            "Add to PATH",
            !path_updates.is_empty(),
        ),
        line_control("InstallDirDlg"),
        footer_button("InstallDirDlg", "Back", BACK_BUTTON_X, "Back"),
        footer_button("InstallDirDlg", "Cancel", CANCEL_BUTTON_X, "Cancel"),
        footer_button("InstallDirDlg", "Next", NEXT_BUTTON_X, "Next"),
        title_control("VerifyReadyDlg", "Title", 24, 24, 472, 40, &ready_title),
        text_control("VerifyReadyDlg", "Body", 24, 82, 472, 60, ready_body),
        line_control("VerifyReadyDlg"),
        footer_button("VerifyReadyDlg", "Back", BACK_BUTTON_X, "Back"),
        footer_button("VerifyReadyDlg", "Cancel", CANCEL_BUTTON_X, "Cancel"),
        footer_button("VerifyReadyDlg", "Install", NEXT_BUTTON_X, "Install"),
        title_control(
            "ProgressDlg",
            "Title",
            24,
            24,
            472,
            40,
            &format!("Installing {display_name}"),
        ),
        text_control(
            "ProgressDlg",
            "Version",
            24,
            78,
            472,
            22,
            &format!("Version {}", plan.app_version),
        ),
        text_control(
            "ProgressDlg",
            "ActionText",
            24,
            122,
            472,
            24,
            "Preparing installation...",
        ),
        control_row(
            "ProgressDlg",
            "Progress",
            "ProgressBar",
            24,
            158,
            472,
            18,
            1,
            None,
            None,
        ),
        line_control("ProgressDlg"),
        footer_button("ProgressDlg", "Cancel", NEXT_BUTTON_X, "Cancel"),
        title_control("ExitDlg", "Title", 24, 24, 472, 40, &exit_title),
        text_control(
            "ExitDlg",
            "Body",
            24,
            82,
            472,
            60,
            "Installation completed successfully.",
        ),
        line_control("ExitDlg"),
        footer_button("ExitDlg", "Finish", NEXT_BUTTON_X, "Finish"),
    ];

    for (index, eula) in eulas.iter().enumerate() {
        let position = format!("License agreement {} of {}", index + 1, eulas.len());
        let dialog = license_dialog_id(index);
        rows.extend([
            title_control(&dialog, "Title", 24, 20, 472, 34, &position),
            heading_control(&dialog, "Name", 24, 58, 472, 20, &eula.name),
            control_row(
                &dialog,
                "LicenseText",
                "ScrollableText",
                24,
                86,
                472,
                174,
                7,
                None,
                Some(&eula_rtf(&eula.text)),
            ),
            line_control(&dialog),
            footer_button(&dialog, "Back", BACK_BUTTON_X, "Back"),
            footer_button(&dialog, "Cancel", CANCEL_BUTTON_X, "Cancel"),
            footer_button(&dialog, "Next", NEXT_BUTTON_X, "Next"),
        ]);
        rows.push(checkbox_control(
            &dialog,
            "Accept",
            24,
            272,
            472,
            22,
            &eula_property(index),
            &format!("I accept {}", eula.name),
            true,
        ));
    }

    rows
}

fn text_control(
    dialog: &str,
    control: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    text: &str,
) -> Vec<Value> {
    control_row(
        dialog,
        control,
        "Text",
        x,
        y,
        width,
        height,
        1,
        None,
        Some(text),
    )
}

fn title_control(
    dialog: &str,
    control: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    text: &str,
) -> Vec<Value> {
    text_control(
        dialog,
        control,
        x,
        y,
        width,
        height,
        &format!("{{\\TitleFont}}{text}"),
    )
}

fn heading_control(
    dialog: &str,
    control: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    text: &str,
) -> Vec<Value> {
    text_control(
        dialog,
        control,
        x,
        y,
        width,
        height,
        &format!("{{\\HeadingFont}}{text}"),
    )
}

fn line_control(dialog: &str) -> Vec<Value> {
    control_row(
        dialog,
        "BottomLine",
        "Line",
        0,
        FOOTER_LINE_Y,
        DIALOG_WIDTH,
        0,
        1,
        None,
        None,
    )
}

fn button_control(
    dialog: &str,
    control: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    text: &str,
) -> Vec<Value> {
    control_row(
        dialog,
        control,
        "PushButton",
        x,
        y,
        width,
        height,
        3,
        None,
        Some(text),
    )
}

fn footer_button(dialog: &str, control: &str, x: i32, text: &str) -> Vec<Value> {
    button_control(
        dialog,
        control,
        x,
        FOOTER_BUTTON_Y,
        BUTTON_WIDTH,
        BUTTON_HEIGHT,
        text,
    )
}

#[allow(clippy::too_many_arguments)]
fn checkbox_control(
    dialog: &str,
    control: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    property: &str,
    text: &str,
    enabled: bool,
) -> Vec<Value> {
    control_row(
        dialog,
        control,
        "CheckBox",
        x,
        y,
        width,
        height,
        if enabled { 3 } else { 1 },
        Some(property),
        Some(text),
    )
}

fn edit_control(
    dialog: &str,
    control: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    property: &str,
) -> Vec<Value> {
    control_row(
        dialog,
        control,
        "Edit",
        x,
        y,
        width,
        height,
        3,
        Some(property),
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn control_row(
    dialog: &str,
    control: &str,
    control_type: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    attributes: i32,
    property: Option<&str>,
    text: Option<&str>,
) -> Vec<Value> {
    vec![
        Value::from(dialog),
        Value::from(control),
        Value::from(control_type),
        Value::Int(x),
        Value::Int(y),
        Value::Int(width),
        Value::Int(height),
        Value::Int(attributes),
        property.map(Value::from).unwrap_or(Value::Null),
        text.map(Value::from).unwrap_or(Value::Null),
        Value::Null,
        Value::Null,
    ]
}

fn control_event_rows(plan: &InstallerConfig, eulas: &[Eula]) -> Vec<Vec<Value>> {
    let first_license_dialog = (!eulas.is_empty()).then(|| license_dialog_id(0));
    let first_non_license_dialog = if plan.allows_install_path_selection() {
        "InstallDirDlg"
    } else {
        "VerifyReadyDlg"
    };
    let first_settings_dialog = eulas
        .first()
        .map(|_| {
            first_license_dialog
                .as_deref()
                .unwrap_or(first_non_license_dialog)
        })
        .unwrap_or(first_non_license_dialog);
    let mut rows = vec![
        (
            "WelcomeDlg",
            "Next",
            "NewDialog",
            first_settings_dialog,
            "1".to_owned(),
            1,
        ),
        (
            "WelcomeDlg",
            "Cancel",
            "EndDialog",
            "Exit",
            "1".to_owned(),
            1,
        ),
    ]
    .into_iter()
    .map(|(dialog, control, event, argument, condition, ordering)| {
        vec![
            Value::from(dialog),
            Value::from(control),
            Value::from(event),
            Value::from(argument),
            Value::from(condition.as_str()),
            Value::Int(ordering),
        ]
    })
    .collect::<Vec<_>>();

    for (index, eula) in eulas.iter().enumerate() {
        let dialog = license_dialog_id(index);
        let previous_dialog = if index == 0 {
            None
        } else {
            Some(license_dialog_id(index - 1))
        };
        let next_dialog = eulas.get(index + 1).map(|_| license_dialog_id(index + 1));
        let previous = if index == 0 {
            "WelcomeDlg"
        } else {
            previous_dialog.as_deref().unwrap_or("WelcomeDlg")
        };
        let next = eulas
            .get(index + 1)
            .map(|_| next_dialog.as_deref().unwrap_or(first_non_license_dialog))
            .unwrap_or(first_non_license_dialog);
        let condition = if eula.required {
            format!("{}=\"1\"", eula_property(index))
        } else {
            "1".to_owned()
        };
        rows.extend(control_event_values([
            (&dialog, "Back", "NewDialog", previous, "1".to_owned(), 1),
            (&dialog, "Next", "NewDialog", next, condition, 1),
            (&dialog, "Cancel", "EndDialog", "Exit", "1".to_owned(), 1),
        ]));
    }

    let last_license_dialog = (!eulas.is_empty()).then(|| license_dialog_id(eulas.len() - 1));
    let pre_install_back = eulas
        .last()
        .map(|_| last_license_dialog.as_deref().unwrap_or("WelcomeDlg"))
        .unwrap_or("WelcomeDlg");
    rows.extend(control_event_values([
        (
            "InstallDirDlg",
            "Back",
            "NewDialog",
            pre_install_back,
            "1".to_owned(),
            1,
        ),
        (
            "InstallDirDlg",
            "Browse",
            "DoAction",
            "PickInstallFolder",
            "1".to_owned(),
            1,
        ),
        (
            "InstallDirDlg",
            "Browse",
            "SetTargetPath",
            "INSTALLFOLDER",
            "1".to_owned(),
            2,
        ),
        // Publish INSTALLFOLDER once more through the MSI UI handler. This
        // refreshes the bound Edit control without closing and reopening the
        // current dialog.
        (
            "InstallDirDlg",
            "Browse",
            "[INSTALLFOLDER]",
            "[INSTALLFOLDER]",
            "1".to_owned(),
            3,
        ),
        (
            "InstallDirDlg",
            "Next",
            "SetTargetPath",
            "INSTALLFOLDER",
            "1".to_owned(),
            1,
        ),
        (
            "InstallDirDlg",
            "Next",
            "NewDialog",
            "VerifyReadyDlg",
            "1".to_owned(),
            2,
        ),
        (
            "InstallDirDlg",
            "Cancel",
            "EndDialog",
            "Exit",
            "1".to_owned(),
            1,
        ),
        (
            "VerifyReadyDlg",
            "Back",
            "NewDialog",
            if plan.allows_install_path_selection() {
                "InstallDirDlg"
            } else {
                pre_install_back
            },
            "1".to_owned(),
            1,
        ),
        (
            "VerifyReadyDlg",
            "Install",
            "EndDialog",
            "Return",
            "1".to_owned(),
            1,
        ),
        (
            "VerifyReadyDlg",
            "Cancel",
            "EndDialog",
            "Exit",
            "1".to_owned(),
            1,
        ),
        (
            "ProgressDlg",
            "Cancel",
            "EndDialog",
            "Exit",
            "1".to_owned(),
            1,
        ),
        (
            "ExitDlg",
            "Finish",
            "EndDialog",
            "Return",
            "1".to_owned(),
            1,
        ),
    ]));

    rows
}

fn control_event_values<'a, const N: usize>(
    rows: [(&'a str, &'a str, &'a str, &'a str, String, i32); N],
) -> Vec<Vec<Value>> {
    rows.into_iter()
        .map(|(dialog, control, event, argument, condition, ordering)| {
            vec![
                Value::from(dialog),
                Value::from(control),
                Value::from(event),
                Value::from(argument),
                Value::from(condition.as_str()),
                Value::Int(ordering),
            ]
        })
        .collect()
}

fn control_condition_rows(eulas: &[Eula]) -> Vec<Vec<Value>> {
    let mut rows = Vec::new();

    for (index, eula) in eulas.iter().enumerate() {
        if eula.required {
            let property = eula_property(index);
            let dialog = license_dialog_id(index);
            rows.push(vec![
                Value::from(dialog.as_str()),
                Value::from("Next"),
                Value::from("Disable"),
                Value::from(format!("{property}<>\"1\"").as_str()),
            ]);
            rows.push(vec![
                Value::from(dialog.as_str()),
                Value::from("Next"),
                Value::from("Enable"),
                Value::from(format!("{property}=\"1\"").as_str()),
            ]);
        }
    }

    rows
}

fn insert<W: Read + Write + Seek>(
    package: &mut Package<W>,
    table: &str,
    rows: Vec<Vec<Value>>,
) -> anyhow::Result<()> {
    package
        .insert_rows(Insert::into(table).rows(rows))
        .with_context(|| format!("failed to insert MSI table {table} rows"))
}

fn row(values: [&str; 2]) -> Vec<Value> {
    values.into_iter().map(Value::from).collect()
}

fn shortcuts(
    plan: &InstallerConfig,
    files: &[PackageFile],
) -> anyhow::Result<Vec<ShortcutComponent>> {
    let mut output = Vec::new();

    for (index, shortcut) in plan.shortcuts.iter().enumerate() {
        let target = install_relative_path(&shortcut.target)?;
        let file = files
            .iter()
            .find(|file| file.directory_root == "INSTALLFOLDER" && file.install_path == target)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "MSI shortcut {} target {} is not in payload",
                    shortcut.name,
                    shortcut.target
                )
            })?;
        let directory = shortcut
            .directory
            .as_deref()
            .map(|directory| format!("ShortcutDir{}", identifier(directory)))
            .unwrap_or_else(|| "ProgramMenuFolder".to_owned());
        let directory_name = shortcut
            .directory
            .clone()
            .unwrap_or_else(|| display_name(plan).to_owned());
        let icon = shortcut
            .icon
            .as_deref()
            .map(|icon| shortcut_icon_id(icon, files))
            .transpose()?;

        output.push(ShortcutComponent {
            id: format!("Shortcut{}", index + 1),
            directory,
            directory_name,
            name: default_dir(&shortcut.name),
            component: format!("ShortcutComponent{}", index + 1),
            registry: format!("ShortcutRegistry{}", index + 1),
            target_file: file.id.clone(),
            icon,
        });
    }

    Ok(output)
}

fn icons(
    plan: &InstallerConfig,
    files: &[PackageFile],
    shortcuts: &[ShortcutComponent],
    display_icon_source: Option<&str>,
) -> anyhow::Result<Vec<IconStream>> {
    let mut output = Vec::new();

    if let Some(source) = display_icon_source {
        output.push(("ProductIcon".to_owned(), PathBuf::from(source)));
    } else if let Some(display_icon) = plan.display_icon.as_deref()
        && let Some(source) = installed_file_source(display_icon, files)
    {
        output.push(("ProductIcon".to_owned(), source));
    }

    for shortcut in shortcuts
        .iter()
        .filter_map(|shortcut| shortcut.icon.as_deref())
    {
        if output.iter().any(|(id, _)| id == shortcut) {
            continue;
        }
        let source = files
            .iter()
            .find(|file| shortcut == shortcut_icon_id_from_file(file))
            .map(|file| file.source.clone())
            .ok_or_else(|| {
                anyhow::anyhow!("MSI shortcut icon {shortcut} is not backed by a payload file")
            })?;
        output.push((shortcut.to_owned(), source));
    }

    Ok(output)
}

fn product_icon(icons: &[IconStream]) -> Option<&str> {
    icons.first().map(|(id, _)| id.as_str())
}

fn shortcut_icon_id(icon: &str, files: &[PackageFile]) -> anyhow::Result<String> {
    let icon_path = install_relative_path(icon)?;
    files
        .iter()
        .find(|file| file.directory_root == "INSTALLFOLDER" && file.install_path == icon_path)
        .map(shortcut_icon_id_from_file)
        .ok_or_else(|| anyhow::anyhow!("MSI shortcut icon {icon} is not in payload"))
}

fn shortcut_icon_id_from_file(file: &PackageFile) -> String {
    format!("Icon{}", identifier(&file.id))
}

fn installed_file_source(destination: &str, files: &[PackageFile]) -> Option<PathBuf> {
    let path = install_relative_path(destination).ok()?;
    files
        .iter()
        .find(|file| file.directory_root == "INSTALLFOLDER" && file.install_path == path)
        .map(|file| file.source.clone())
}

fn license_dialog_id(index: usize) -> String {
    format!("LicenseDlg{}", index + 1)
}

fn eula_property(index: usize) -> String {
    format!("EULA_ACCEPTED_{}", index + 1)
}

fn eula_rtf(text: &str) -> String {
    if text.trim_start().starts_with("{\\rtf") {
        return text.to_owned();
    }

    let mut output = String::from(
        "{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0 Segoe UI;}}\\viewkind4\\uc1\\pard\\f0\\fs18 ",
    );
    for character in text.chars() {
        match character {
            '\\' => output.push_str("\\\\"),
            '{' => output.push_str("\\{"),
            '}' => output.push_str("\\}"),
            '\r' => {}
            '\n' => output.push_str("\\par\n"),
            '\t' => output.push_str("\\tab "),
            character if character.is_ascii_control() => {}
            character if character.is_ascii() => output.push(character),
            character => {
                let mut units = [0; 2];
                for unit in character.encode_utf16(&mut units) {
                    output.push_str(&format!("\\u{}?", *unit as i16));
                }
            }
        }
    }
    output.push('}');
    output
}

fn path_updates(files: &[PackageFile]) -> Vec<String> {
    let mut directories = Vec::<String>::new();
    for file in files
        .iter()
        .filter(|file| file.directory_root == "INSTALLFOLDER")
        .filter(|file| {
            file.install_path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        })
    {
        if !directories.contains(&file.directory) {
            directories.push(file.directory.clone());
        }
    }

    directories
}

fn environment_rows(path_updates: &[String]) -> Vec<Vec<Value>> {
    path_updates
        .iter()
        .enumerate()
        .map(|(index, directory)| {
            vec![
                Value::from(path_update_id(index).as_str()),
                Value::from("=-Path"),
                Value::from(format!("[~];[{directory}]").as_str()),
                Value::from(path_update_component(index).as_str()),
            ]
        })
        .collect()
}

fn registry_rows(
    plan: &InstallerConfig,
    _files: &[PackageFile],
    directory_ids: &DirectoryIds,
    shortcuts: &[ShortcutComponent],
    path_updates: &[String],
) -> anyhow::Result<Vec<Vec<Value>>> {
    let mut rows = Vec::new();
    for (index, file) in plan
        .associated_files
        .iter()
        .filter(|file| matches!(file.kind, AssociatedFileKind::Directory))
        .enumerate()
    {
        let associated = associated_path(&file.path)?;
        let directory = directory_ids.id_for_existing(&associated.root, &associated.relative)?;
        rows.push(vec![
            Value::from(associated_directory_registry(index).as_str()),
            Value::Int(HKCU),
            Value::from(format!("Software\\cargo-crapapp\\{}", plan.app_name).as_str()),
            Value::from(associated_directory_registry(index).as_str()),
            Value::from(format!("[{directory}]").as_str()),
            Value::from(associated_directory_component(index).as_str()),
        ]);
    }
    for shortcut in shortcuts {
        rows.push(vec![
            Value::from(shortcut.registry.as_str()),
            Value::Int(HKCU),
            Value::from(format!("Software\\cargo-crapapp\\{}\\Shortcuts", plan.app_name).as_str()),
            Value::from(shortcut.registry.as_str()),
            Value::from("#1"),
            Value::from(shortcut.component.as_str()),
        ]);
    }
    for (index, directory) in path_updates.iter().enumerate() {
        rows.push(vec![
            Value::from(path_update_registry(index).as_str()),
            Value::Int(HKCU),
            Value::from(format!("Software\\cargo-crapapp\\{}\\Path", plan.app_name).as_str()),
            Value::from(path_update_registry(index).as_str()),
            Value::from(format!("[{directory}]").as_str()),
            Value::from(path_update_component(index).as_str()),
        ]);
    }

    Ok(rows)
}

fn remove_registry_rows(
    plan: &InstallerConfig,
    files: &[PackageFile],
    shortcuts: &[ShortcutComponent],
    path_updates: &[String],
) -> anyhow::Result<Vec<Vec<Value>>> {
    let key = format!("Software\\cargo-crapapp\\{}", plan.app_name);
    let component = cleanup_component(files, plan, shortcuts, path_updates)?;
    let mut rows = vec![vec![
        Value::from("RemoveAppRegistryKey"),
        Value::Int(HKCU),
        Value::from(key.as_str()),
        Value::Null,
        Value::from(component.as_str()),
    ]];
    if !shortcuts.is_empty() {
        rows.push(vec![
            Value::from("RemoveShortcutRegistryKey"),
            Value::Int(HKCU),
            Value::from(format!("{key}\\Shortcuts").as_str()),
            Value::Null,
            Value::from(component.as_str()),
        ]);
    }
    if !path_updates.is_empty() {
        rows.push(vec![
            Value::from("RemovePathRegistryKey"),
            Value::Int(HKCU),
            Value::from(format!("{key}\\Path").as_str()),
            Value::Null,
            Value::from(component.as_str()),
        ]);
    }

    Ok(rows)
}

fn cleanup_component<'a>(
    files: &'a [PackageFile],
    plan: &'a InstallerConfig,
    shortcuts: &'a [ShortcutComponent],
    path_updates: &'a [String],
) -> anyhow::Result<String> {
    if let Some(file) = files.first() {
        return Ok(file.component.clone());
    }
    if plan
        .associated_files
        .iter()
        .any(|file| matches!(file.kind, AssociatedFileKind::Directory))
    {
        return Ok(associated_directory_component(0));
    }
    if let Some(shortcut) = shortcuts.first() {
        return Ok(shortcut.component.clone());
    }
    if !path_updates.is_empty() {
        return Ok(path_update_component(0));
    }
    Err(anyhow!("MSI has no component to own registry cleanup"))
}

fn identifier(value: &str) -> String {
    let mut id = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            id.push(character);
        }
    }

    if id.is_empty() { "Item".to_owned() } else { id }
}

fn package_code(plan: &InstallerConfig) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!(
            "cargo-crapapp:msi:v3:package:{}:{}:{}:{}",
            plan.app_name, plan.app_version, plan.bundle_target, plan.bundled_at
        )
        .as_bytes(),
    )
}

fn product_code(plan: &InstallerConfig) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!(
            "cargo-crapapp:msi:v2:product:{}:{}:{}",
            plan.app_name, plan.app_version, plan.bundle_target
        )
        .as_bytes(),
    )
}

fn upgrade_code(plan: &InstallerConfig) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("cargo-crapapp:msi:upgrade:{}", plan.app_name).as_bytes(),
    )
}

fn component_code(plan: &InstallerConfig, file: &PackageFile) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!(
            "cargo-crapapp:msi:v2:component:{}:{}:{}:{}",
            plan.app_name, plan.app_version, plan.bundle_target, file.id
        )
        .as_bytes(),
    )
}

fn shortcut_component_code(plan: &InstallerConfig, shortcut: &ShortcutComponent) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!(
            "cargo-crapapp:msi:v2:shortcut-component:{}:{}:{}:{}",
            plan.app_name, plan.app_version, plan.bundle_target, shortcut.id
        )
        .as_bytes(),
    )
}

fn path_component_code(plan: &InstallerConfig, index: usize) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!(
            "cargo-crapapp:msi:v2:path-component:{}:{}:{}:{}",
            plan.app_name,
            plan.app_version,
            plan.bundle_target,
            path_update_id(index)
        )
        .as_bytes(),
    )
}

fn msi_version(version: &str) -> String {
    let mut parts = version.split('.').take(3).collect::<Vec<_>>();
    while parts.len() < 3 {
        parts.push("0");
    }
    parts.join(".")
}

fn msi_platform(target: &str) -> &'static str {
    match target.split_once('-').map(|(architecture, _)| architecture) {
        Some("aarch64") => "Arm64",
        Some("x86_64") => "x64",
        _ => "Intel",
    }
}

fn msi_page_count(target: &str) -> i32 {
    if target.starts_with("aarch64-") {
        500
    } else if target.starts_with("x86_64-") {
        200
    } else {
        100
    }
}

fn component_attributes(target: &str) -> i32 {
    if target.starts_with("aarch64-") || target.starts_with("x86_64-") {
        COMPONENT_64BIT
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use msi::{Expr, Select};

    use super::control_event_rows;
    use crate::windows_installer::{
        AssociatedFile, AssociatedFileKind, Eula, InstallerConfig, PayloadEntry, Shortcut,
        msi::build,
    };

    fn test_plan(
        payload: Vec<PayloadEntry>,
        associated_files: Vec<AssociatedFile>,
        shortcuts: Vec<Shortcut>,
        eulas: Vec<Eula>,
        display_icon: Option<String>,
        _display_icon_source: Option<String>,
    ) -> InstallerConfig {
        InstallerConfig {
            app_name: "example".to_owned(),
            app_version: "1.2.3".to_owned(),
            display_name: Some("Example App".to_owned()),
            publisher: Some("ufnkam".to_owned()),
            bundle_target: "x86_64-pc-windows-gnu".to_owned(),
            required_variables: vec!["INSTALLPATH".to_owned()],
            payload,
            associated_files,
            shortcuts,
            eulas,
            display_icon,
            ..Default::default()
        }
    }

    fn payload(source: impl ToString, destination: &str, executable: bool) -> PayloadEntry {
        PayloadEntry {
            source: Some(source.to_string()),
            destination: destination.to_owned(),
            executable,
            offset: 0,
            len: 0,
            bytes: &[],
        }
    }

    #[test]
    fn msi_skips_directory_selection_without_installpath_variable() {
        let mut plan = test_plan(Vec::new(), Vec::new(), Vec::new(), Vec::new(), None, None);
        plan.required_variables.clear();

        let events = control_event_rows(&plan, &[]);
        assert!(events.iter().any(|row| {
            row[0].as_str() == Some("WelcomeDlg")
                && row[1].as_str() == Some("Next")
                && row[2].as_str() == Some("NewDialog")
                && row[3].as_str() == Some("VerifyReadyDlg")
        }));
        assert!(events.iter().any(|row| {
            row[0].as_str() == Some("VerifyReadyDlg")
                && row[1].as_str() == Some("Back")
                && row[2].as_str() == Some("NewDialog")
                && row[3].as_str() == Some("WelcomeDlg")
        }));
    }

    #[test]
    fn msi_package_opens_as_installer_database() {
        let temp_dir =
            std::env::temp_dir().join(format!("cargo-crapapp-msi-{}", std::process::id()));
        let source = temp_dir.join("example.exe");
        let eula = temp_dir.join("EULA.txt");
        let output = temp_dir.join("example.msi");

        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        fs::write(&source, b"bin").expect("source should be written");
        fs::write(&eula, "License terms").expect("eula should be written");

        build(
            &test_plan(
                vec![payload(
                    source.display().to_string(),
                    "$INSTALLPATH/example.exe",
                    true,
                )],
                Vec::new(),
                Vec::new(),
                vec![
                    Eula {
                        name: "EULA.txt".to_owned(),
                        text: "License terms".to_owned(),
                        required: true,
                    },
                    Eula {
                        name: "NOTICE.txt".to_owned(),
                        text: "Optional notice".to_owned(),
                        required: false,
                    },
                ],
                None,
                None,
            ),
            &output,
            b"folder-picker-action",
            None,
        )
        .expect("msi should be written");

        let mut package = msi::open(&output).expect("msi should open");
        assert_eq!(package.summary_info().arch(), Some("x64"));
        assert_eq!(package.summary_info().page_count(), Some(200));
        assert_eq!(package.summary_info().word_count(), Some(10));
        assert_eq!(
            package
                .select_rows(
                    Select::table("Property")
                        .with(Expr::col("Property").eq(Expr::string("ALLUSERS")))
                )
                .unwrap()
                .next()
                .and_then(|row| row["Value"].as_str().map(str::to_owned))
                .as_deref(),
            Some("2")
        );
        assert_eq!(
            package
                .select_rows(
                    Select::table("Component")
                        .with(Expr::col("Component").eq(Expr::string("Component1")))
                )
                .unwrap()
                .next()
                .and_then(|row| row["Attributes"].as_int()),
            Some(super::COMPONENT_64BIT)
        );
        assert!(
            package
                .get_table("Directory")
                .expect("Directory table should exist")
                .has_column("Directory_Parent")
        );
        assert!(
            package
                .get_table("Feature")
                .expect("Feature table should exist")
                .has_column("Feature_Parent")
        );
        assert!(package.has_table("Product") || package.has_table("Property"));
        assert!(package.has_table("Shortcut"));
        assert!(package.has_table("Icon"));
        assert!(package.has_table("CreateFolder"));
        assert!(package.has_table("InstallUISequence"));
        assert!(package.has_table("Dialog"));
        assert!(package.has_table("Control"));
        assert!(package.has_table("ControlEvent"));
        assert!(package.has_table("ControlCondition"));
        assert!(package.has_table("CheckBox"));
        assert!(package.has_table("Environment"));
        assert!(package.has_table("RemoveRegistry"));
        assert!(package.has_table("TextStyle"));
        assert!(package.has_stream("example.cab"));
        let welcome_dialog = package
            .select_rows(
                Select::table("Dialog").with(Expr::col("Dialog").eq(Expr::string("WelcomeDlg"))),
            )
            .unwrap()
            .next()
            .expect("welcome dialog should exist");
        assert_eq!(welcome_dialog["Width"].as_int(), Some(super::DIALOG_WIDTH));
        assert_eq!(
            welcome_dialog["Height"].as_int(),
            Some(super::DIALOG_HEIGHT)
        );
        let publisher_directory = package
            .select_rows(
                Select::table("Directory")
                    .with(Expr::col("Directory").eq(Expr::string("PUBLISHERFOLDER"))),
            )
            .unwrap()
            .next()
            .expect("publisher directory should exist");
        assert_eq!(
            publisher_directory["Directory_Parent"].as_str(),
            Some("USERPROFILEDIR")
        );
        assert_eq!(publisher_directory["DefaultDir"].as_str(), Some("ufnkam"));
        let install_directory = package
            .select_rows(
                Select::table("Directory")
                    .with(Expr::col("Directory").eq(Expr::string("INSTALLFOLDER"))),
            )
            .unwrap()
            .next()
            .expect("install directory should exist");
        assert_eq!(
            install_directory["Directory_Parent"].as_str(),
            Some("PUBLISHERFOLDER")
        );
        assert_eq!(install_directory["DefaultDir"].as_str(), Some("example"));
        assert_eq!(
            package
                .select_rows(
                    Select::table("AppSearch")
                        .with(Expr::col("Property").eq(Expr::string("USERPROFILEDIR")))
                )
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            package
                .select_rows(
                    Select::table("RegLocator")
                        .with(Expr::col("Signature_").eq(Expr::string("UserProfileRegistry")))
                )
                .unwrap()
                .len(),
            1
        );
        assert!(
            package
                .select_rows(
                    Select::table("InstallUISequence")
                        .with(Expr::col("Action").eq(Expr::string("WelcomeDlg")))
                )
                .unwrap()
                .len()
                == 1
        );
        assert!(
            package
                .select_rows(
                    Select::table("ControlEvent").with(
                        Expr::col("Dialog_")
                            .eq(Expr::string("InstallDirDlg"))
                            .and(Expr::col("Control_").eq(Expr::string("Browse")))
                            .and(Expr::col("Event").eq(Expr::string("[INSTALLFOLDER]")))
                            .and(Expr::col("Argument").eq(Expr::string("[INSTALLFOLDER]"))),
                    )
                )
                .unwrap()
                .len()
                == 1,
            "the folder picker must refresh the path field from INSTALLFOLDER"
        );
        assert_eq!(
            package
                .select_rows(
                    Select::table("CustomAction")
                        .with(Expr::col("Action").eq(Expr::string("BrowseForInstallFolder")))
                )
                .unwrap()
                .len(),
            0
        );
        for action in [
            "CleanupUserAddRemovePrograms",
            "CleanupMachineAddRemovePrograms",
        ] {
            assert_eq!(
                package
                    .select_rows(
                        Select::table("CustomAction")
                            .with(Expr::col("Action").eq(Expr::string(action)))
                    )
                    .unwrap()
                    .len(),
                0
            );
            assert_eq!(
                package
                    .select_rows(
                        Select::table("InstallExecuteSequence")
                            .with(Expr::col("Action").eq(Expr::string(action)))
                    )
                    .unwrap()
                    .len(),
                0
            );
        }
        assert_eq!(
            package
                .select_rows(
                    Select::table("Dialog")
                        .with(Expr::col("Dialog").eq(Expr::string("ProgressDlg")))
                )
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            package
                .select_rows(
                    Select::table("Control").with(
                        Expr::col("Dialog_")
                            .eq(Expr::string("ProgressDlg"))
                            .and(Expr::col("Type").eq(Expr::string("ProgressBar")))
                    )
                )
                .unwrap()
                .len(),
            1
        );
        let license_text = package
            .select_rows(
                Select::table("Control").with(
                    Expr::col("Dialog_")
                        .eq(Expr::string("LicenseDlg1"))
                        .and(Expr::col("Control").eq(Expr::string("LicenseText"))),
                ),
            )
            .unwrap()
            .next()
            .expect("license text should exist");
        let license_rtf = license_text["Text"]
            .as_str()
            .expect("license text should be embedded");
        assert!(license_rtf.starts_with("{\\rtf1"));
        assert!(license_rtf.contains("License terms"));
        assert_eq!(license_text["Attributes"].as_int(), Some(7));
        assert!(
            package
                .select_rows(
                    Select::table("Control").with(
                        Expr::col("Dialog_")
                            .eq(Expr::string("InstallDirDlg"))
                            .and(Expr::col("Control").eq(Expr::string("Browse")))
                    )
                )
                .unwrap()
                .len()
                == 1
        );
        assert!(
            package
                .select_rows(
                    Select::table("ControlEvent").with(
                        Expr::col("Dialog_")
                            .eq(Expr::string("InstallDirDlg"))
                            .and(Expr::col("Control_").eq(Expr::string("Browse")))
                            .and(Expr::col("Event").eq(Expr::string("DoAction")))
                            .and(Expr::col("Argument").eq(Expr::string("PickInstallFolder")),)
                    )
                )
                .unwrap()
                .len()
                == 1
        );
        assert_eq!(
            package
                .select_rows(
                    Select::table("Dialog").with(Expr::col("Dialog").eq(Expr::string("BrowseDlg"))),
                )
                .unwrap()
                .len(),
            0,
            "the legacy MSI BrowseDlg must not be authored"
        );
        assert_eq!(
            package
                .select_rows(
                    Select::table("CustomAction")
                        .with(Expr::col("Action").eq(Expr::string("PickInstallFolder"))),
                )
                .unwrap()
                .len(),
            1
        );
        assert!(package.has_stream("Binary.PickInstallFolderAction"));
        assert!(
            package
                .select_rows(
                    Select::table("Control").with(
                        Expr::col("Dialog_")
                            .eq(Expr::string("LicenseDlg1"))
                            .and(Expr::col("Type").eq(Expr::string("ScrollableText")))
                    )
                )
                .unwrap()
                .len()
                == 1
        );
        assert!(
            package
                .select_rows(
                    Select::table("ControlCondition").with(
                        Expr::col("Dialog_")
                            .eq(Expr::string("LicenseDlg1"))
                            .and(Expr::col("Control_").eq(Expr::string("Next")))
                    )
                )
                .unwrap()
                .len()
                == 2
        );
        assert_eq!(
            package
                .select_rows(
                    Select::table("CheckBox")
                        .with(Expr::col("Property").eq(Expr::string("EULA_ACCEPTED_1")))
                )
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            package
                .select_rows(
                    Select::table("Control").with(
                        Expr::col("Dialog_")
                            .eq(Expr::string("LicenseDlg2"))
                            .and(Expr::col("Control").eq(Expr::string("Accept")))
                    )
                )
                .unwrap()
                .len(),
            1,
            "optional EULAs must still show an acceptance checkbox"
        );
        assert_eq!(
            package
                .select_rows(
                    Select::table("CheckBox")
                        .with(Expr::col("Property").eq(Expr::string("EULA_ACCEPTED_2"))),
                )
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            package
                .select_rows(
                    Select::table("ControlCondition").with(
                        Expr::col("Dialog_")
                            .eq(Expr::string("LicenseDlg2"))
                            .and(Expr::col("Control_").eq(Expr::string("Next"))),
                    ),
                )
                .unwrap()
                .len(),
            0,
            "optional EULAs must not gate the Next button"
        );
        assert!(
            package
                .select_rows(
                    Select::table("Control").with(
                        Expr::col("Dialog_")
                            .eq(Expr::string("InstallDirDlg"))
                            .and(Expr::col("Control").eq(Expr::string("AddToPath")))
                            .and(Expr::col("Type").eq(Expr::string("CheckBox")))
                    )
                )
                .unwrap()
                .len()
                == 1
        );
        assert_eq!(
            package
                .select_rows(Select::table("Environment"))
                .unwrap()
                .len(),
            1
        );
        assert!(
            package
                .select_rows(
                    Select::table("Control").with(
                        Expr::col("Dialog_")
                            .eq(Expr::string("InstallDirDlg"))
                            .and(Expr::col("Type").eq(Expr::string("Edit")))
                            .and(Expr::col("Property").eq(Expr::string("INSTALLFOLDER"))),
                    ),
                )
                .unwrap()
                .len()
                == 1
        );
        assert!(
            package
                .select_rows(
                    Select::table("ControlEvent").with(
                        Expr::col("Dialog_")
                            .eq(Expr::string("InstallDirDlg"))
                            .and(Expr::col("Control_").eq(Expr::string("Next")))
                            .and(Expr::col("Event").eq(Expr::string("SetTargetPath")))
                            .and(Expr::col("Argument").eq(Expr::string("INSTALLFOLDER"))),
                    ),
                )
                .unwrap()
                .len()
                == 1
        );
        assert_eq!(
            package
                .select_rows(
                    Select::table("InstallExecuteSequence")
                        .with(Expr::col("Action").eq(Expr::string("RegisterUser")))
                )
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            package
                .select_rows(
                    Select::table("ControlEvent").with(
                        Expr::col("Dialog_")
                            .eq(Expr::string("ExitDlg"))
                            .and(Expr::col("Control_").eq(Expr::string("Finish")))
                            .and(Expr::col("Event").eq(Expr::string("EndDialog")))
                            .and(Expr::col("Argument").eq(Expr::string("Return"))),
                    ),
                )
                .unwrap()
                .len(),
            1
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn msi_package_contains_shortcuts_icons_and_associated_files() {
        let temp_dir =
            std::env::temp_dir().join(format!("cargo-crapapp-msi-full-{}", std::process::id()));
        let source = temp_dir.join("example.exe");
        let icon = temp_dir.join("example.ico");
        let output = temp_dir.join("example.msi");

        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        fs::write(&source, b"bin").expect("source should be written");
        fs::write(&icon, b"icon").expect("icon should be written");

        build(
            &test_plan(
                vec![
                    payload(
                        source.display().to_string(),
                        "$INSTALLPATH/example.exe",
                        true,
                    ),
                    payload(
                        icon.display().to_string(),
                        "$INSTALLPATH/example.ico",
                        false,
                    ),
                ],
                vec![
                    AssociatedFile {
                        path: "$INSTALLPATH/settings.json".to_owned(),
                        kind: AssociatedFileKind::File,
                        eula_report: false,
                    },
                    AssociatedFile {
                        path: "$HOMEPATH/Documents/Example App/saves".to_owned(),
                        kind: AssociatedFileKind::Directory,
                        eula_report: false,
                    },
                ],
                vec![Shortcut {
                    target: "$INSTALLPATH/example.exe".to_owned(),
                    name: "Example App".to_owned(),
                    directory: Some("Example App".to_owned()),
                    icon: Some("$INSTALLPATH/example.ico".to_owned()),
                }],
                Vec::new(),
                Some("$INSTALLPATH/example.ico".to_owned()),
                Some(icon.display().to_string()),
            ),
            &output,
            b"folder-picker-action",
            Some(icon.to_str().expect("icon path should be UTF-8")),
        )
        .expect("msi should be written");

        let mut package = msi::open(&output).expect("msi should open");
        assert_eq!(
            package
                .select_rows(Select::table("Shortcut"))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(package.select_rows(Select::table("Icon")).unwrap().len(), 2);
        assert!(package.has_stream("Icon.ProductIcon"));
        assert_eq!(
            package
                .select_rows(Select::table("CreateFolder"))
                .unwrap()
                .len(),
            1
        );
        assert!(
            package
                .select_rows(
                    Select::table("Property")
                        .with(Expr::col("Property").eq(Expr::string("ARPPRODUCTICON")))
                )
                .unwrap()
                .len()
                == 1
        );
        assert!(
            package
                .select_rows(
                    Select::table("RemoveRegistry")
                        .with(Expr::col("RemoveRegistry").eq(Expr::string("RemoveAppRegistryKey")))
                )
                .unwrap()
                .len()
                == 1
        );
        let component_rows = package
            .select_rows(
                Select::table("Component")
                    .with(Expr::col("Component").eq(Expr::string("ShortcutComponent1"))),
            )
            .unwrap();
        assert_eq!(component_rows.len(), 1);
        let component = component_rows.into_iter().next().unwrap();
        assert_eq!(
            component["Attributes"].as_int(),
            Some(super::COMPONENT_64BIT | super::COMPONENT_REGISTRY_KEYPATH)
        );
        assert_eq!(component["KeyPath"].as_str(), Some("ShortcutRegistry1"));

        let associated_component_rows = package
            .select_rows(
                Select::table("Component")
                    .with(Expr::col("Component").eq(Expr::string("AssociatedDirectory1"))),
            )
            .unwrap();
        assert_eq!(associated_component_rows.len(), 1);
        let associated_component = associated_component_rows.into_iter().next().unwrap();
        assert_eq!(
            associated_component["Attributes"].as_int(),
            Some(super::COMPONENT_64BIT | super::COMPONENT_REGISTRY_KEYPATH)
        );
        assert_eq!(
            associated_component["KeyPath"].as_str(),
            Some("RegistryAssociatedDirectory1")
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
