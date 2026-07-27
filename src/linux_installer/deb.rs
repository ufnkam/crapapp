use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, bail};
use chrono::{DateTime, TimeZone, Utc};
use flate2::Compression;
use flate2::write::GzEncoder;

use crate::linux_installer::{GeneratedFile, install_relative_path};
use crate::manifest_file::{AssociatedFile, AssociatedFileKind, EulaFile};
use crate::payload_file::PayloadFile;

pub struct DebSpec {
    pub package: String,
    pub version: String,
    pub bundled_at: String,
    pub maintainer: String,
    pub summary: String,
    pub description: String,
    pub homepage: Option<String>,
    pub architecture: String,
    pub files: Vec<PayloadFile>,
    pub generated_files: Vec<GeneratedFile>,
    pub associated_files: Vec<AssociatedFile>,
    pub eulas: Vec<EulaFile>,
}

pub fn build(spec: &DebSpec, output: &Path) -> anyhow::Result<()> {
    validate(spec)?;

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let control = control_archive(spec)?;
    let data = data_archive(spec)?;
    let timestamp = package_timestamp(&spec.bundled_at);
    let mut package = Vec::new();
    package.extend_from_slice(b"!<arch>\n");
    write_ar_entry(&mut package, "debian-binary", b"2.0\n", timestamp)?;
    write_ar_entry(&mut package, "control.tar.gz", &control, timestamp)?;
    write_ar_entry(&mut package, "data.tar.gz", &data, timestamp)?;

    fs::write(output, package).with_context(|| format!("failed to write {}", output.display()))?;

    Ok(())
}

fn validate(spec: &DebSpec) -> anyhow::Result<()> {
    if spec.files.is_empty() {
        bail!("deb package has no files to package");
    }

    Ok(())
}

fn control_archive(spec: &DebSpec) -> anyhow::Result<Vec<u8>> {
    let timestamp = package_timestamp(&spec.bundled_at);
    let homepage = spec
        .homepage
        .as_deref()
        .filter(|homepage| !homepage.trim().is_empty())
        .map(|homepage| format!("Homepage: {homepage}\n"))
        .unwrap_or_default();
    let control = format!(
        "Package: {}\nVersion: {}\nArchitecture: {}\nMaintainer: {}\nInstalled-Size: {}\nSection: utils\nPriority: optional\n{}X-Cargo-Crapapp-Bundled-At: {}\nDescription: {}\n {}\n",
        spec.package,
        spec.version,
        spec.architecture,
        spec.maintainer,
        installed_size_kbytes(spec)?,
        homepage,
        spec.bundled_at,
        spec.summary,
        deb_long_description(&spec.description)
    );

    let mut tar = Vec::new();
    write_tar_file(&mut tar, "./control", control.as_bytes(), 0o644, timestamp)?;
    write_tar_file(
        &mut tar,
        "./postinst",
        metadata_cache_refresh_script().as_bytes(),
        0o755,
        timestamp,
    )?;
    finish_tar(&mut tar);
    gzip(&tar)
}

fn metadata_cache_refresh_script() -> &'static str {
    r#"#!/bin/sh
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database /usr/share/applications >/dev/null 2>&1 || :
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor >/dev/null 2>&1 || :
fi
if command -v appstreamcli >/dev/null 2>&1; then
  appstreamcli refresh-cache --force >/dev/null 2>&1 || :
fi
exit 0
"#
}

