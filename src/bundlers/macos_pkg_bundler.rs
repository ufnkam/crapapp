use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

use crate::build_manifest::BuildManifest;
use crate::bundlers::MacosInstallerKind;
use crate::bundlers::macos_app_bundler::{MacosAppBundler, bundle_identifier, bundle_name};
use crate::bundlers::shared;
use crate::macos_pkg::{self, FileSpec, PkgSpec};
use crate::manifest_file::EulaFile;
use crate::payload_file::PayloadFile;
use crate::platform_manifests::{MacosPkgConfig, MacosPlatformManifest};
use crate::target_manifest::TargetManifest;

const APPLICATIONS_DIR: &str = "/Applications";
const ROOT_DIR: &str = "/";

pub struct MacosPkgBundler {}

impl MacosPkgBundler {
    pub fn bundle(
        build_manifest: &BuildManifest,
        build_dir: &Path,
        platform_manifest: &MacosPlatformManifest,
        target_manifest: &TargetManifest,
        bundle: &MacosInstallerKind,
    ) -> anyhow::Result<()> {
        let target_dir = build_dir
            .join(&platform_manifest.platform)
            .join(&target_manifest.target)
            .join(bundle.to_string());

        if target_dir.exists() {
            fs::remove_dir_all(&target_dir)
                .with_context(|| format!("failed to remove {}", target_dir.display()))?;
        }
        fs::create_dir_all(&target_dir)
            .with_context(|| format!("failed to create {}", target_dir.display()))?;

        let stage_dir = target_dir.join("stage");
        let app_name = format!("{}.app", bundle_name(build_manifest));
        let app_install_path = app_install_path(&platform_manifest.pkg)?;
        let install_from_root = platform_manifest.pkg.link_bins;
        let app_path = if install_from_root {
            stage_dir
                .join(absolute_install_path(
                    app_install_path,
                    "macOS pkg install_path",
                )?)
                .join(&app_name)
        } else {
            stage_dir.join(&app_name)
        };

        MacosAppBundler::bundle_to_path(
            build_manifest,
            platform_manifest,
            target_manifest,
            &app_path,
        )?;

        if platform_manifest.pkg.link_bins {
            write_bin_shims(
                &stage_dir,
                &platform_manifest.pkg,
                app_install_path,
                &app_name,
                target_manifest,
            )?;
        }

        let pkg_path = target_dir.join(format!("{}.pkg", artifact_file_stem(build_manifest)));
        let spec = pkg_spec(
            build_manifest,
            &platform_manifest.pkg,
            &platform_manifest.eulas,
            &stage_dir,
        )?;

        macos_pkg::build(&spec, &pkg_path)
            .with_context(|| format!("failed to write {}", pkg_path.display()))?;
        fs::remove_dir_all(&stage_dir)
            .with_context(|| format!("failed to remove {}", stage_dir.display()))?;

        Ok(())
    }
}

fn pkg_spec(
    build_manifest: &BuildManifest,
    config: &MacosPkgConfig,
    eulas: &[EulaFile],
    stage_dir: &Path,
) -> anyhow::Result<PkgSpec> {
    let install_path = if config.link_bins {
        ROOT_DIR
    } else {
        app_install_path(config)?
    };

    Ok(PkgSpec {
        name: artifact_file_stem(build_manifest),
        display_name: bundle_name(build_manifest),
        identifier: config
            .identifier
            .clone()
            .unwrap_or_else(|| bundle_identifier(build_manifest)),
        version: build_manifest.version.clone(),
        install_path: install_path.to_owned(),
        license: pkg_license(eulas)?,
        files: package_files(stage_dir)?,
    })
}

fn pkg_license(eulas: &[EulaFile]) -> anyhow::Result<Option<Vec<u8>>> {
    if eulas.is_empty() {
        return Ok(None);
    }

    let mut license = String::new();
    for (index, eula) in eulas.iter().enumerate() {
        let path = Path::new(eula.path());
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read macOS pkg EULA file {}", path.display()))?;

        if index > 0 {
            license.push_str("\n\n");
        }
        license.push_str("==== ");
        license.push_str(eula.path());
        if !eula.required() {
            license.push_str(" (optional on platforms with custom EULA UI)");
        }
        license.push_str(" ====\n\n");
        license.push_str(&text);
        if !license.ends_with('\n') {
            license.push('\n');
        }
    }

    Ok(Some(license.into_bytes()))
}

fn package_files(stage_dir: &Path) -> anyhow::Result<Vec<FileSpec>> {
    let mut files = Vec::new();
    collect_package_files(stage_dir, stage_dir, &mut files)?;
    files.sort_by(|left, right| left.dest.cmp(&right.dest));

    if files.is_empty() {
        bail!("macOS pkg has no files to package");
    }

    Ok(files)
}

