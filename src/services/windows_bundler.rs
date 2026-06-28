use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::services::build_manifest::{BuildManifest, PlatformManifest};
use crate::services::build_variable::BuildVariable;
use crate::services::payload_file::PayloadFile;

const SETUP_CARGO_TOML: &str =
    include_str!("../../embedded-templates/windows-installer-cli/Cargo.toml.template");
const SETUP_BUILD_RS: &str =
    include_str!("../../embedded-templates/windows-installer-cli/build.rs");
const SETUP_RS: &str = include_str!("../../embedded-templates/windows-installer-cli/src/setup.rs");
const UNINSTALL_RS: &str =
    include_str!("../../embedded-templates/windows-installer-cli/src/uninstall.rs");
const GENERATED_RS: &str =
    include_str!("../../embedded-templates/windows-installer-cli/src/generated.rs");
const CORE_CARGO_TOML: &str =
    include_str!("../../embedded-templates/windows-installer-core/Cargo.toml.template");
const CORE_RS: &str = include_str!("../../embedded-templates/windows-installer-core/src/lib.rs");
const CORE_CONFIG_RS: &str =
    include_str!("../../embedded-templates/windows-installer-core/src/config.rs");
const CORE_CLI_RS: &str =
    include_str!("../../embedded-templates/windows-installer-core/src/cli.rs");
const CORE_INSTALL_RS: &str =
    include_str!("../../embedded-templates/windows-installer-core/src/install.rs");
const CORE_REGISTRY_RS: &str =
    include_str!("../../embedded-templates/windows-installer-core/src/registry.rs");
const CORE_UNINSTALL_RS: &str =
    include_str!("../../embedded-templates/windows-installer-core/src/uninstall.rs");
const INSTALL_ICON: &[u8] = include_bytes!("../../embedded-templates/assets/install.ico");
const GENERATED_WORKSPACE_PLACEHOLDER: &str = "# crapapp_template_generated_workspace!()";
const GENERATED_CORE_DEPENDENCY_PLACEHOLDER: &str =
    r#"crapapp-windows-installer-core = { path = "../windows-installer-core" }"#;
const GENERATED_CORE_DEPENDENCY: &str =
    r#"crapapp-windows-installer-core = { path = "windows-installer-core" }"#;
const PAYLOAD_SOURCE_PLACEHOLDER: &str = "crapapp_template_payload_source!()";
const APP_NAME_PLACEHOLDER: &str = "crapapp_template_app_name!()";
const APP_VERSION_PLACEHOLDER: &str = "crapapp_template_app_version!()";
const APP_PUBLISHER_PLACEHOLDER: &str = "crapapp_template_app_publisher!()";
const APP_DISPLAY_ICON_PLACEHOLDER: &str = "crapapp_template_app_display_icon!()";
const REQUIRED_VARIABLES_PLACEHOLDER: &str = "// crapapp_template_required_variables!()";
const UNINSTALLER_SOURCE_PLACEHOLDER: &str = "crapapp_template_uninstaller_bytes!()";
const UNINSTALL_ENTRIES_PLACEHOLDER: &str = "// crapapp_template_uninstall_entries!()";
const PATH_ENTRIES_PLACEHOLDER: &str = "// crapapp_template_path_entries!()";

pub struct WindowsBundler<'a> {
    build_manifest: &'a BuildManifest,
    build_dir: &'a Path,
}

impl<'a> WindowsBundler<'a> {
    pub fn new(build_manifest: &'a BuildManifest, build_dir: &'a Path) -> Self {
        Self {
            build_manifest,
            build_dir,
        }
    }

    pub fn bundle(&self) -> Result<()> {
        let windows = self.windows_platform()?;

        for target in &windows.targets {
            let output_dir = self.build_dir.join("windows").join(&target.target);
            let setup_source_dir = output_dir.join("setup-src");

            remove_dir_if_exists(&setup_source_dir)?;
            fs::create_dir_all(setup_source_dir.join("src")).with_context(|| {
                format!(
                    "failed to create setup project at {}",
                    setup_source_dir.display()
                )
            })?;

            self.write_setup_project(windows, &target.files, &setup_source_dir)?;
            self.build_uninstaller(&target.target, &setup_source_dir)?;
            self.write_setup_rs_with_uninstaller(windows, &target.target, &setup_source_dir)?;
            self.build_setup(&target.target, &setup_source_dir)?;
            self.copy_setup_output(&target.target, &setup_source_dir, &output_dir)?;
            self.clean_setup_source(&setup_source_dir)?;
        }

        Ok(())
    }

