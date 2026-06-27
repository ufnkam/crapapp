use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::services::build_manifest::{BuildManifest, PlatformManifest};
use crate::services::toolchain_manifest::{BuildVariable, PayloadFile};

const SETUP_CARGO_TOML: &str = include_str!("../../templates/windows-setup/Cargo.toml");
const SETUP_BUILD_RS: &str = include_str!("../../templates/windows-setup/build.rs");
const SETUP_RS: &str = include_str!("../../templates/windows-setup/src/setup.rs");
const UNINSTALL_RS: &str = include_str!("../../templates/windows-setup/src/uninstall.rs");
const PAYLOAD_SOURCE_PLACEHOLDER: &str = "__CRAPAPP_PAYLOAD_SOURCE__";
const APP_NAME_PLACEHOLDER: &str = "__CRAPAPP_APP_NAME__";
const APP_VERSION_PLACEHOLDER: &str = "__CRAPAPP_APP_VERSION__";
const REQUIRED_VARIABLES_PLACEHOLDER: &str = "__CRAPAPP_REQUIRED_VARIABLES__";
const UNINSTALLER_SOURCE_PLACEHOLDER: &str = "__CRAPAPP_UNINSTALLER_SOURCE__";
const UNINSTALL_ENTRIES_PLACEHOLDER: &str = "__CRAPAPP_UNINSTALL_ENTRIES__";
const PATH_ENTRIES_PLACEHOLDER: &str = "__CRAPAPP_PATH_ENTRIES__";

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

        for toolchain in &windows.toolchains {
            let output_dir = self.build_dir.join("windows").join(&toolchain.toolchain);
            let setup_source_dir = output_dir.join("setup-src");

            fs::create_dir_all(setup_source_dir.join("src")).with_context(|| {
                format!(
                    "failed to create setup project at {}",
                    setup_source_dir.display()
                )
            })?;

            self.write_setup_project(windows, &toolchain.files, &setup_source_dir)?;
            self.build_uninstaller(&toolchain.toolchain, &setup_source_dir)?;
            self.write_setup_rs_with_uninstaller(windows, &toolchain.toolchain, &setup_source_dir)?;
            self.build_setup(&toolchain.toolchain, &setup_source_dir)?;
            self.copy_setup_output(&toolchain.toolchain, &setup_source_dir, &output_dir)?;
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

        fs::write(setup_source_dir.join("Cargo.toml"), SETUP_CARGO_TOML)
            .with_context(|| "failed to write setup Cargo.toml")?;
        fs::write(setup_source_dir.join("build.rs"), setup_build_rs(files)?)
            .with_context(|| "failed to write setup build.rs")?;
        fs::write(
            setup_source_dir.join("src").join("setup.rs"),
            setup_main_rs(
                &self.build_manifest.app_name,
                &self.build_manifest.version,
                &platform.variables,
                "",
                files,
            ),
        )
        .with_context(|| "failed to write setup.rs")?;
        fs::write(
            setup_source_dir.join("src").join("uninstall.rs"),
            uninstall_rs(&self.build_manifest.app_name, files),
        )
        .with_context(|| "failed to write uninstall.rs")?;

        Ok(())
    }

    fn build_uninstaller(&self, toolchain: &str, setup_source_dir: &Path) -> Result<()> {
        let status = Command::new("cargo")
            .arg("build")
            .arg("--release")
            .arg("--target")
            .arg(toolchain)
            .arg("--bin")
            .arg("uninstall")
            .current_dir(setup_source_dir)
            .status()
            .with_context(|| format!("failed to build uninstall.exe for {toolchain}"))?;

        if !status.success() {
            bail!("uninstall.exe build failed for {toolchain}");
        }

        Ok(())
    }

    fn write_setup_rs_with_uninstaller(
        &self,
        platform: &PlatformManifest,
        toolchain: &str,
        setup_source_dir: &Path,
    ) -> Result<()> {
        let uninstaller =
            fs::canonicalize(release_exe_path(toolchain, setup_source_dir, "uninstall"))
                .with_context(|| format!("failed to find built uninstall.exe for {toolchain}"))?;

        fs::write(
            setup_source_dir.join("src").join("setup.rs"),
            setup_main_rs(
                &self.build_manifest.app_name,
                &self.build_manifest.version,
                &platform.variables,
                &uninstaller.display().to_string(),
                &platform
                    .toolchains
                    .iter()
                    .find(|toolchain_manifest| toolchain_manifest.toolchain == toolchain)
                    .context("failed to find toolchain manifest")?
                    .files,
            ),
        )
        .with_context(|| "failed to write setup.rs with embedded uninstaller")?;

        Ok(())
    }

    fn build_setup(&self, toolchain: &str, setup_source_dir: &Path) -> Result<()> {
        let status = Command::new("cargo")
            .arg("build")
            .arg("--release")
            .arg("--target")
            .arg(toolchain)
            .arg("--bin")
            .arg("setup")
            .current_dir(setup_source_dir)
            .status()
            .with_context(|| format!("failed to build setup.exe for {toolchain}"))?;

        if !status.success() {
            bail!("setup.exe build failed for {toolchain}");
        }

        Ok(())
    }

    fn copy_setup_output(
        &self,
        toolchain: &str,
        setup_source_dir: &Path,
        output_dir: &Path,
    ) -> Result<()> {
        copy_release_exe(toolchain, setup_source_dir, output_dir, "setup")
    }
}

