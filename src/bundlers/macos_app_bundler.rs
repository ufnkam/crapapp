use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, bail};
use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};

use crate::build_manifest::BuildManifest;
use crate::bundlers::MacosInstallerKind;
use crate::bundlers::shared;
use crate::payload_file::PayloadFile;
use crate::platform_manifests::MacosPlatformManifest;
use crate::target_manifest::TargetManifest;

const CONTENTS_DIR: &str = "Contents";
const MACOS_DIR: &str = "MacOS";

pub struct MacosAppBundler {}

impl MacosAppBundler {
    pub fn bundle(
        build_manifest: &BuildManifest,
        build_dir: &Path,
        platform_manifest: &MacosPlatformManifest,
        target_manifest: &TargetManifest,
        bundle: &MacosInstallerKind,
    ) -> anyhow::Result<()> {
        if target_manifest.files.is_empty() {
            bail!("macOS app bundle has no files to package");
        }

        let bundle_path = build_dir
            .join(&platform_manifest.platform)
            .join(&target_manifest.target)
            .join(bundle.to_string())
            .join(format!("{}.app", bundle_name(build_manifest)));

        Self::bundle_to_path(
            build_manifest,
            platform_manifest,
            target_manifest,
            &bundle_path,
        )
    }

    pub(crate) fn bundle_to_path(
        build_manifest: &BuildManifest,
        platform_manifest: &MacosPlatformManifest,
        target_manifest: &TargetManifest,
        bundle_path: &Path,
    ) -> anyhow::Result<()> {
        if bundle_path.exists() {
            fs::remove_dir_all(bundle_path)
                .with_context(|| format!("failed to remove {}", bundle_path.display()))?;
        }

        let contents_dir = bundle_path.join(CONTENTS_DIR);
        let macos_dir = contents_dir.join(MACOS_DIR);

        fs::create_dir_all(&macos_dir)
            .with_context(|| format!("failed to create {}", macos_dir.display()))?;

        for file in &target_manifest.files {
            let destination = bundle_destination(file, bundle_path)?;
            copy_payload_file(file, &destination)?;
        }

        if let Some(icon_source) = &platform_manifest.display_icon_source {
            let icon_file_name = shared::icon_file_name(icon_source)?;
            let destination = contents_dir.join("Resources").join(icon_file_name);
            let parent = destination.parent().ok_or_else(|| {
                anyhow::anyhow!(
                    "failed to resolve destination directory for {}",
                    destination.display()
                )
            })?;
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
            fs::copy(icon_source, &destination).with_context(|| {
                format!(
                    "failed to copy display icon {} to {}",
                    icon_source,
                    destination.display()
                )
            })?;
        }

        write_info_plist(
            build_manifest,
            platform_manifest,
            target_manifest,
            bundle_path,
        )?;
        let pkg_info_path = bundle_path.join(CONTENTS_DIR).join("PkgInfo");

        fs::write(&pkg_info_path, "APPL????")
            .with_context(|| format!("failed to write {}", pkg_info_path.display()))?;

        Ok(())
    }
}

pub(crate) fn bundle_name(build_manifest: &BuildManifest) -> String {
    if let Some(display_name) = &build_manifest.build.display_name {
        let display_name = display_name.trim();

        if !display_name.is_empty() {
            return display_name.to_owned();
        }
    }

    build_manifest.app_name.clone()
}

fn bundle_destination(file: &PayloadFile, bundle_path: &Path) -> anyhow::Result<PathBuf> {
    if file.executable {
        let file_name = executable_file_name(file)?;
        return Ok(bundle_path
            .join(CONTENTS_DIR)
            .join(MACOS_DIR)
            .join(file_name));
    }

    let relative_destination = validated_relative_path(&file.destination)?;
    Ok(bundle_path.join(CONTENTS_DIR).join(relative_destination))
}

fn executable_file_name(file: &PayloadFile) -> anyhow::Result<&OsStr> {
    Path::new(&file.source).file_name().ok_or_else(|| {
        anyhow::anyhow!(
            "executable payload source {} must point to a file",
            file.source
        )
    })
}

fn validated_relative_path(destination: &str) -> anyhow::Result<PathBuf> {
    let path = Path::new(destination);
    let mut relative_path = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Normal(part) => relative_path.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!("macOS bundle destination {destination} must be relative");
            }
        }
    }

    if relative_path.as_os_str().is_empty() {
        bail!("macOS bundle destination must not be empty");
    }

    Ok(relative_path)
}