    fn windows_platform(&self) -> Result<&PlatformManifest> {
        self.build_manifest
            .platforms
            .iter()
            .find(|platform| platform.platform == "windows")
            .context("windows platform is not configured")
    }

    fn write_setup_project(
        &self,
        platform: &PlatformManifest,
        files: &[PayloadFile],
        setup_source_dir: &Path,
    ) -> Result<()> {
        if files.is_empty() {
            bail!("windows bundle has no files to package");
        }

        fs::write(setup_source_dir.join("Cargo.toml"), setup_cargo_toml())
            .with_context(|| "failed to write setup Cargo.toml")?;
        fs::write(setup_source_dir.join("build.rs"), setup_build_rs(files)?)
            .with_context(|| "failed to write setup build.rs")?;
        self.write_core_project(setup_source_dir)?;
        self.write_generated_rs(platform, files, "", setup_source_dir)?;
        write_setup_assets(setup_source_dir)?;
        fs::write(setup_source_dir.join("src").join("setup.rs"), SETUP_RS)
            .with_context(|| "failed to write setup.rs")?;
        fs::write(
            setup_source_dir.join("src").join("uninstall.rs"),
            UNINSTALL_RS,
        )
        .with_context(|| "failed to write uninstall.rs")?;

        Ok(())
    }

    fn build_uninstaller(&self, target: &str, setup_source_dir: &Path) -> Result<()> {
        remove_dir_if_exists(&setup_source_dir.join("target"))?;

        let status = Command::new("cargo")
            .arg("build")
            .arg("--release")
            .arg("--target")
            .arg(target)
            .arg("--bin")
            .arg("uninstall")
            .current_dir(setup_source_dir)
            .status()
            .with_context(|| format!("failed to build uninstall.exe for {target}"))?;

        if !status.success() {
            bail!("uninstall.exe build failed for {target}");
        }

        Ok(())
    }

    fn write_setup_rs_with_uninstaller(
        &self,
        platform: &PlatformManifest,
        target: &str,
        setup_source_dir: &Path,
    ) -> Result<()> {
        let uninstaller = fs::canonicalize(release_exe_path(target, setup_source_dir, "uninstall"))
            .with_context(|| format!("failed to find built uninstall.exe for {target}"))?;
        let embedded_uninstaller = setup_source_dir.join("assets").join("uninstall.exe");
        fs::copy(&uninstaller, &embedded_uninstaller).with_context(|| {
            format!(
                "failed to copy {} to {}",
                uninstaller.display(),
                embedded_uninstaller.display()
            )
        })?;
        let embedded_uninstaller = fs::canonicalize(&embedded_uninstaller).with_context(|| {
            format!(
                "failed to find copied uninstall.exe at {}",
                embedded_uninstaller.display()
            )
        })?;

        remove_dir_if_exists(&setup_source_dir.join("target"))?;

        let target_manifest = platform
            .targets
            .iter()
            .find(|target_manifest| target_manifest.target == target)
            .context("failed to find target manifest")?;

        fs::write(
            setup_source_dir.join("build.rs"),
            setup_build_rs(&target_manifest.files)?,
        )
        .with_context(|| "failed to write setup build.rs with embedded uninstaller payload")?;

        self.write_core_project(setup_source_dir)?;
        self.write_generated_rs(
            platform,
            &target_manifest.files,
            &embedded_uninstaller.display().to_string(),
            setup_source_dir,
        )?;

        fs::write(setup_source_dir.join("src").join("setup.rs"), SETUP_RS)
            .with_context(|| "failed to write setup.rs with embedded uninstaller")?;

        Ok(())
    }

