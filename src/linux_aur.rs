use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use flate2::Compression;
use flate2::write::GzEncoder;
use sha1::{Digest, Sha1};

use crate::manifest_file::{AssociatedFile, AssociatedFileKind, EulaFile};
use crate::payload_file::PayloadFile;

const TAR_TIMESTAMP: u64 = 946_684_800;

pub struct AurSpec {
    pub package: String,
    pub version: String,
    pub description: String,
    pub architecture: String,
    pub files: Vec<PayloadFile>,
    pub associated_files: Vec<AssociatedFile>,
    pub eulas: Vec<EulaFile>,
}

pub fn build(spec: &AurSpec, output: &Path) -> anyhow::Result<()> {
    validate(spec)?;

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    fs::write(output, source_archive(spec)?)
        .with_context(|| format!("failed to write {}", output.display()))?;

    Ok(())
}

fn validate(spec: &AurSpec) -> anyhow::Result<()> {
    if spec.files.is_empty() {
        bail!("AUR package has no files to package");
    }

    Ok(())
}

fn source_archive(spec: &AurSpec) -> anyhow::Result<Vec<u8>> {
    let root = format!("{}-{}", spec.package, spec.version);
    let mut tar = Vec::new();
    let entries = source_entries(spec)?;
    let pkgbuild = pkgbuild(spec, &entries)?;
    write_tar_file(
        &mut tar,
        &format!("{root}/PKGBUILD"),
        pkgbuild.as_bytes(),
        0o644,
    )?;

    for entry in entries {
        let bytes = fs::read(&entry.source)
            .with_context(|| format!("failed to read {}", entry.source.display()))?;
        write_tar_file(
            &mut tar,
            &format!("{root}/{}", entry.archive_path),
            &bytes,
            entry.mode,
        )?;
    }

    finish_tar(&mut tar);
    gzip(&tar)
}

struct SourceEntry {
    source: PathBuf,
    archive_path: String,
    install_path: String,
    mode: u32,
}

fn source_entries(spec: &AurSpec) -> anyhow::Result<Vec<SourceEntry>> {
    let mut entries = Vec::new();

    for (index, file) in spec.files.iter().enumerate() {
        entries.push(SourceEntry {
            source: PathBuf::from(&file.source),
            archive_path: format!("payload/{index}-{}", source_file_name(&file.source)?),
            install_path: package_path(&file.destination)?,
            mode: if file.executable { 0o755 } else { 0o644 },
        });
    }

    for eula in &spec.eulas {
        let source = PathBuf::from(eula.path());
        entries.push(SourceEntry {
            archive_path: format!("licenses/{}", source_file_name(eula.path())?),
            install_path: format!(
                "usr/share/licenses/{}/{}",
                spec.package,
                source_file_name(eula.path())?
            ),
            source,
            mode: 0o644,
        });
    }

    Ok(entries)
}

fn pkgbuild(spec: &AurSpec, entries: &[SourceEntry]) -> anyhow::Result<String> {
    let source = entries
        .iter()
        .map(|entry| format!("'{}'", entry.archive_path))
        .collect::<Vec<_>>()
        .join(" ");
    let checksums = entries
        .iter()
        .map(|entry| {
            fs::read(&entry.source)
                .map(|bytes| format!("'{}'", hex(&sha1(&bytes))))
                .with_context(|| format!("failed to read {}", entry.source.display()))
        })
        .collect::<anyhow::Result<Vec<_>>>()?
        .join(" ");

    let mut package = String::new();
    package.push_str(&format!("pkgname={}\n", spec.package));
    package.push_str(&format!("pkgver={}\n", pkgver(&spec.version)));
    package.push_str("pkgrel=1\n");
    package.push_str(&format!("pkgdesc='{}'\n", single_quoted(&spec.description)));
    package.push_str("url=''\n");
    package.push_str("license=('custom')\n");
    package.push_str(&format!("arch=('{}')\n", spec.architecture));
    package.push_str(&format!("source=({source})\n"));
    package.push_str(&format!("sha1sums=({checksums})\n\n"));
    package.push_str("package() {\n");

    for entry in entries {
        let install = if entry.mode & 0o111 != 0 {
            "install -Dm755"
        } else {
            "install -Dm644"
        };
        package.push_str(&format!(
            "  {install} \"$srcdir/{}\" \"$pkgdir/{}\"\n",
            entry.archive_path, entry.install_path
        ));
    }

    for file in &spec.associated_files {
        let path = package_path(&file.path)?;
        match file.kind {
            AssociatedFileKind::Directory => {
                package.push_str(&format!("  install -dm755 \"$pkgdir/{path}\"\n"));
            }
            AssociatedFileKind::File => {
                package.push_str(&format!("  install -Dm644 /dev/null \"$pkgdir/{path}\"\n"));
            }
        }
    }

    package.push_str("}\n");
    Ok(package)
}

