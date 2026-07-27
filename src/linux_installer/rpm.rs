use std::fs;
use std::path::Path;

use anyhow::{Context, bail};
use chrono::{DateTime, TimeZone, Utc};
use rpm::{BuildConfig, CompressionType, FileOptions, PackageBuilder, Scriptlet};

use crate::linux_installer::{GeneratedFile, install_relative_path};
use crate::manifest_file::{AssociatedFile, AssociatedFileKind, EulaFile};
use crate::payload_file::PayloadFile;

pub struct RpmSpec {
    pub package: String,
    pub version: String,
    pub release: String,
    pub bundled_at: String,
    pub summary: String,
    pub description: String,
    pub architecture: String,
    pub license: String,
    pub homepage: Option<String>,
    pub publisher: String,
    pub files: Vec<PayloadFile>,
    pub generated_files: Vec<GeneratedFile>,
    pub associated_files: Vec<AssociatedFile>,
    pub eulas: Vec<EulaFile>,
}

pub fn build(spec: &RpmSpec, output: &Path) -> anyhow::Result<()> {
    validate(spec)?;

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let mut builder = PackageBuilder::new(
        &spec.package,
        &spec.version,
        &spec.license,
        &spec.architecture,
        &spec.summary,
    );

    builder
        .using_config(
            BuildConfig::v4()
                .compression(CompressionType::Gzip)
                .source_date(package_timestamp(&spec.bundled_at)),
        )
        .release(&spec.release)
        .description(format!(
            "{}\n\nBundled-At: {}",
            spec.description, spec.bundled_at
        ))
        .build_host("cargo-crapapp");
    if let Some(homepage) = &spec.homepage {
        builder.url(homepage);
    }
    builder
        .vendor(&spec.publisher)
        .post_install_script(Scriptlet::new(metadata_cache_refresh_script()))
        .post_uninstall_script(Scriptlet::new(metadata_cache_refresh_script()));

    for file in &spec.files {
        builder
            .with_file(
                &file.source,
                FileOptions::new(format!("/{}", install_relative_path(&file.destination)?))
                    .permissions(if file.executable { 0o755 } else { 0o644 }),
            )
            .with_context(|| format!("failed to add {} as {}", file.source, file.destination))?;
    }

    for file in &spec.generated_files {
        builder
            .with_file_contents(
                file.bytes.clone(),
                FileOptions::new(format!("/{}", install_relative_path(&file.install_path)?))
                    .permissions(if file.executable { 0o755 } else { 0o644 }),
            )
            .with_context(|| format!("failed to add generated file {}", file.install_path))?;
    }

    for file in &spec.associated_files {
        match file.kind {
            AssociatedFileKind::Directory => {
                builder
                    .with_dir_entry(
                        FileOptions::dir(format!("/{}", install_relative_path(&file.path)?))
                            .permissions(0o755),
                    )
                    .with_context(|| format!("failed to add directory {}", file.path))?;
            }
            AssociatedFileKind::File => {
                builder
                    .with_file_contents(
                        Vec::new(),
                        FileOptions::new(format!("/{}", install_relative_path(&file.path)?))
                            .permissions(0o644),
                    )
                    .with_context(|| format!("failed to add file {}", file.path))?;
            }
        }
    }

    for eula in &spec.eulas {
        let source = Path::new(eula.path());
        let file_name = source
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("LICENSE");
        builder
            .with_file(
                source,
                FileOptions::new(format!("/usr/share/licenses/{}/{file_name}", spec.package))
                    .permissions(0o644),
            )
            .with_context(|| format!("failed to add Linux package EULA {}", source.display()))?;
    }

    let package = builder.build()?;
    package
        .write_file(output)
        .with_context(|| format!("failed to write {}", output.display()))?;

    Ok(())
}

fn metadata_cache_refresh_script() -> &'static str {
    r#"if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database /usr/share/applications >/dev/null 2>&1 || :
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor >/dev/null 2>&1 || :
fi
if command -v appstreamcli >/dev/null 2>&1; then
  appstreamcli refresh-cache --force >/dev/null 2>&1 || :
fi
"#
}

fn validate(spec: &RpmSpec) -> anyhow::Result<()> {
    if spec.files.is_empty() {
        bail!("rpm package has no files to package");
    }

    Ok(())
}

fn package_timestamp(value: &str) -> u32 {
    DateTime::parse_from_rfc3339(value)
        .map(|value| {
            value
                .with_timezone(&Utc)
                .timestamp()
                .clamp(0, u32::MAX as i64) as u32
        })
        .unwrap_or_else(|_| {
            Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0)
                .single()
                .expect("fixed source date must be valid")
                .timestamp() as u32
        })
}

#[cfg(test)]
mod tests {
    use super::{RpmSpec, build};
    use crate::linux_installer::GeneratedFile;
    use crate::payload_file::PayloadFile;
    use std::fs;

    #[test]
    fn rpm_package_has_rpm_lead() {
        let temp_dir =
            std::env::temp_dir().join(format!("cargo-crapapp-rpm-{}", std::process::id()));
        let source = temp_dir.join("example");
        let output = temp_dir.join("example.rpm");

        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        fs::write(&source, b"bin").expect("source should be written");

        build(
            &RpmSpec {
                package: "example".to_owned(),
                version: "1.0.0".to_owned(),
                release: "1".to_owned(),
                bundled_at: "2000-01-01T00:00:00Z".to_owned(),
                summary: "Example".to_owned(),
                description: "Example".to_owned(),
                architecture: "x86_64".to_owned(),
                license: "MIT".to_owned(),
                homepage: Some("https://example.com".to_owned()),
                publisher: "Example Publisher".to_owned(),
                files: vec![PayloadFile::executable(
                    source.display().to_string(),
                    "/usr/bin/example".to_owned(),
                )],
                generated_files: vec![
                    GeneratedFile {
                        install_path: "/usr/share/metainfo/com.example.app.metainfo.xml".to_owned(),
                        bytes: b"<component/>".to_vec(),
                        executable: false,
                    },
                    GeneratedFile {
                        install_path: "/usr/share/doc/example/copyright".to_owned(),
                        bytes: b"License: MIT\n".to_vec(),
                        executable: false,
                    },
                ],
                associated_files: Vec::new(),
                eulas: Vec::new(),
            },
            &output,
        )
        .expect("rpm should be written");

        let bytes = fs::read(&output).expect("rpm should be readable");
        assert_eq!(&bytes[..4], &[0xed, 0xab, 0xee, 0xdb]);
        let package = ::rpm::Package::open(&output).expect("rpm should open");
        assert_eq!(package.metadata.get_summary().unwrap(), "Example");
        assert_eq!(package.metadata.get_license().unwrap(), "MIT");
        assert_eq!(package.metadata.get_url().unwrap(), "https://example.com");
        assert!(
            package
                .metadata
                .get_file_entries()
                .unwrap()
                .iter()
                .any(|entry| entry.path().to_string_lossy()
                    == "/usr/share/metainfo/com.example.app.metainfo.xml")
        );
        assert!(
            package
                .metadata
                .get_file_entries()
                .unwrap()
                .iter()
                .any(|entry| entry.path().to_string_lossy() == "/usr/share/doc/example/copyright")
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
