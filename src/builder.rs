use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::build_manifest::BuildManifest;
use crate::bundlers::{LinuxBundler, MacosBundler, WindowsBundler};
use crate::platform_manifest::{PlatformBuildManifest, PlatformManifest};

pub struct Builder<'a> {
    build_manifest: &'a BuildManifest,
}

impl<'a> Builder<'a> {
    pub fn new(build_manifest: &'a BuildManifest) -> Self {
        Self { build_manifest }
    }

    pub fn build(&self) -> Result<()> {
        let build_root = build_root()?;
        validate_build_environments(self.build_manifest)?;

        for platform in &self.build_manifest.platforms {
            for target in platform.targets() {
                build_target(&build_root, self.build_manifest, target)?;
            }
        }

        Ok(())
    }

    pub fn bundle(&self, build: bool) -> Result<()> {
        let build_root = build_root()?;
        let build_dir = build_root.join(".crapapp_build");
        reset_build_dir(&build_dir)?;

        if build {
            validate_build_environments(self.build_manifest)?;

            for platform in &self.build_manifest.platforms {
                for target in platform.targets() {
                    build_target(&build_root, self.build_manifest, target)?;
                }
            }
        }

        validate_bundle_inputs(self.build_manifest)?;

        if let Some(PlatformBuildManifest::Windows(platform)) =
            self.build_manifest.get_platform_config("windows")
        {
            WindowsBundler::new(self.build_manifest, platform, &build_dir).bundle()?;
        }

        if let Some(PlatformBuildManifest::Macos(platform)) =
            self.build_manifest.get_platform_config("macos")
        {
            MacosBundler::new(self.build_manifest, platform, &build_dir).bundle()?;
        }

        if let Some(PlatformBuildManifest::Linux(platform)) =
            self.build_manifest.get_platform_config("linux")
        {
            LinuxBundler::new(self.build_manifest, platform, &build_dir).bundle()?;
        }

        Ok(())
    }
}

fn build_target(build_root: &Path, build_manifest: &BuildManifest, target: &str) -> Result<()> {
    validate_target_build_environment(target)?;

    let mut command = Command::new("cargo");
    configure_target_toolchain(&mut command, target)?;
    command.current_dir(build_root);
    command.arg("build").arg("--release");
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

    let status = command
        .status()
        .with_context(|| format!("failed to run cargo build for {target}"))?;

    if !status.success() {
        bail!("cargo build failed for {target}");
    }

    Ok(())
}

fn validate_build_environments(build_manifest: &BuildManifest) -> Result<()> {
    for platform in &build_manifest.platforms {
        for target in platform.targets() {
            validate_target_build_environment(target)?;
        }
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
        "cannot build target {target} from macOS without a Linux GNU linker and sysroot; \
         install x86_64-unknown-linux-gnu-gcc, configure {linker_env} or RUSTFLAGS for a Linux \
         toolchain, run on Linux, or build the target binaries elsewhere and use cargo crapapp \
         bundle --no-build"
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
    if !cfg!(target_os = "macos") || target != "x86_64-unknown-linux-gnu" {
        return None;
    }

    let cc = find_executable("x86_64-unknown-linux-gnu-gcc")
        .or_else(|| find_executable("x86_64-linux-gnu-gcc"))?;
    let ar = find_executable("x86_64-unknown-linux-gnu-ar")
        .or_else(|| find_executable("x86_64-linux-gnu-ar"));

    Some(LinuxGnuToolchain { cc, ar })
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

fn validate_bundle_inputs(build_manifest: &BuildManifest) -> Result<()> {
    for platform in &build_manifest.platforms {
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

    Ok(())
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