fn pkgver(version: &str) -> String {
    version.replace('-', "_")
}

fn single_quoted(value: &str) -> String {
    value.replace('\'', "'\"'\"'")
}

fn source_file_name(path: &str) -> anyhow::Result<String> {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("source path {path} must have a UTF-8 file name"))
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

fn gzip(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes)?;
    Ok(encoder.finish()?)
}

fn write_tar_file(output: &mut Vec<u8>, path: &str, bytes: &[u8], mode: u32) -> anyhow::Result<()> {
    write_tar_header(output, path, mode, bytes.len() as u64)?;
    output.extend_from_slice(bytes);
    while !output.len().is_multiple_of(512) {
        output.push(0);
    }
    Ok(())
}

fn write_tar_header(output: &mut Vec<u8>, path: &str, mode: u32, size: u64) -> anyhow::Result<()> {
    if path.len() > 100 {
        bail!("tar path {path} is too long");
    }

    let mut header = [0u8; 512];
    write_tar_bytes(&mut header[0..100], path.as_bytes());
    write_tar_octal(&mut header[100..108], mode as u64)?;
    write_tar_octal(&mut header[108..116], 0)?;
    write_tar_octal(&mut header[116..124], 0)?;
    write_tar_octal(&mut header[124..136], size)?;
    write_tar_octal(&mut header[136..148], TAR_TIMESTAMP)?;
    for byte in &mut header[148..156] {
        *byte = b' ';
    }
    header[156] = b'0';
    write_tar_bytes(&mut header[257..263], b"ustar\0");
    write_tar_bytes(&mut header[263..265], b"00");
    let checksum = header.iter().map(|byte| *byte as u64).sum::<u64>();
    let text = format!("{checksum:06o}\0 ");
    header[148..156].copy_from_slice(text.as_bytes());
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

fn finish_tar(output: &mut Vec<u8>) {
    output.extend_from_slice(&[0; 1024]);
}

fn sha1(bytes: &[u8]) -> Vec<u8> {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    hasher.finalize().to_vec()
}

fn hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push_str(&format!("{byte:02x}"));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::{AurSpec, build};
    use crate::payload_file::PayloadFile;
    use std::fs;

    #[test]
    fn aur_source_package_contains_pkgbuild() {
        let temp_dir =
            std::env::temp_dir().join(format!("cargo-crapapp-aur-{}", std::process::id()));
        let source = temp_dir.join("example");
        let output = temp_dir.join("example.src.tar.gz");

        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        fs::write(&source, b"bin").expect("source should be written");

        build(
            &AurSpec {
                package: "example".to_owned(),
                version: "1.0.0".to_owned(),
                description: "Example".to_owned(),
                architecture: "x86_64".to_owned(),
                files: vec![PayloadFile::executable(
                    source.display().to_string(),
                    "/usr/bin/example".to_owned(),
                )],
                associated_files: Vec::new(),
                eulas: Vec::new(),
            },
            &output,
        )
        .expect("AUR package should be written");

        assert!(output.is_file());
        assert!(fs::metadata(&output).expect("metadata").len() > 0);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