fn copy_payload_file(file: &PayloadFile, destination: &Path) -> anyhow::Result<()> {
    let parent = destination.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "failed to resolve destination directory for {}",
            destination.display()
        )
    })?;

    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    fs::copy(&file.source, destination).with_context(|| {
        format!(
            "failed to copy {} to {}",
            file.source,
            destination.display()
        )
    })?;

    if file.executable {
        shared::set_executable_permissions(destination)?;
    }

    Ok(())
}

fn write_info_plist(
    build_manifest: &BuildManifest,
    platform_manifest: &MacosPlatformManifest,
    target_manifest: &TargetManifest,
    bundle_path: &Path,
) -> anyhow::Result<()> {
    let plist = info_plist(build_manifest, platform_manifest, target_manifest)?;
    let plist_path = bundle_path.join(CONTENTS_DIR).join("Info.plist");

    fs::write(&plist_path, plist)
        .with_context(|| format!("failed to write {}", plist_path.display()))?;

    Ok(())
}

fn info_plist(
    build_manifest: &BuildManifest,
    platform_manifest: &MacosPlatformManifest,
    target_manifest: &TargetManifest,
) -> anyhow::Result<String> {
    let executable = primary_executable_name(build_manifest, platform_manifest, target_manifest)?;
    let bundle_name = bundle_name(build_manifest);
    let identifier = bundle_identifier(build_manifest);

    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 4);
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    writer.write_event(Event::DocType(BytesText::new(
        r#"plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "https://www.apple.com/DTDs/PropertyList-1.0.dtd""#,
    )))?;

    let mut plist = BytesStart::new("plist");
    plist.push_attribute(("version", "1.0"));
    writer.write_event(Event::Start(plist))?;
    writer.write_event(Event::Start(BytesStart::new("dict")))?;

    write_plist_entry(&mut writer, "CFBundleDevelopmentRegion", "en")?;
    write_plist_entry(&mut writer, "CFBundleDisplayName", &bundle_name)?;
    write_plist_entry(&mut writer, "CFBundleExecutable", executable)?;
    write_plist_entry(&mut writer, "CFBundleIdentifier", &identifier)?;

    if let Some(icon_file) = info_plist_icon_file(platform_manifest) {
        write_plist_entry(&mut writer, "CFBundleIconFile", icon_file)?;
    }

    write_plist_entry(&mut writer, "CFBundleInfoDictionaryVersion", "6.0")?;
    write_plist_entry(&mut writer, "CFBundleName", &bundle_name)?;
    write_plist_entry(&mut writer, "CFBundlePackageType", "APPL")?;
    write_plist_entry(
        &mut writer,
        "CFBundleShortVersionString",
        &build_manifest.version,
    )?;
    write_plist_entry(&mut writer, "CFBundleVersion", &build_manifest.version)?;

    writer.write_event(Event::End(BytesEnd::new("dict")))?;
    writer.write_event(Event::End(BytesEnd::new("plist")))?;

    let mut plist = String::from_utf8(writer.into_inner()).context("Info.plist is not UTF-8")?;
    plist.push('\n');

    Ok(plist)
}

fn write_plist_entry(writer: &mut Writer<Vec<u8>>, key: &str, value: &str) -> anyhow::Result<()> {
    writer.write_event(Event::Start(BytesStart::new("key")))?;
    writer.write_event(Event::Text(BytesText::new(key)))?;
    writer.write_event(Event::End(BytesEnd::new("key")))?;
    writer.write_event(Event::Start(BytesStart::new("string")))?;
    writer.write_event(Event::Text(BytesText::new(value)))?;
    writer.write_event(Event::End(BytesEnd::new("string")))?;

    Ok(())
}

fn info_plist_icon_file(platform_manifest: &MacosPlatformManifest) -> Option<&str> {
    let display_icon = platform_manifest.display_icon.as_deref()?;
    let icon_path = Path::new(display_icon);

    if !shared::path_has_extension(icon_path, &["icns"]) {
        return None;
    }

    icon_path.file_name().and_then(OsStr::to_str)
}

fn primary_executable_name<'a>(
    _build_manifest: &BuildManifest,
    platform_manifest: &'a MacosPlatformManifest,
    target_manifest: &'a TargetManifest,
) -> anyhow::Result<&'a str> {
    let mut first_executable = None;

    for file in &target_manifest.files {
        if !file.executable {
            continue;
        }

        let Some(file_name) = Path::new(&file.source).file_name().and_then(OsStr::to_str) else {
            bail!(
                "executable payload source {} must point to a file",
                file.source
            );
        };

        if first_executable.is_none() {
            first_executable = Some(file_name);
        }

        if platform_manifest
            .app_binary
            .as_deref()
            .filter(|app_binary| !app_binary.trim().is_empty())
            .is_some_and(|app_binary| app_binary == file_name)
        {
            return Ok(file_name);
        }
    }

    if let Some(app_binary) = platform_manifest
        .app_binary
        .as_deref()
        .filter(|app_binary| !app_binary.trim().is_empty())
    {
        bail!("macOS app_binary references missing executable payload {app_binary}");
    }

    first_executable.ok_or_else(|| anyhow::anyhow!("macOS app bundle has no executable payload"))
}

