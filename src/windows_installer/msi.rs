use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Read, Seek, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use msi::{Column, Insert, Package, PackageType, Value};
use uuid::Uuid;

use crate::manifest_file::{AssociatedFile, AssociatedFileKind};
use crate::payload_file::PayloadFile;
use crate::target_manifest::Shortcut;

const DEFAULT_MANUFACTURER: &str = "unknown";
const COMPONENT_64BIT: i32 = 256;
const HKCU: i32 = 1;

pub struct MsiSpec {
    pub package: String,
    pub display_name: String,
    pub version: String,
    pub manufacturer: String,
    pub files: Vec<PayloadFile>,
    pub associated_files: Vec<AssociatedFile>,
    pub shortcuts: Vec<Shortcut>,
    pub display_icon: Option<String>,
    pub display_icon_source: Option<String>,
}

pub fn build(spec: &MsiSpec, output: &Path) -> anyhow::Result<()> {
    validate(spec)?;

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
    package
        .summary_info_mut()
        .set_subject(spec.display_name.clone());
    package
        .summary_info_mut()
        .set_author(spec.manufacturer.clone());
    package.summary_info_mut().set_arch("x64");
    package
        .summary_info_mut()
        .set_creating_application("cargo-crapapp");
    package.summary_info_mut().set_uuid(package_code(spec));

    let files = msi_files(spec)?;
    let associated_dirs = associated_directories(spec)?;
    let shortcut_rows = shortcuts(spec, &files)?;
    let icons = icons(spec, &files, &shortcut_rows)?;
    let cabinet_stream = cabinet_stream_name(&spec.package);
    create_schema(&mut package)?;
    insert_rows(
        &mut package,
        spec,
        &cabinet_stream,
        &files,
        &associated_dirs,
        &shortcut_rows,
        &icons,
    )?;

    let cab = cabinet(&files)?;
    package
        .write_stream(&cabinet_stream)
        .context("failed to create MSI cabinet stream")?
        .write_all(&cab)
        .context("failed to write MSI cabinet stream")?;

    package
        .into_inner()
        .context("failed to flush MSI database")?;

    Ok(())
}

fn validate(spec: &MsiSpec) -> anyhow::Result<()> {
    if spec.files.is_empty() {
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
struct MsiFile {
    id: String,
    component: String,
    payload: MsiFilePayload,
    cabinet_name: String,
    directory: String,
    directory_root: String,
    directory_path: PathBuf,
    install_path: PathBuf,
    file_name: String,
    size: i32,
}

#[derive(Clone, Debug)]
enum MsiFilePayload {
    Source(PathBuf),
    Bytes(Vec<u8>),
}

impl MsiFilePayload {
    fn len(&self) -> anyhow::Result<u64> {
        match self {
            Self::Source(path) => fs::metadata(path)
                .with_context(|| format!("failed to read metadata for {}", path.display()))
                .map(|metadata| metadata.len()),
            Self::Bytes(bytes) => Ok(bytes.len() as u64),
        }
    }
}

#[derive(Clone, Debug)]
struct MsiDirectory {
    id: String,
    component: String,
    registry: String,
    root: String,
    path: PathBuf,
}

#[derive(Clone, Debug)]
struct MsiShortcut {
    id: String,
    directory: String,
    directory_name: String,
    name: String,
    component: String,
    target_file: String,
    icon: Option<String>,
}

#[derive(Clone, Debug)]
struct MsiIcon {
    id: String,
    source: PathBuf,
}

fn msi_files(spec: &MsiSpec) -> anyhow::Result<Vec<MsiFile>> {
    let mut directories = DirectoryIds::default();
    let mut output = Vec::new();

    for (index, file) in spec.files.iter().enumerate() {
        let relative = install_relative_path(&file.destination)?;
        let file_name = relative
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "MSI destination {} must have a UTF-8 file name",
                    file.destination
                )
            })?
            .to_owned();
        let parent = relative.parent().unwrap_or_else(|| Path::new(""));
        let directory = directories.id_for("INSTALLFOLDER", parent)?;
        let source = PathBuf::from(&file.source);
        let payload = MsiFilePayload::Source(source);
        let size = payload.len()?;

        if size > i32::MAX as u64 {
            bail!("MSI payload file {} is too large", file.source);
        }

        output.push(MsiFile {
            id: format!("File{}", index + 1),
            component: format!("Component{}", index + 1),
            payload,
            cabinet_name: format!("File{}", index + 1),
            directory,
            directory_root: "INSTALLFOLDER".to_owned(),
            directory_path: parent.to_path_buf(),
            install_path: relative,
            file_name,
            size: size as i32,
        });
    }

    for file in spec
        .associated_files
        .iter()
        .filter(|file| matches!(file.kind, AssociatedFileKind::File))
    {
        let associated = associated_path(&file.path)?;
        let file_name = associated
            .relative
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "MSI associated file {} must have a UTF-8 file name",
                    file.path
                )
            })?
            .to_owned();
        let parent = associated
            .relative
            .parent()
            .unwrap_or_else(|| Path::new(""));
        let directory = directories.id_for(&associated.root, parent)?;
        let index = output.len() + 1;
        output.push(MsiFile {
            id: format!("File{index}"),
            component: format!("Component{index}"),
            payload: MsiFilePayload::Bytes(Vec::new()),
            cabinet_name: format!("File{index}"),
            directory,
            directory_root: associated.root,
            directory_path: parent.to_path_buf(),
            install_path: associated.relative,
            file_name,
            size: 0,
        });
    }

    Ok(output)
}

