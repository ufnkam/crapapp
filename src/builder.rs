use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::build_manifest::BuildManifest;
use crate::bundlers::{
    LinuxBundler, LinuxBundlerKind, MacosBundler, MacosBundlerKind, WindowsBundler,
    WindowsBundlerKind,
};
use crate::platform_manifest::{PlatformBuildManifest, PlatformManifest};
use crate::progress;

#[derive(Debug)]
pub struct BundleSelection {
    windows: Vec<WindowsBundlerKind>,
    macos: Vec<MacosBundlerKind>,
    linux: Vec<LinuxBundlerKind>,
}

impl BundleSelection {
    pub fn new(
        windows: Vec<WindowsBundlerKind>,
        macos: Vec<MacosBundlerKind>,
        linux: Vec<LinuxBundlerKind>,
    ) -> Self {
        Self {
            windows,
            macos,
            linux,
        }
    }

    fn all() -> Self {
        Self::new(Vec::new(), Vec::new(), Vec::new())
    }

    fn has_platform_filters(&self) -> bool {
        !self.windows.is_empty() || !self.macos.is_empty() || !self.linux.is_empty()
    }

    fn includes_windows(&self) -> bool {
        !self.has_platform_filters() || !self.windows.is_empty()
    }

    fn includes_macos(&self) -> bool {
        !self.has_platform_filters() || !self.macos.is_empty()
    }

    fn includes_linux(&self) -> bool {
        !self.has_platform_filters() || !self.linux.is_empty()
    }

    fn windows_bundles(
        &self,
        configured: &[WindowsBundlerKind],
    ) -> Result<Vec<WindowsBundlerKind>> {
        select_bundles("windows", configured, &self.windows)
    }

    fn macos_bundles(&self, configured: &[MacosBundlerKind]) -> Result<Vec<MacosBundlerKind>> {
        select_bundles("macos", configured, &self.macos)
    }

    fn linux_bundles(&self, configured: &[LinuxBundlerKind]) -> Result<Vec<LinuxBundlerKind>> {
        select_bundles("linux", configured, &self.linux)
    }
}

pub struct Builder<'a> {
    build_manifest: &'a BuildManifest,
}

impl<'a> Builder<'a> {
    pub fn new(build_manifest: &'a BuildManifest) -> Self {
        Self { build_manifest }
    }

    pub fn build(&self) -> Result<()> {
        let build_root = build_root()?;
        let selection = BundleSelection::all();
        validate_build_environments(self.build_manifest, &selection)?;

        for platform in &self.build_manifest.platforms {
            if platform_included(platform, &selection) {
                for target in platform.targets() {
                    build_target(&build_root, self.build_manifest, target)?;
                }
            }
        }

        Ok(())
    }

    pub fn bundle(&self, build: bool, selection: &BundleSelection) -> Result<()> {
        validate_selected_platforms(self.build_manifest, selection)?;

        let build_root = build_root()?;
        let build_dir = build_root.join(".crapapp_build");
        reset_build_dir(&build_dir)?;

        if build {
            validate_build_environments(self.build_manifest, selection)?;

            for platform in &self.build_manifest.platforms {
                if platform_included(platform, selection) {
                    for target in platform.targets() {
                        build_target(&build_root, self.build_manifest, target)?;
                    }
                }
            }
        }

        validate_bundle_inputs(self.build_manifest, selection)?;

        if let Some(PlatformBuildManifest::Windows(platform)) =
            self.build_manifest.get_platform_config("windows")
            && selection.includes_windows()
        {
            let bundles = selection.windows_bundles(&platform.bundle)?;
            WindowsBundler::new(self.build_manifest, platform, &build_dir).bundle(&bundles)?;
        }

        if let Some(PlatformBuildManifest::Macos(platform)) =
            self.build_manifest.get_platform_config("macos")
            && selection.includes_macos()
        {
            let bundles = selection.macos_bundles(&platform.bundle)?;
            MacosBundler::new(self.build_manifest, platform, &build_dir).bundle(&bundles)?;
        }

        if let Some(PlatformBuildManifest::Linux(platform)) =
            self.build_manifest.get_platform_config("linux")
            && selection.includes_linux()
        {
            let bundles = selection.linux_bundles(&platform.bundle)?;
            LinuxBundler::new(self.build_manifest, platform, &build_dir).bundle(&bundles)?;
        }

        Ok(())
    }
}