pub(crate) fn bundle_identifier(build_manifest: &BuildManifest) -> String {
    let app_component = identifier_component(&build_manifest.app_name);

    match build_manifest.build.publisher.as_deref() {
        Some(publisher) => {
            let publisher_component = identifier_component(publisher);
            format!("com.{publisher_component}.{app_component}")
        }
        None => format!("com.crapapp.{app_component}"),
    }
}

fn identifier_component(value: &str) -> String {
    let mut component = String::new();
    let mut previous_was_separator = false;

    for character in value.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            component.push(character);
            previous_was_separator = false;
        } else if !previous_was_separator && !component.is_empty() {
            component.push('-');
            previous_was_separator = true;
        }
    }

    while component.ends_with('-') {
        component.pop();
    }

    if component.is_empty() {
        "app".to_owned()
    } else {
        component
    }
}

#[cfg(test)]
mod tests {
    use super::{MacosAppBundler, bundle_identifier, info_plist, validated_relative_path};
    use crate::build_config_manifest::BuildConfigManifest;
    use crate::build_manifest::BuildManifest;
    use crate::bundlers::MacosInstallerKind;
    use crate::payload_file::PayloadFile;
    use crate::platform_manifests::MacosPlatformManifest;
    use crate::target_manifest::TargetManifest;
    use std::fs;

    #[test]
    fn plist_falls_back_to_first_executable() {
        let build_manifest = build_manifest();
        let target_manifest = TargetManifest {
            target: "aarch64-apple-darwin".to_owned(),
            files: vec![
                PayloadFile::executable(
                    "target/aarch64-apple-darwin/release/helper".to_owned(),
                    "bin/helper".to_owned(),
                ),
                PayloadFile::executable(
                    "target/aarch64-apple-darwin/release/example".to_owned(),
                    "bin/example".to_owned(),
                ),
            ],
            shortcuts: Vec::new(),
        };

        let platform_manifest = platform_manifest(None, None);
        let plist = info_plist(&build_manifest, &platform_manifest, &target_manifest)
            .expect("plist should render");

        assert!(plist.contains("<string>helper</string>"));
    }

    #[test]
    fn plist_uses_configured_app_binary() {
        let build_manifest = build_manifest();
        let target_manifest = TargetManifest {
            target: "aarch64-apple-darwin".to_owned(),
            files: vec![
                PayloadFile::executable(
                    "target/aarch64-apple-darwin/release/example".to_owned(),
                    "bin/example".to_owned(),
                ),
                PayloadFile::executable(
                    "target/aarch64-apple-darwin/release/example-gui".to_owned(),
                    "bin/example-gui".to_owned(),
                ),
            ],
            shortcuts: Vec::new(),
        };

        let platform_manifest = platform_manifest_with_app_binary(None, None, Some("example-gui"));
        let plist = info_plist(&build_manifest, &platform_manifest, &target_manifest)
            .expect("plist should render");

        assert!(plist.contains("<key>CFBundleExecutable</key>"));
        assert!(plist.contains("<string>example-gui</string>"));
    }

    #[test]
    fn plist_escapes_xml_text() {
        let mut build_manifest = build_manifest();
        build_manifest.build.display_name = Some("Example & Tool".to_owned());
        build_manifest.version = "1.0.<dev>".to_owned();
        let target_manifest = TargetManifest {
            target: "aarch64-apple-darwin".to_owned(),
            files: vec![PayloadFile::executable(
                "target/aarch64-apple-darwin/release/example".to_owned(),
                "bin/example".to_owned(),
            )],
            shortcuts: Vec::new(),
        };

        let platform_manifest = platform_manifest(None, None);
        let plist = info_plist(&build_manifest, &platform_manifest, &target_manifest)
            .expect("plist should render");

        assert!(plist.contains("Example &amp; Tool"));
        assert!(plist.contains("1.0.&lt;dev&gt;"));
    }

    #[test]
    fn bundle_identifier_uses_publisher_when_available() {
        let build_manifest = build_manifest();

        assert_eq!(bundle_identifier(&build_manifest), "com.ufnkam.example");
    }