fn associated_directories(spec: &MsiSpec) -> anyhow::Result<Vec<MsiDirectory>> {
    let mut directories = DirectoryIds::default();
    let mut output = Vec::new();

    for file in spec
        .associated_files
        .iter()
        .filter(|file| matches!(file.kind, AssociatedFileKind::Directory))
    {
        let associated = associated_path(&file.path)?;
        let id = directories.id_for(&associated.root, &associated.relative)?;
        let index = output.len() + 1;
        output.push(MsiDirectory {
            id,
            component: format!("AssociatedDirectory{index}"),
            registry: format!("RegistryAssociatedDirectory{index}"),
            root: associated.root,
            path: associated.relative,
        });
    }

    Ok(output)
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
            Column::build("Directory_").nullable().id_string(72),
            Column::build("DefaultDir").string(255),
        ],
    )?;
    create_table(
        package,
        "Feature",
        vec![
            Column::build("Feature").primary_key().id_string(38),
            Column::build("Feature_").nullable().id_string(38),
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
        "InstallExecuteSequence",
        vec![
            Column::build("Action").primary_key().id_string(72),
            Column::build("Condition").nullable().formatted_string(255),
            Column::build("Sequence").nullable().int16(),
        ],
    )?;

    Ok(())
}

fn create_table<W: Write + Seek>(
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

fn insert_rows<W: Read + Write + Seek>(
    package: &mut Package<W>,
    spec: &MsiSpec,
    cabinet_stream: &str,
    files: &[MsiFile],
    associated_dirs: &[MsiDirectory],
    shortcuts: &[MsiShortcut],
    icons: &[MsiIcon],
) -> anyhow::Result<()> {
    let manufacturer = if spec.manufacturer.trim().is_empty() {
        DEFAULT_MANUFACTURER
    } else {
        &spec.manufacturer
    };

    let mut property_rows = vec![
        row(["ProductCode", &guid(&product_code(spec))]),
        row(["ProductName", &spec.display_name]),
        row(["ProductVersion", &msi_version(&spec.version)]),
        row(["ProductLanguage", "1033"]),
        row(["Manufacturer", manufacturer]),
        row(["UpgradeCode", &guid(&upgrade_code(spec))]),
        row(["MSIINSTALLPERUSER", "1"]),
        row(["ARPNOREPAIR", "1"]),
        row(["ARPNOMODIFY", "1"]),
    ];
    if let Some(product_icon) = product_icon(icons) {
        property_rows.push(row(["ARPPRODUCTICON", product_icon]));
    }
    insert(package, "Property", property_rows)?;
    insert(
        package,
        "Directory",
        directory_rows(spec, files, associated_dirs, shortcuts)?,
    )?;
    insert(
        package,
        "Feature",
        vec![vec![
            Value::from("DefaultFeature"),
            Value::Null,
            Value::from(spec.display_name.as_str()),
            Value::from(spec.display_name.as_str()),
            Value::Int(1),
            Value::Int(1),
            Value::from("INSTALLFOLDER"),
            Value::Int(0),
        ]],
    )?;

    let component_rows = files
        .iter()
        .map(|file| {
            vec![
                Value::from(file.component.as_str()),
                Value::from(guid(&component_code(spec, file)).as_str()),
                Value::from(file.directory.as_str()),
                Value::Int(COMPONENT_64BIT),
                Value::Null,
                Value::from(file.id.as_str()),
            ]
        })
        .chain(associated_dirs.iter().map(|directory| {
            vec![
                Value::from(directory.component.as_str()),
                Value::from(guid(&directory_component_code(spec, directory)).as_str()),
                Value::from(directory.id.as_str()),
                Value::Int(COMPONENT_64BIT),
                Value::Null,
                Value::from(directory.registry.as_str()),
            ]
        }))
        .collect();
    insert(package, "Component", component_rows)?;

    let feature_component_rows = files
        .iter()
        .map(|file| {
            vec![
                Value::from("DefaultFeature"),
                Value::from(file.component.as_str()),
            ]
        })
        .chain(associated_dirs.iter().map(|directory| {
            vec![
                Value::from("DefaultFeature"),
                Value::from(directory.component.as_str()),
            ]
        }))
        .collect();
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
    if !associated_dirs.is_empty() {
        insert(
            package,
            "CreateFolder",
            associated_dirs
                .iter()
                .map(|directory| {
                    vec![
                        Value::from(directory.id.as_str()),
                        Value::from(directory.component.as_str()),
                    ]
                })
                .collect(),
        )?;
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
                        Value::from(shortcut.name.as_str()),
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
                .map(|icon| vec![Value::from(icon.id.as_str()), Value::Binary])
                .collect(),
        )?;
        for icon in icons {
            package
                .write_stream(&icon.id)
                .with_context(|| format!("failed to create MSI icon stream {}", icon.id))?
                .write_all(
                    &fs::read(&icon.source).with_context(|| {
                        format!("failed to read icon {}", icon.source.display())
                    })?,
                )
                .with_context(|| format!("failed to write MSI icon stream {}", icon.id))?;
        }
    }
    let registry_rows = registry_rows(spec, files, associated_dirs)?;
    if !registry_rows.is_empty() {
        insert(package, "Registry", registry_rows)?;
    }
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
        install_execute_sequence(),
    )?;

    Ok(())
}