fn build_target(build_root: &Path, build_manifest: &BuildManifest, target: &str) -> Result<()> {
    validate_target_build_environment(target)?;

    let mut command = Command::new("cargo");
    configure_target_toolchain(&mut command, target)?;
    command.current_dir(build_root);
    command.arg("build").arg("--release").arg("--quiet");
    command.arg("--target-dir").arg(build_root.join("target"));
    command.arg("--target").arg(target);

    for package in &build_manifest.build.packages {
        command.arg("--package").arg(package);
    }

    if !build_manifest.build.features.is_empty() {
        command
            .arg("--features")
            .arg(build_manifest.build.features.join(" "));
    }

    let status = progress::run(&format!("Building target {target}"), || {
        command
            .status()
            .with_context(|| format!("failed to run cargo build for {target}"))
    })?;

    if !status.success() {
        bail!("cargo build failed for {target}");
    }

    Ok(())
}

fn validate_build_environments(
    build_manifest: &BuildManifest,
    selection: &BundleSelection,
) -> Result<()> {
    for platform in &build_manifest.platforms {
        if platform_included(platform, selection) {
            for target in platform.targets() {
                validate_target_build_environment(target)?;
            }
        }
    }

    Ok(())
}

fn validate_selected_platforms(
    build_manifest: &BuildManifest,
    selection: &BundleSelection,
) -> Result<()> {
    if !selection.windows.is_empty() && build_manifest.get_platform_config("windows").is_none() {
        bail!("windows bundles were requested, but CRAP.toml has no windows platform");
    }
    if !selection.macos.is_empty() && build_manifest.get_platform_config("macos").is_none() {
        bail!("macos bundles were requested, but CRAP.toml has no macos platform");
    }
    if !selection.linux.is_empty() && build_manifest.get_platform_config("linux").is_none() {
        bail!("linux bundles were requested, but CRAP.toml has no linux platform");
    }

    Ok(())
}

fn validate_target_build_environment(target: &str) -> Result<()> {
    if !cfg!(target_os = "macos") || !target.ends_with("-unknown-linux-gnu") {
        return Ok(());
    }

    let linker_env = format!(
        "CARGO_TARGET_{}_LINKER",
        target.replace('-', "_").to_ascii_uppercase()
    );
    if std::env::var_os(&linker_env).is_some() || std::env::var_os("RUSTFLAGS").is_some() {
        return Ok(());
    }

    if linux_gnu_toolchain(target).is_some() {
        return Ok(());
    }

    bail!(
        "cannot build target {target} from macOS with only the Rust target installed; \
         a Linux GNU linker and sysroot are also required. \
         install {}, configure {linker_env} or RUSTFLAGS for a Linux \
         toolchain, run on Linux, or build the target binaries elsewhere and use cargo crapapp \
         bundle --no-build",
        linux_gnu_linker_hint(target)
    );
}

fn configure_target_toolchain(command: &mut Command, target: &str) -> Result<()> {
    let Some(toolchain) = linux_gnu_toolchain(target) else {
        return Ok(());
    };

    let env_key = cargo_target_env_key(target, "LINKER");
    if std::env::var_os(&env_key).is_none() {
        command.env(env_key, &toolchain.cc);
    }

    let cc_key = build_script_env_key("CC", target);
    if std::env::var_os(&cc_key).is_none() {
        command.env(cc_key, &toolchain.cc);
    }

    if let Some(ar) = &toolchain.ar {
        let ar_key = build_script_env_key("AR", target);
        if std::env::var_os(&ar_key).is_none() {
            command.env(ar_key, ar);
        }
    }

    Ok(())
}

struct LinuxGnuToolchain {
    cc: PathBuf,
    ar: Option<PathBuf>,
}