fn write_bin_shims(
    stage_dir: &Path,
    config: &MacosPkgConfig,
    app_install_path: &str,
    app_name: &str,
    target_manifest: &TargetManifest,
) -> anyhow::Result<()> {
    let bin_dir = config.bin_dir();
    let bin_path = stage_dir.join(absolute_install_path(bin_dir, "macOS pkg bin_dir")?);
    let mut names = Vec::new();

    for file in target_manifest.files.iter().filter(|file| file.executable) {
        let name = executable_name(file)?;
        if names.contains(&name) {
            bail!("macOS pkg has more than one executable named {name}");
        }

        let shim_path = bin_path.join(&name);
        let app_binary = format!("{app_install_path}/{app_name}/Contents/MacOS/{name}");
        write_executable_shim(&shim_path, &app_binary)?;
        names.push(name);
    }

    if names.is_empty() {
        bail!("macOS pkg has no executable payload to link");
    }

    Ok(())
}

fn write_executable_shim(shim_path: &Path, app_binary: &str) -> anyhow::Result<()> {
    if app_binary.contains(['\n', '\r']) {
        bail!("macOS pkg executable shim path must not contain newlines");
    }

    let parent = shim_path.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "failed to resolve shim directory for {}",
            shim_path.display()
        )
    })?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;

    let shim = format!(
        r#"#!/bin/sh
APP_BINARY=$(cat <<'CRAPAPP_APP_BINARY'
{app_binary}
CRAPAPP_APP_BINARY
)
exec "$APP_BINARY" "$@"
"#
    );
    fs::write(shim_path, shim)
        .with_context(|| format!("failed to write {}", shim_path.display()))?;
    shared::set_executable_permissions(shim_path)?;

    Ok(())
}

fn executable_name(file: &PayloadFile) -> anyhow::Result<String> {
    Path::new(&file.source)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "executable payload source {} must point to a UTF-8 file name",
                file.source
            )
        })
}

fn app_install_path(config: &MacosPkgConfig) -> anyhow::Result<&str> {
    let install_path = config.install_path.as_deref().unwrap_or(APPLICATIONS_DIR);

    if !install_path.starts_with('/') {
        bail!("macOS pkg install_path {install_path} must be absolute");
    }

    Ok(install_path)
}

fn absolute_install_path(path: &str, field: &str) -> anyhow::Result<PathBuf> {
    if !path.starts_with('/') {
        bail!("{field} {path} must be absolute");
    }

    let mut relative = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            std::path::Component::RootDir => {}
            std::path::Component::Normal(part) => relative.push(part),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir | std::path::Component::Prefix(_) => {
                bail!("{field} {path} must not contain parent or prefix components");
            }
        }
    }

    Ok(relative)
}

fn collect_package_files(
    stage_dir: &Path,
    current: &Path,
    files: &mut Vec<FileSpec>,
) -> anyhow::Result<()> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(current)
        .with_context(|| format!("failed to read package directory {}", current.display()))?
    {
        entries.push(entry.with_context(|| {
            format!(
                "failed to read package directory entry in {}",
                current.display()
            )
        })?);
    }
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_package_files(stage_dir, &path, files)?;
            continue;
        }

        if !path.is_file() {
            continue;
        }

        let relative_path = path
            .strip_prefix(stage_dir)
            .with_context(|| format!("failed to calculate relative path for {}", path.display()))?;
        let dest = slash_path(relative_path)?;
        files.push(FileSpec { src: path, dest });
    }

    Ok(())
}

fn slash_path(path: &Path) -> anyhow::Result<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        let std::path::Component::Normal(part) = component else {
            bail!("package path {} must be relative", path.display());
        };
        let Some(part) = part.to_str() else {
            bail!("package path {} must be UTF-8", path.display());
        };
        parts.push(part);
    }

    if parts.is_empty() {
        bail!("package path must not be empty");
    }

    Ok(parts.join("/"))
}

fn artifact_file_stem(build_manifest: &BuildManifest) -> String {
    bundle_name(build_manifest).replace(['/', '\\', ':'], "-")
}

#[cfg(test)]
mod tests {
    use super::{MacosPkgBundler, package_files, slash_path, write_bin_shims};
    use crate::build_config_manifest::BuildConfigManifest;
    use crate::build_manifest::BuildManifest;
    use crate::bundlers::MacosInstallerKind;
    use crate::manifest_file::EulaFile;
    use crate::payload_file::PayloadFile;
    use crate::platform_manifests::{MacosPkgConfig, MacosPlatformManifest};
    use crate::target_manifest::TargetManifest;
    use std::fs;
    use std::path::Path;