fn directory_rows(
    spec: &MsiSpec,
    files: &[MsiFile],
    associated_dirs: &[MsiDirectory],
    shortcuts: &[MsiShortcut],
) -> anyhow::Result<Vec<Vec<Value>>> {
    let mut parents = BTreeMap::<String, (String, String)>::new();
    parents.insert(
        "TARGETDIR".to_owned(),
        (String::new(), "SourceDir".to_owned()),
    );
    parents.insert(
        "LocalAppDataFolder".to_owned(),
        ("TARGETDIR".to_owned(), ".".to_owned()),
    );
    parents.insert(
        "INSTALLFOLDER".to_owned(),
        (
            "LocalAppDataFolder".to_owned(),
            default_dir(&spec.display_name),
        ),
    );
    parents.insert(
        "UserProfileFolder".to_owned(),
        ("TARGETDIR".to_owned(), ".".to_owned()),
    );
    parents.insert(
        "PersonalFolder".to_owned(),
        ("TARGETDIR".to_owned(), ".".to_owned()),
    );
    parents.insert(
        "ProgramMenuFolder".to_owned(),
        ("TARGETDIR".to_owned(), ".".to_owned()),
    );

    let mut directory_ids = DirectoryIds::default();
    for file in files {
        directory_ids.id_for(&file.directory_root, &file.directory_path)?;
    }
    for directory in associated_dirs {
        directory_ids.id_for(&directory.root, &directory.path)?;
    }

    let directory_map = directory_ids.ids;
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
    name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "-")
}