fn data_archive(spec: &DebSpec) -> anyhow::Result<Vec<u8>> {
    let mut tar = Vec::new();
    let timestamp = package_timestamp(&spec.bundled_at);
    let mut directories = BTreeSet::new();

    for file in &spec.files {
        let bytes =
            fs::read(&file.source).with_context(|| format!("failed to read {}", file.source))?;
        let path = tar_path(&file.destination)?;
        collect_parent_directories(&path, &mut directories);
        let mode = if file.executable { 0o755 } else { 0o644 };
        write_tar_file(&mut tar, &path, &bytes, mode, timestamp)?;
    }

    for file in &spec.generated_files {
        let path = tar_path(&file.install_path)?;
        collect_parent_directories(&path, &mut directories);
        let mode = if file.executable { 0o755 } else { 0o644 };
        write_tar_file(&mut tar, &path, &file.bytes, mode, timestamp)?;
    }

    for file in &spec.associated_files {
        let path = tar_path(&file.path)?;
        match file.kind {
            AssociatedFileKind::Directory => {
                collect_parent_directories(&path, &mut directories);
                directories.remove(&path);
                write_tar_directory(&mut tar, &path, 0o755, timestamp)?
            }
            AssociatedFileKind::File => {
                collect_parent_directories(&path, &mut directories);
                write_tar_file(&mut tar, &path, b"", 0o644, timestamp)?
            }
        }
    }

    for eula in &spec.eulas {
        let source = Path::new(eula.path());
        let bytes = fs::read(source)
            .with_context(|| format!("failed to read Linux package EULA {}", source.display()))?;
        let file_name = source
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("LICENSE");
        let path = format!("./usr/share/doc/{}/licenses/{file_name}", spec.package);
        collect_parent_directories(&path, &mut directories);
        write_tar_file(&mut tar, &path, &bytes, 0o644, timestamp)?;
    }

    let existing_tar = std::mem::take(&mut tar);
    for directory in directories {
        write_tar_directory(&mut tar, &directory, 0o755, timestamp)?;
    }
    tar.extend_from_slice(&existing_tar);

    finish_tar(&mut tar);
    gzip(&tar)
}

fn collect_parent_directories(path: &str, directories: &mut BTreeSet<String>) {
    let path = path.strip_prefix("./").unwrap_or(path);
    let mut parts = path.split('/').collect::<Vec<_>>();
    parts.pop();

    let mut current = String::from("./");
    for part in parts {
        if part.is_empty() {
            continue;
        }
        if current != "./" {
            current.push('/');
        }
        current.push_str(part);
        directories.insert(current.clone());
    }
}

fn installed_size_kbytes(spec: &DebSpec) -> anyhow::Result<u64> {
    let mut bytes = 0;
    for file in &spec.files {
        bytes += fs::metadata(&file.source)
            .with_context(|| format!("failed to read metadata for {}", file.source))?
            .len();
    }
    for file in &spec.generated_files {
        bytes += file.bytes.len() as u64;
    }
    Ok(bytes.div_ceil(1024))
}

fn deb_long_description(description: &str) -> String {
    description
        .lines()
        .map(|line| if line.is_empty() { "." } else { line })
        .collect::<Vec<_>>()
        .join("\n ")
}

fn tar_path(path: &str) -> anyhow::Result<String> {
    Ok(format!("./{}", install_relative_path(path)?))
}

fn gzip(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes)?;
    Ok(encoder.finish()?)
}

fn package_timestamp(value: &str) -> u64 {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc).timestamp().max(0) as u64)
        .unwrap_or_else(|_| {
            Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0)
                .single()
                .expect("fixed source date must be valid")
                .timestamp() as u64
        })
}

fn write_ar_entry(
    output: &mut Vec<u8>,
    name: &str,
    bytes: &[u8],
    timestamp: u64,
) -> anyhow::Result<()> {
    if name.len() > 15 {
        bail!("ar entry name {name} is too long");
    }
    let mode = format!("{:o}", 0o644);
    let header = format!(
        "{:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n",
        name,
        timestamp,
        0,
        0,
        mode,
        bytes.len()
    );
    output.extend_from_slice(header.as_bytes());
    output.extend_from_slice(bytes);
    if !bytes.len().is_multiple_of(2) {
        output.push(b'\n');
    }

    Ok(())
}

fn write_tar_file(
    output: &mut Vec<u8>,
    path: &str,
    bytes: &[u8],
    mode: u32,
    timestamp: u64,
) -> anyhow::Result<()> {
    write_tar_header(output, path, mode, bytes.len() as u64, b'0', timestamp)?;
    output.extend_from_slice(bytes);
    pad_tar(output);
    Ok(())
}

fn write_tar_directory(
    output: &mut Vec<u8>,
    path: &str,
    mode: u32,
    timestamp: u64,
) -> anyhow::Result<()> {
    let path = if path.ends_with('/') {
        path.to_owned()
    } else {
        format!("{path}/")
    };
    write_tar_header(output, &path, mode, 0, b'5', timestamp)
}