    fn write_core_project(&self, setup_source_dir: &Path) -> Result<()> {
        let core_dir = setup_source_dir.join("windows-installer-core");
        let core_src_dir = core_dir.join("src");
        fs::create_dir_all(&core_src_dir)
            .with_context(|| format!("failed to create {}", core_src_dir.display()))?;
        fs::write(core_dir.join("Cargo.toml"), CORE_CARGO_TOML)
            .with_context(|| "failed to write installer core Cargo.toml")?;
        fs::write(core_src_dir.join("lib.rs"), CORE_RS)
            .with_context(|| "failed to write installer core lib.rs")?;
        fs::write(core_src_dir.join("config.rs"), CORE_CONFIG_RS)
            .with_context(|| "failed to write installer core config.rs")?;
        fs::write(core_src_dir.join("cli.rs"), CORE_CLI_RS)
            .with_context(|| "failed to write installer core cli.rs")?;
        fs::write(core_src_dir.join("install.rs"), CORE_INSTALL_RS)
            .with_context(|| "failed to write installer core install.rs")?;
        fs::write(core_src_dir.join("registry.rs"), CORE_REGISTRY_RS)
            .with_context(|| "failed to write installer core registry.rs")?;
        fs::write(core_src_dir.join("uninstall.rs"), CORE_UNINSTALL_RS)
            .with_context(|| "failed to write installer core uninstall.rs")?;

        Ok(())
    }

    fn write_generated_rs(
        &self,
        platform: &PlatformManifest,
        files: &[PayloadFile],
        uninstaller_source: &str,
        setup_source_dir: &Path,
    ) -> Result<()> {
        let display_icon = display_icon_destination(platform, files);
        fs::write(
            setup_source_dir.join("src").join("generated.rs"),
            generated_rs(
                &self.build_manifest.app_name,
                &self.build_manifest.version,
                self.build_manifest.build.publisher.as_deref(),
                display_icon.as_deref(),
                &platform.variables,
                uninstaller_source,
                files,
            ),
        )
        .with_context(|| "failed to write installer generated.rs")?;

        Ok(())
    }

    fn build_setup(&self, target: &str, setup_source_dir: &Path) -> Result<()> {
        remove_dir_if_exists(&setup_source_dir.join("target"))?;

        let status = Command::new("cargo")
            .arg("build")
            .arg("--release")
            .arg("--target")
            .arg(target)
            .arg("--bin")
            .arg("setup")
            .current_dir(setup_source_dir)
            .status()
            .with_context(|| format!("failed to build setup.exe for {target}"))?;

        if !status.success() {
            bail!("setup.exe build failed for {target}");
        }

        Ok(())
    }

    fn copy_setup_output(
        &self,
        target: &str,
        setup_source_dir: &Path,
        output_dir: &Path,
    ) -> Result<()> {
        copy_release_exe(target, setup_source_dir, output_dir, "setup")
    }

    fn clean_setup_source(&self, setup_source_dir: &Path) -> Result<()> {
        fs::remove_dir_all(setup_source_dir).with_context(|| {
            format!(
                "failed to remove generated setup project {}",
                setup_source_dir.display()
            )
        })
    }
}

fn remove_dir_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path).with_context(|| format!("failed to remove {}", path.display()))?;
    }

    Ok(())
}

fn copy_release_exe(
    target: &str,
    setup_source_dir: &Path,
    output_dir: &Path,
    name: &str,
) -> Result<()> {
    let source = release_exe_path(target, setup_source_dir, name);
    let destination = output_dir.join(format!("{name}.exe"));

    fs::copy(&source, &destination).with_context(|| {
        format!(
            "failed to copy {} to {}",
            source.display(),
            destination.display()
        )
    })?;

    Ok(())
}

fn release_exe_path(target: &str, setup_source_dir: &Path, name: &str) -> PathBuf {
    setup_source_dir
        .join("target")
        .join(target)
        .join("release")
        .join(format!("{name}.exe"))
}

fn setup_cargo_toml() -> String {
    SETUP_CARGO_TOML
        .replace(GENERATED_WORKSPACE_PLACEHOLDER, "[workspace]")
        .replace(
            GENERATED_CORE_DEPENDENCY_PLACEHOLDER,
            GENERATED_CORE_DEPENDENCY,
        )
}

fn setup_build_rs(files: &[PayloadFile]) -> Result<String> {
    let payload = payload_source(files)?;

    Ok(SETUP_BUILD_RS.replace(PAYLOAD_SOURCE_PLACEHOLDER, &payload))
}

fn write_setup_assets(setup_source_dir: &Path) -> Result<()> {
    let assets_dir = setup_source_dir.join("assets");
    fs::create_dir_all(&assets_dir)
        .with_context(|| format!("failed to create {}", assets_dir.display()))?;
    fs::write(assets_dir.join("install.ico"), INSTALL_ICON)
        .with_context(|| "failed to write setup install icon")?;

    Ok(())
}