fn install_execute_sequence() -> Vec<Vec<Value>> {
    [
        ("CostInitialize", 800),
        ("FileCost", 900),
        ("CostFinalize", 1000),
        ("InstallValidate", 1400),
        ("InstallInitialize", 1500),
        ("ProcessComponents", 1600),
        ("RemoveRegistryValues", 2600),
        ("RemoveShortcuts", 3200),
        ("UnpublishFeatures", 1800),
        ("RemoveFiles", 3500),
        ("CreateFolders", 3700),
        ("InstallFiles", 4000),
        ("CreateShortcuts", 4500),
        ("WriteRegistryValues", 5000),
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

fn shortcuts(spec: &MsiSpec, files: &[MsiFile]) -> anyhow::Result<Vec<MsiShortcut>> {
    let mut output = Vec::new();

    for (index, shortcut) in spec.shortcuts.iter().enumerate() {
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
            .unwrap_or_else(|| spec.display_name.clone());
        let icon = shortcut
            .icon
            .as_deref()
            .map(|icon| shortcut_icon_id(icon, files))
            .transpose()?;

        output.push(MsiShortcut {
            id: format!("Shortcut{}", index + 1),
            directory,
            directory_name,
            name: default_dir(&shortcut.name),
            component: file.component.clone(),
            target_file: file.id.clone(),
            icon,
        });
    }

    Ok(output)
}

fn icons(
    spec: &MsiSpec,
    files: &[MsiFile],
    shortcuts: &[MsiShortcut],
) -> anyhow::Result<Vec<MsiIcon>> {
    let mut output = Vec::new();

    if let Some(source) = spec.display_icon_source.as_deref() {
        output.push(MsiIcon {
            id: "ProductIcon".to_owned(),
            source: PathBuf::from(source),
        });
    } else if let Some(display_icon) = spec.display_icon.as_deref() {
        if let Some(source) = installed_file_source(display_icon, files) {
            output.push(MsiIcon {
                id: "ProductIcon".to_owned(),
                source,
            });
        }
    }

    for shortcut in shortcuts
        .iter()
        .filter_map(|shortcut| shortcut.icon.as_deref())
    {
        if output.iter().any(|icon| icon.id == shortcut) {
            continue;
        }
        let source = files
            .iter()
            .find(|file| shortcut == shortcut_icon_id_from_file(file))
            .and_then(|file| match &file.payload {
                MsiFilePayload::Source(path) => Some(path.clone()),
                MsiFilePayload::Bytes(_) => None,
            })
            .ok_or_else(|| {
                anyhow::anyhow!("MSI shortcut icon {shortcut} is not backed by a payload file")
            })?;
        output.push(MsiIcon {
            id: shortcut.to_owned(),
            source,
        });
    }

    Ok(output)
}

fn product_icon(icons: &[MsiIcon]) -> Option<&str> {
    icons.first().map(|icon| icon.id.as_str())
}

fn shortcut_icon_id(icon: &str, files: &[MsiFile]) -> anyhow::Result<String> {
    let icon_path = install_relative_path(icon)?;
    files
        .iter()
        .find(|file| file.directory_root == "INSTALLFOLDER" && file.install_path == icon_path)
        .map(shortcut_icon_id_from_file)
        .ok_or_else(|| anyhow::anyhow!("MSI shortcut icon {icon} is not in payload"))
}

fn shortcut_icon_id_from_file(file: &MsiFile) -> String {
    format!("Icon{}", identifier(&file.id))
}

fn installed_file_source(destination: &str, files: &[MsiFile]) -> Option<PathBuf> {
    let path = install_relative_path(destination).ok()?;
    files
        .iter()
        .find(|file| file.directory_root == "INSTALLFOLDER" && file.install_path == path)
        .and_then(|file| match &file.payload {
            MsiFilePayload::Source(path) => Some(path.clone()),
            MsiFilePayload::Bytes(_) => None,
        })
}

fn registry_rows(
    spec: &MsiSpec,
    _files: &[MsiFile],
    associated_dirs: &[MsiDirectory],
) -> anyhow::Result<Vec<Vec<Value>>> {
    let mut rows = Vec::new();
    for directory in associated_dirs {
        rows.push(vec![
            Value::from(directory.registry.as_str()),
            Value::Int(HKCU),
            Value::from(format!("Software\\cargo-crapapp\\{}", spec.package).as_str()),
            Value::from(directory.registry.as_str()),
            Value::from(format!("[{}]", directory.id).as_str()),
            Value::from(directory.component.as_str()),
        ]);
    }

    Ok(rows)
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

fn cabinet(files: &[MsiFile]) -> anyhow::Result<Vec<u8>> {
    let mut builder = cab::CabinetBuilder::new();
    {
        let folder = builder.add_folder(cab::CompressionType::MsZip);
        for file in files {
            folder.add_file(file.cabinet_name.clone());
        }
    }

    let cursor = Cursor::new(Vec::new());
    let mut writer = builder
        .build(cursor)
        .context("failed to create MSI cabinet")?;
    while let Some(mut file_writer) = writer
        .next_file()
        .context("failed to create MSI cabinet file")?
    {
        let source = files
            .iter()
            .find(|file| file.cabinet_name == file_writer.file_name())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "failed to resolve MSI cabinet source {}",
                    file_writer.file_name()
                )
            })?;
        match &source.payload {
            MsiFilePayload::Source(path) => {
                let mut input = fs::File::open(path)
                    .with_context(|| format!("failed to read {}", path.display()))?;
                std::io::copy(&mut input, &mut file_writer).with_context(|| {
                    format!(
                        "failed to write MSI cabinet file {}",
                        file_writer.file_name()
                    )
                })?;
            }
            MsiFilePayload::Bytes(bytes) => {
                file_writer.write_all(bytes).with_context(|| {
                    format!(
                        "failed to write MSI cabinet file {}",
                        file_writer.file_name()
                    )
                })?;
            }
        }
    }

    Ok(writer.finish()?.into_inner())
}