fn copy_release_exe(
    toolchain: &str,
    setup_source_dir: &Path,
    output_dir: &Path,
    name: &str,
) -> Result<()> {
    let source = release_exe_path(toolchain, setup_source_dir, name);
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

fn release_exe_path(toolchain: &str, setup_source_dir: &Path, name: &str) -> PathBuf {
    setup_source_dir
        .join("target")
        .join(toolchain)
        .join("release")
        .join(format!("{name}.exe"))
}

fn setup_build_rs(files: &[PayloadFile]) -> Result<String> {
    let payload = payload_source(files)?;

    Ok(SETUP_BUILD_RS.replace(PAYLOAD_SOURCE_PLACEHOLDER, &rust_string(&payload)))
}

fn payload_source(files: &[PayloadFile]) -> Result<String> {
    let mut output = String::from("static PAYLOAD: &[PayloadEntry] = &[\n");

    for file in files {
        let source_path = absolute_path(&file.source)?;

        output.push_str(&format!(
            "    PayloadEntry {{ destination: {}, executable: {}, bytes: include_bytes!({}) }},\n",
            rust_string(&file.destination),
            file.executable,
            rust_string(&source_path.display().to_string()),
        ));
    }

    output.push_str("];\n");
    Ok(output)
}

fn setup_main_rs(
    app_name: &str,
    app_version: &str,
    variables: &[BuildVariable],
    uninstaller_source: &str,
    files: &[PayloadFile],
) -> String {
    let required_variables = variables
        .iter()
        .map(|variable| rust_string(variable.name()))
        .collect::<Vec<_>>()
        .join(", ");

    SETUP_RS
        .replace(APP_NAME_PLACEHOLDER, &rust_string(app_name))
        .replace(APP_VERSION_PLACEHOLDER, &rust_string(app_version))
        .replace(REQUIRED_VARIABLES_PLACEHOLDER, &required_variables)
        .replace(PATH_ENTRIES_PLACEHOLDER, &rust_array(&path_entries(files)))
        .replace(
            UNINSTALLER_SOURCE_PLACEHOLDER,
            &rust_string(uninstaller_source),
        )
}

fn uninstall_rs(app_name: &str, files: &[PayloadFile]) -> String {
    UNINSTALL_RS
        .replace(APP_NAME_PLACEHOLDER, &rust_string(app_name))
        .replace(
            UNINSTALL_ENTRIES_PLACEHOLDER,
            &rust_array(&uninstall_entries(files)),
        )
        .replace(PATH_ENTRIES_PLACEHOLDER, &rust_array(&path_entries(files)))
}

fn absolute_path(path: &str) -> Result<PathBuf> {
    fs::canonicalize(path).with_context(|| format!("failed to find payload source {path}"))
}

fn rust_string(value: &str) -> String {
    format!("{value:?}")
}

fn uninstall_entries(files: &[PayloadFile]) -> Vec<String> {
    files.iter().map(|file| file.destination.clone()).collect()
}

fn path_entries(files: &[PayloadFile]) -> Vec<String> {
    let mut entries = files
        .iter()
        .filter(|file| file.executable)
        .filter_map(|file| payload_parent(&file.destination))
        .collect::<Vec<_>>();

    entries.sort();
    entries.dedup();
    entries
}

fn payload_parent(path: &str) -> Option<String> {
    let separator = path.rfind(['/', '\\'])?;
    let parent = path[..separator].to_owned();

    if parent.is_empty() {
        None
    } else {
        Some(parent)
    }
}

fn rust_array(values: &[String]) -> String {
    values
        .iter()
        .map(|value| rust_string(value))
        .collect::<Vec<_>>()
        .join(", ")
}
