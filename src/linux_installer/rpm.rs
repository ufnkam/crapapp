use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use chrono::{DateTime, TimeZone, Utc};
use rpm::{BuildConfig, CompressionType, FileOptions, PackageBuilder};

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
    pub files: Vec<PayloadFile>,
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

    for file in &spec.files {
        builder
            .with_file(
                &file.source,
                FileOptions::new(format!("/{}", package_path(&file.destination)?))
                    .permissions(if file.executable { 0o755 } else { 0o644 }),
            )
            .with_context(|| format!("failed to add {} as {}", file.source, file.destination))?;
    }

    for file in &spec.associated_files {
        match file.kind {
            AssociatedFileKind::Directory => {
                builder
                    .with_dir_entry(
                        FileOptions::dir(format!("/{}", package_path(&file.path)?))
                            .permissions(0o755),
                    )
                    .with_context(|| format!("failed to add directory {}", file.path))?;
            }
            AssociatedFileKind::File => {
                builder
                    .with_file_contents(
                        Vec::new(),
                        FileOptions::new(format!("/{}", package_path(&file.path)?))
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

fn validate(spec: &RpmSpec) -> anyhow::Result<()> {
    if spec.files.is_empty() {
        bail!("rpm package has no files to package");
    }

    Ok(())
}

fn package_path(path: &str) -> anyhow::Result<String> {
    if path.trim().is_empty() {
        bail!("package path must not be empty");
    }

    let path = Path::new(path);
    let mut relative = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::RootDir | std::path::Component::CurDir => {}
            std::path::Component::Normal(part) => relative.push(part),
            std::path::Component::ParentDir | std::path::Component::Prefix(_) => {
                bail!(
                    "package path {} must not contain parent or prefix components",
                    path.display()
                );
            }
        }
    }

    if relative.as_os_str().is_empty() {
        bail!(
            "package path {} must not resolve to package root",
            path.display()
        );
    }

    Ok(relative.display().to_string())
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
                license: "custom".to_owned(),
                files: vec![PayloadFile::executable(
                    source.display().to_string(),
                    "/usr/bin/example".to_owned(),
                )],
                associated_files: Vec::new(),
                eulas: Vec::new(),
            },
            &output,
        )
        .expect("rpm should be written");

        let bytes = fs::read(&output).expect("rpm should be readable");
        assert_eq!(&bytes[..4], &[0xed, 0xab, 0xee, 0xdb]);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