    #[test]
    fn package_files_are_stage_relative() {
        let temp_dir =
            std::env::temp_dir().join(format!("cargo-crapapp-pkg-files-{}", std::process::id()));
        let app_dir = temp_dir.join("Applications/Example App.app");
        let bin = app_dir.join("Contents/MacOS/example");
        let asset = app_dir.join("Contents/Resources/assets/config.json");
        let shim = temp_dir.join("usr/local/bin/example");

        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(bin.parent().expect("bin parent")).expect("bin parent should exist");
        fs::create_dir_all(asset.parent().expect("asset parent"))
            .expect("asset parent should exist");
        fs::create_dir_all(shim.parent().expect("shim parent")).expect("shim parent should exist");
        fs::write(&bin, b"bin").expect("bin should be written");
        fs::write(&asset, b"{}").expect("asset should be written");
        fs::write(&shim, b"shim").expect("shim should be written");

        let files = package_files(&temp_dir).expect("files should collect");
        let destinations = files
            .iter()
            .map(|file| file.dest.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            destinations,
            vec![
                "Applications/Example App.app/Contents/MacOS/example",
                "Applications/Example App.app/Contents/Resources/assets/config.json",
                "usr/local/bin/example",
            ]
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn bin_shims_execute_installed_app_binary() {
        let temp_dir =
            std::env::temp_dir().join(format!("cargo-crapapp-pkg-shims-{}", std::process::id()));
        let source_dir = temp_dir.join("source");
        let executable = source_dir.join("example cli");

        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&source_dir).expect("source dir should be created");
        fs::write(&executable, b"bin").expect("executable should be written");

        let target_manifest = TargetManifest {
            target: "aarch64-apple-darwin".to_owned(),
            files: vec![PayloadFile::executable(
                executable.display().to_string(),
                "bin/example cli".to_owned(),
            )],
            shortcuts: Vec::new(),
        };
        let config = MacosPkgConfig {
            identifier: None,
            install_path: Some("/Applications".to_owned()),
            bin_dir: Some("/usr/local/bin".to_owned()),
            link_bins: true,
        };

        write_bin_shims(
            &temp_dir,
            &config,
            "/Applications",
            "Example App.app",
            &target_manifest,
        )
        .expect("shim should be written");

        let shim_path = temp_dir.join("usr/local/bin/example cli");
        let shim = fs::read_to_string(&shim_path).expect("shim should be readable");

        assert_eq!(
            shim,
            "#!/bin/sh\nAPP_BINARY=$(cat <<'CRAPAPP_APP_BINARY'\n/Applications/Example App.app/Contents/MacOS/example cli\nCRAPAPP_APP_BINARY\n)\nexec \"$APP_BINARY\" \"$@\"\n"
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn slash_path_rejects_non_relative_paths() {
        assert!(slash_path(Path::new("/Applications/Example.app")).is_err());
    }

    #[test]
    fn pkg_bundle_writes_flat_package() {
        let temp_dir =
            std::env::temp_dir().join(format!("cargo-crapapp-pkg-bundle-{}", std::process::id()));
        let source_dir = temp_dir.join("source");
        let executable = source_dir.join("example");
        let eula = source_dir.join("EULA.txt");

        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&source_dir).expect("source dir should be created");
        fs::write(&executable, b"bin").expect("executable should be written");
        fs::write(&eula, b"pkg terms").expect("EULA should be written");

        let build_manifest = BuildManifest {
            app_name: "example".to_owned(),
            version: "1.0.0".to_owned(),
            build: BuildConfigManifest {
                publisher: Some("ufnkam".to_owned()),
                display_name: Some("Example App".to_owned()),
                packages: Vec::new(),
                features: Vec::new(),
            },
            platforms: Vec::new(),
        };
        let platform_manifest = MacosPlatformManifest::new(
            Vec::new(),
            None,
            None,
            Some("example"),
            vec![MacosInstallerKind::Pkg],
            MacosPkgConfig {
                identifier: Some("com.ufnkam.example".to_owned()),
                install_path: Some("/Applications".to_owned()),
                bin_dir: Some("/usr/local/bin".to_owned()),
                link_bins: true,
            },
            vec![EulaFile::Path(eula.display().to_string())],
        );
        let target_manifest = TargetManifest {
            target: "aarch64-apple-darwin".to_owned(),
            files: vec![PayloadFile::executable(
                executable.display().to_string(),
                "bin/example".to_owned(),
            )],
            shortcuts: Vec::new(),
        };

        MacosPkgBundler::bundle(
            &build_manifest,
            &temp_dir.join("build"),
            &platform_manifest,
            &target_manifest,
            &MacosInstallerKind::Pkg,
        )
        .expect("pkg bundle should be created");

        let pkg_path = temp_dir
            .join("build")
            .join("macos")
            .join("aarch64-apple-darwin")
            .join("pkg")
            .join("Example App.pkg");
        let pkg_bytes = fs::read(&pkg_path).expect("pkg should be readable");

        assert!(pkg_path.is_file());
        assert_eq!(&pkg_bytes[..4], b"xar!");
        assert!(
            pkg_bytes
                .windows("License.txt".len())
                .any(|window| { window == b"License.txt" })
        );
        assert!(
            !pkg_path
                .parent()
                .expect("pkg parent")
                .join("stage")
                .exists()
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