fn package_code(spec: &MsiSpec) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!(
            "cargo-crapapp:msi:package:{}:{}",
            spec.package, spec.version
        )
        .as_bytes(),
    )
}

fn product_code(spec: &MsiSpec) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!(
            "cargo-crapapp:msi:product:{}:{}",
            spec.package, spec.version
        )
        .as_bytes(),
    )
}

fn upgrade_code(spec: &MsiSpec) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("cargo-crapapp:msi:upgrade:{}", spec.package).as_bytes(),
    )
}

fn component_code(spec: &MsiSpec, file: &MsiFile) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!(
            "cargo-crapapp:msi:component:{}:{}:{}",
            spec.package, spec.version, file.id
        )
        .as_bytes(),
    )
}

fn directory_component_code(spec: &MsiSpec, directory: &MsiDirectory) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!(
            "cargo-crapapp:msi:directory-component:{}:{}:{}",
            spec.package, spec.version, directory.id
        )
        .as_bytes(),
    )
}

fn guid(uuid: &Uuid) -> String {
    uuid.braced().to_string().to_ascii_uppercase()
}

fn msi_version(version: &str) -> String {
    let mut parts = version.split('.').take(3).collect::<Vec<_>>();
    while parts.len() < 3 {
        parts.push("0");
    }
    parts.join(".")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use msi::{Expr, Select};

    use crate::manifest_file::{AssociatedFile, AssociatedFileKind};
    use crate::payload_file::PayloadFile;
    use crate::target_manifest::Shortcut;
    use crate::windows_installer::msi::{MsiSpec, build};

    #[test]
    fn msi_package_opens_as_installer_database() {
        let temp_dir =
            std::env::temp_dir().join(format!("cargo-crapapp-msi-{}", std::process::id()));
        let source = temp_dir.join("example.exe");
        let output = temp_dir.join("example.msi");

        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        fs::write(&source, b"bin").expect("source should be written");

        build(
            &MsiSpec {
                package: "example".to_owned(),
                display_name: "Example App".to_owned(),
                version: "1.2.3".to_owned(),
                manufacturer: "ufnkam".to_owned(),
                files: vec![PayloadFile::executable(
                    source.display().to_string(),
                    "$INSTALLPATH/example.exe".to_owned(),
                )],
                associated_files: Vec::new(),
                shortcuts: Vec::new(),
                display_icon: None,
                display_icon_source: None,
            },
            &output,
        )
        .expect("msi should be written");

        let package = msi::open(&output).expect("msi should open");
        assert!(package.has_table("Product") || package.has_table("Property"));
        assert!(package.has_table("Shortcut"));
        assert!(package.has_table("Icon"));
        assert!(package.has_table("CreateFolder"));
        assert!(package.has_stream("example.cab"));

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
            &MsiSpec {
                package: "example".to_owned(),
                display_name: "Example App".to_owned(),
                version: "1.2.3".to_owned(),
                manufacturer: "ufnkam".to_owned(),
                files: vec![
                    PayloadFile::executable(
                        source.display().to_string(),
                        "$INSTALLPATH/example.exe".to_owned(),
                    ),
                    PayloadFile::data(
                        icon.display().to_string(),
                        "$INSTALLPATH/example.ico".to_owned(),
                    ),
                ],
                associated_files: vec![
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
                shortcuts: vec![Shortcut {
                    target: "$INSTALLPATH/example.exe".to_owned(),
                    name: "Example App".to_owned(),
                    directory: Some("Example App".to_owned()),
                    icon: Some("$INSTALLPATH/example.ico".to_owned()),
                }],
                display_icon: Some("$INSTALLPATH/example.ico".to_owned()),
                display_icon_source: Some(icon.display().to_string()),
            },
            &output,
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

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