fn write_tar_header(
    output: &mut Vec<u8>,
    path: &str,
    mode: u32,
    size: u64,
    typeflag: u8,
    timestamp: u64,
) -> anyhow::Result<()> {
    let name = path.strip_prefix("./").unwrap_or(path);
    if name.len() > 100 {
        bail!("tar path {path} is too long");
    }

    let mut header = [0u8; 512];
    write_tar_bytes(&mut header[0..100], name.as_bytes());
    write_tar_octal(&mut header[100..108], mode as u64)?;
    write_tar_octal(&mut header[108..116], 0)?;
    write_tar_octal(&mut header[116..124], 0)?;
    write_tar_octal(&mut header[124..136], size)?;
    write_tar_octal(&mut header[136..148], timestamp)?;
    for byte in &mut header[148..156] {
        *byte = b' ';
    }
    header[156] = typeflag;
    write_tar_bytes(&mut header[257..263], b"ustar\0");
    write_tar_bytes(&mut header[263..265], b"00");
    write_tar_bytes(&mut header[265..297], b"root");
    write_tar_bytes(&mut header[297..329], b"root");

    let checksum = header.iter().map(|byte| *byte as u64).sum::<u64>();
    write_tar_checksum(&mut header[148..156], checksum)?;
    output.extend_from_slice(&header);

    Ok(())
}

fn write_tar_bytes(field: &mut [u8], bytes: &[u8]) {
    let len = bytes.len().min(field.len());
    field[..len].copy_from_slice(&bytes[..len]);
}

fn write_tar_octal(field: &mut [u8], value: u64) -> anyhow::Result<()> {
    let width = field.len();
    let text = format!("{value:0width$o}", width = width - 1);
    if text.len() + 1 > width {
        bail!("tar octal value {value} does not fit in {width} bytes");
    }
    field[..text.len()].copy_from_slice(text.as_bytes());
    field[text.len()] = 0;
    Ok(())
}

fn write_tar_checksum(field: &mut [u8], value: u64) -> anyhow::Result<()> {
    let text = format!("{value:06o}\0 ");
    if text.len() != field.len() {
        bail!("tar checksum field has invalid width");
    }
    field.copy_from_slice(text.as_bytes());
    Ok(())
}

fn pad_tar(output: &mut Vec<u8>) {
    while !output.len().is_multiple_of(512) {
        output.push(0);
    }
}

fn finish_tar(output: &mut Vec<u8>) {
    output.extend_from_slice(&[0; 1024]);
}

#[cfg(test)]
mod tests {
    use super::{DebSpec, build};
    use crate::payload_file::PayloadFile;
    use std::fs;

    #[test]
    fn deb_package_contains_standard_members() {
        let temp_dir =
            std::env::temp_dir().join(format!("cargo-crapapp-deb-{}", std::process::id()));
        let source_dir = temp_dir.join("source");
        let executable = source_dir.join("../../example");
        let output = temp_dir.join("example.deb");

        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&source_dir).expect("source dir should be created");
        fs::write(&executable, b"bin").expect("executable should be written");

        build(
            &DebSpec {
                package: "example".to_owned(),
                version: "1.0.0".to_owned(),
                bundled_at: "2000-01-01T00:00:00Z".to_owned(),
                maintainer: "ufnkam".to_owned(),
                summary: "Example App".to_owned(),
                description: "Example App".to_owned(),
                homepage: Some("https://example.com".to_owned()),
                architecture: "amd64".to_owned(),
                files: vec![PayloadFile::executable(
                    executable.display().to_string(),
                    "/usr/bin/example".to_owned(),
                )],
                generated_files: Vec::new(),
                associated_files: Vec::new(),
                eulas: Vec::new(),
            },
            &output,
        )
        .expect("deb should be written");

        let bytes = fs::read(&output).expect("deb should be readable");
        assert!(bytes.starts_with(b"!<arch>\n"));
        assert_eq!(&bytes[8..24], b"debian-binary   ");
        assert!(
            bytes
                .windows("debian-binary".len())
                .any(|window| { window == b"debian-binary" })
        );
        assert!(
            bytes
                .windows("control.tar.gz".len())
                .any(|window| { window == b"control.tar.gz" })
        );
        assert!(
            bytes
                .windows("data.tar.gz".len())
                .any(|window| { window == b"data.tar.gz" })
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