fn payload_source(files: &[PayloadFile]) -> Result<String> {
    let mut source = String::from("{\n");
    source
        .push_str("    let out_dir = Path::new(&env::var(\"OUT_DIR\").unwrap()).to_path_buf();\n");
    source.push_str(
        "    let mut payload = String::from(\"static PAYLOAD: &[PayloadEntry] = &[\\n\");\n",
    );

    for (index, file) in files.iter().enumerate() {
        let source_path = fs::canonicalize(&file.source)
            .with_context(|| format!("failed to find payload source {}", &file.source))?;
        let payload_file_name = format!("payload-{index}.bin");

        source.push_str(&format!(
            "    let bytes = fs::read(\"{}\").unwrap();\n",
            source_path.to_string_lossy().escape_default(),
        ));
        source.push_str(&format!(
            "    fs::write(out_dir.join(\"{}\"), encode_payload(&bytes)).unwrap();\n",
            payload_file_name.escape_default(),
        ));
        source.push_str(&format!(
            "    payload.push_str(\"    PayloadEntry {{ destination: \\\"{}\\\", executable: {}, bytes: include_bytes!(concat!(env!(\\\"OUT_DIR\\\"), \\\"/{}\\\")) }},\\n\");\n",
            file.destination.escape_default(),
            file.executable,
            payload_file_name.escape_default(),
        ));
    }

    source.push_str("    payload.push_str(\"];\\n\");\n");
    source.push_str("    payload\n");
    source.push('}');

    Ok(source)
}

fn generated_rs(
    app_name: &str,
    app_version: &str,
    publisher: Option<&str>,
    display_icon: Option<&str>,
    variables: &[BuildVariable],
    uninstaller_source: &str,
    files: &[PayloadFile],
) -> String {
    let required_variables = variables
        .iter()
        .map(|variable| variable.name())
        .map(|name| format!("\"{}\"", name.escape_default()))
        .collect::<Vec<_>>()
        .join(", ");
    let mut path_entries = files
        .iter()
        .filter(|file| file.executable)
        .filter_map(|file| payload_parent(&file.destination))
        .map(|entry| format!("\"{}\"", entry.escape_default()))
        .collect::<Vec<_>>();
    path_entries.sort();
    path_entries.dedup();
    let path_entries = path_entries.join(", ");
    let uninstall_entries = files
        .iter()
        .map(|file| format!("\"{}\"", file.destination.escape_default()))
        .collect::<Vec<_>>()
        .join(", ");
    let uninstaller_bytes = if uninstaller_source.is_empty() {
        "&[]".to_owned()
    } else {
        format!(
            "include_bytes!(\"{}\")",
            uninstaller_source.escape_default()
        )
    };

    GENERATED_RS
        .replace(
            APP_NAME_PLACEHOLDER,
            &format!("\"{}\"", app_name.escape_default()),
        )
        .replace(
            APP_VERSION_PLACEHOLDER,
            &format!("\"{}\"", app_version.escape_default()),
        )
        .replace(
            APP_PUBLISHER_PLACEHOLDER,
            &match publisher {
                Some(value) => format!("Some(\"{}\")", value.escape_default()),
                None => "None".to_owned(),
            },
        )
        .replace(
            APP_DISPLAY_ICON_PLACEHOLDER,
            &match display_icon {
                Some(value) => format!("Some(\"{}\")", value.escape_default()),
                None => "None".to_owned(),
            },
        )
        .replace(REQUIRED_VARIABLES_PLACEHOLDER, &required_variables)
        .replace(UNINSTALL_ENTRIES_PLACEHOLDER, &uninstall_entries)
        .replace(PATH_ENTRIES_PLACEHOLDER, &path_entries)
        .replace(UNINSTALLER_SOURCE_PLACEHOLDER, &uninstaller_bytes)
}

fn display_icon_destination(platform: &PlatformManifest, files: &[PayloadFile]) -> Option<String> {
    platform.icons.iter().find_map(|icon| {
        let binary_file_name = format!("{}.exe", &icon.binary);

        files
            .iter()
            .find(|file| {
                let dest_path = Path::new(&file.destination);
                file.executable
                    && dest_path.file_name() == Some(std::ffi::OsStr::new(&binary_file_name))
            })
            .map(|file| file.destination.clone())
    })
}

fn payload_parent(path: &str) -> Option<String> {
    let parent = Path::new(path).parent()?;

    if parent.as_os_str().is_empty() {
        None
    } else {
        Some(
            parent
                .components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/"),
        )
    }
}