fn linux_gnu_toolchain(target: &str) -> Option<LinuxGnuToolchain> {
    if !cfg!(target_os = "macos") {
        return None;
    }

    let (cc_names, ar_names): (&[&str], &[&str]) = match target {
        "x86_64-unknown-linux-gnu" => (
            &["x86_64-unknown-linux-gnu-gcc", "x86_64-linux-gnu-gcc"],
            &["x86_64-unknown-linux-gnu-ar", "x86_64-linux-gnu-ar"],
        ),
        "aarch64-unknown-linux-gnu" => (
            &["aarch64-unknown-linux-gnu-gcc", "aarch64-linux-gnu-gcc"],
            &["aarch64-unknown-linux-gnu-ar", "aarch64-linux-gnu-ar"],
        ),
        _ => return None,
    };

    let cc = cc_names.iter().find_map(|name| find_executable(name))?;
    let ar = ar_names.iter().find_map(|name| find_executable(name));

    Some(LinuxGnuToolchain { cc, ar })
}

fn linux_gnu_linker_hint(target: &str) -> &'static str {
    match target {
        "aarch64-unknown-linux-gnu" => "aarch64-unknown-linux-gnu-gcc",
        _ => "x86_64-unknown-linux-gnu-gcc",
    }
}

fn cargo_target_env_key(target: &str, suffix: &str) -> String {
    format!(
        "CARGO_TARGET_{}_{}",
        target.replace('-', "_").to_ascii_uppercase(),
        suffix
    )
}

fn build_script_env_key(prefix: &str, target: &str) -> String {
    format!("{prefix}_{}", target.replace('-', "_"))
}

fn find_executable(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&paths) {
        let path = directory.join(name);
        if path.is_file() {
            return Some(path);
        }
    }

    None
}

fn validate_bundle_inputs(
    build_manifest: &BuildManifest,
    selection: &BundleSelection,
) -> Result<()> {
    for platform in &build_manifest.platforms {
        if platform_included(platform, selection) {
            for target in target_manifests(platform) {
                for file in &target.files {
                    if !Path::new(&file.source).is_file() {
                        bail!(
                            "{} bundle input for target {} is missing: {}",
                            platform.platform(),
                            target.target,
                            file.source
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

fn platform_included(platform: &PlatformBuildManifest, selection: &BundleSelection) -> bool {
    match platform {
        PlatformBuildManifest::Windows(_) => selection.includes_windows(),
        PlatformBuildManifest::Macos(_) => selection.includes_macos(),
        PlatformBuildManifest::Linux(_) => selection.includes_linux(),
    }
}

fn select_bundles<T>(platform: &str, configured: &[T], requested: &[T]) -> Result<Vec<T>>
where
    T: Copy + Eq + std::fmt::Display,
{
    if requested.is_empty() {
        return Ok(configured.to_vec());
    }

    let mut selected = Vec::with_capacity(requested.len());
    for bundle in requested {
        if !configured.contains(bundle) {
            bail!("{platform} bundle {bundle} is not configured in CRAP.toml");
        }
        if !selected.contains(bundle) {
            selected.push(*bundle);
        }
    }

    Ok(selected)
}

fn target_manifests(platform: &PlatformBuildManifest) -> &[crate::target_manifest::TargetManifest] {
    match platform {
        PlatformBuildManifest::Windows(platform) => &platform.targets,
        PlatformBuildManifest::Macos(platform) => &platform.targets,
        PlatformBuildManifest::Linux(platform) => &platform.targets,
    }
}

fn build_root() -> Result<PathBuf> {
    std::env::current_dir().context("failed to resolve current directory")
}

fn reset_build_dir(build_dir: &Path) -> Result<()> {
    if build_dir.exists() {
        fs::remove_dir_all(build_dir)
            .with_context(|| format!("failed to remove {}", build_dir.display()))?;
    }

    fs::create_dir_all(build_dir)
        .with_context(|| format!("failed to create {}", build_dir.display()))?;

    Ok(())
}