    #[test]
    fn relative_destination_rejects_parent_traversal() {
        assert!(validated_relative_path("../outside").is_err());
        assert!(validated_relative_path("assets/../outside").is_err());
    }

    #[test]
    fn app_bundle_contains_macos_layout() {
        let temp_dir = std::env::temp_dir().join(format!(
            "cargo-crapapp-macos-app-bundle-{}",
            std::process::id()
        ));
        let source_dir = temp_dir.join("source");
        let executable = source_dir.join("example");
        let data_file = source_dir.join("settings.toml");

        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&source_dir).expect("source dir should be created");
        fs::write(&executable, b"bin").expect("executable should be written");
        fs::write(&data_file, b"settings").expect("data file should be written");

        let build_manifest = build_manifest();
        let platform_manifest = platform_manifest(None, None);
        let target_manifest = TargetManifest {
            target: "aarch64-apple-darwin".to_owned(),
            files: vec![
                PayloadFile::executable(executable.display().to_string(), "bin/example".to_owned()),
                PayloadFile::data(
                    data_file.display().to_string(),
                    "Resources/etc/example/settings.toml".to_owned(),
                ),
            ],
            shortcuts: Vec::new(),
        };

        MacosAppBundler::bundle(
            &build_manifest,
            &temp_dir.join("build"),
            &platform_manifest,
            &target_manifest,
            &MacosInstallerKind::App,
        )
        .expect("app bundle should be created");

        let app_path = temp_dir
            .join("build")
            .join("macos")
            .join("aarch64-apple-darwin")
            .join("app")
            .join("example.app");

        assert!(app_path.join("Contents/Info.plist").is_file());
        assert!(app_path.join("Contents/PkgInfo").is_file());
        assert!(app_path.join("Contents/MacOS/example").is_file());
        assert!(
            app_path
                .join("Contents/Resources/etc/example/settings.toml")
                .is_file()
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn app_bundle_copies_icns_display_icon() {
        let temp_dir = std::env::temp_dir().join(format!(
            "cargo-crapapp-macos-app-icon-{}",
            std::process::id()
        ));
        let source_dir = temp_dir.join("source");
        let executable = source_dir.join("example");
        let icon = source_dir.join("App Icon.icns");

        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&source_dir).expect("source dir should be created");
        fs::write(&executable, b"bin").expect("executable should be written");
        fs::write(&icon, b"icns").expect("icon should be written");

        let build_manifest = build_manifest();
        let icon_path = icon.display().to_string();
        let platform_manifest = platform_manifest(Some(&icon_path), Some(&icon_path));
        let target_manifest = TargetManifest {
            target: "aarch64-apple-darwin".to_owned(),
            files: vec![PayloadFile::executable(
                executable.display().to_string(),
                "bin/example".to_owned(),
            )],
            shortcuts: Vec::new(),
        };

        MacosAppBundler::bundle(
            &build_manifest,
            &temp_dir.join("build"),
            &platform_manifest,
            &target_manifest,
            &MacosInstallerKind::App,
        )
        .expect("app bundle should be created");

        let app_path = temp_dir
            .join("build")
            .join("macos")
            .join("aarch64-apple-darwin")
            .join("app")
            .join("example.app");
        let plist = fs::read_to_string(app_path.join("Contents/Info.plist"))
            .expect("plist should be readable");

        assert!(app_path.join("Contents/Resources/App Icon.icns").is_file());
        assert!(plist.contains("<key>CFBundleIconFile</key>"));
        assert!(plist.contains("<string>App Icon.icns</string>"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    fn build_manifest() -> BuildManifest {
        BuildManifest {
            app_name: "example".to_owned(),
            version: "1.0.0".to_owned(),
            build: BuildConfigManifest {
                publisher: Some("ufnkam".to_owned()),
                display_name: None,
                packages: Vec::new(),
                features: Vec::new(),
            },
            platforms: Vec::new(),
        }
    }

    fn platform_manifest(
        display_icon: Option<&str>,
        display_icon_source: Option<&str>,
    ) -> MacosPlatformManifest {
        platform_manifest_with_app_binary(display_icon, display_icon_source, None)
    }

    fn platform_manifest_with_app_binary(
        display_icon: Option<&str>,
        display_icon_source: Option<&str>,
        app_binary: Option<&str>,
    ) -> MacosPlatformManifest {
        MacosPlatformManifest::new(
            Vec::new(),
            display_icon,
            display_icon_source,
            app_binary,
            vec![MacosInstallerKind::App],
            Default::default(),
        )
    }
}
