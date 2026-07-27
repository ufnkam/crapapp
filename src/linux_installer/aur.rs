use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use flate2::Compression;
use flate2::write::GzEncoder;
use sha1::{Digest, Sha1};
use tar::{Builder, Header};

use crate::linux_installer::{GeneratedFile, install_relative_path};
use crate::manifest_file::{AssociatedFile, AssociatedFileKind, EulaFile};
use crate::payload_file::PayloadFile;

pub struct AurSpec {
    pub package: String,
    pub version: String,
    pub bundled_at: String,
    pub description: String,
    pub architecture: String,
    pub files: Vec<PayloadFile>,
    pub generated_files: Vec<GeneratedFile>,
    pub associated_files: Vec<AssociatedFile>,
    pub eulas: Vec<EulaFile>,
}

pub fn build(spec: &AurSpec, output: &Path) -> anyhow::Result<()> {
    validate(spec)?;

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    if output.exists() {
        fs::remove_file(output)
            .with_context(|| format!("failed to remove {}", output.display()))?;
    }

    let entries = source_entries(spec)?;
    let package_root = format!("{}/", spec.package);
    let file =
        File::create(output).with_context(|| format!("failed to create {}", output.display()))?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut archive = Builder::new(encoder);
    append_file(
        &mut archive,
        &format!("{package_root}PKGBUILD"),
        0o644,
        pkgbuild(spec, &entries)?.as_bytes(),
    )?;
    append_file(
        &mut archive,
        &format!("{package_root}.SRCINFO"),
        0o644,
        srcinfo(spec, &entries).as_bytes(),
    )?;
    for entry in entries {
        append_file(
            &mut archive,
            &format!("{package_root}{}", entry.archive_path),
            entry.mode,
            &entry.bytes,
        )?;
    }
    let encoder = archive
        .into_inner()
        .context("failed to finish AUR archive")?;
    encoder
        .finish()
        .context("failed to finish AUR compression")?;

    Ok(())
}

fn append_file<W: Write>(
    archive: &mut Builder<W>,
    path: &str,
    mode: u32,
    bytes: &[u8],
) -> anyhow::Result<()> {
    let mut header = Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(mode);
    header.set_mtime(0);
    header.set_cksum();
    archive
        .append_data(&mut header, path, bytes)
        .with_context(|| format!("failed to add {path} to AUR archive"))
}

fn validate(spec: &AurSpec) -> anyhow::Result<()> {
    if spec.files.is_empty() {
        bail!("AUR package has no files to package");
    }

    Ok(())
}

struct SourceEntry {
    archive_path: String,
    install_path: String,
    mode: u32,
    bytes: Vec<u8>,
}

fn source_entries(spec: &AurSpec) -> anyhow::Result<Vec<SourceEntry>> {
    let mut entries = Vec::new();

    for (index, file) in spec.files.iter().enumerate() {
        let source = PathBuf::from(&file.source);
        let bytes =
            fs::read(&source).with_context(|| format!("failed to read {}", source.display()))?;
        entries.push(SourceEntry {
            archive_path: format!("payload-{index}-{}", source_file_name(&file.source)?),
            install_path: install_relative_path(&file.destination)?,
            mode: if file.executable { 0o755 } else { 0o644 },
            bytes,
        });
    }

    for (index, file) in spec.generated_files.iter().enumerate() {
        entries.push(SourceEntry {
            archive_path: format!(
                "generated-{index}-{}",
                source_file_name(&file.install_path)?
            ),
            install_path: install_relative_path(&file.install_path)?,
            mode: if file.executable { 0o755 } else { 0o644 },
            bytes: file.bytes.clone(),
        });
    }

    for eula in &spec.eulas {
        let source = PathBuf::from(eula.path());
        let bytes =
            fs::read(&source).with_context(|| format!("failed to read {}", source.display()))?;
        entries.push(SourceEntry {
            archive_path: format!("license-{}", source_file_name(eula.path())?),
            install_path: format!(
                "usr/share/licenses/{}/{}",
                spec.package,
                source_file_name(eula.path())?
            ),
            mode: 0o644,
            bytes,
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
        .map(|entry| format!("'{}'", hex(&sha1(&entry.bytes))))
        .collect::<Vec<_>>()
        .join(" ");

    let mut package = String::new();
    package.push_str(&format!("# Bundled-At: {}\n", spec.bundled_at));
    package.push_str(&format!("pkgname={}\n", spec.package));
    package.push_str(&format!("pkgver={}\n", pkgver(&spec.version)));
    package.push_str("pkgrel=1\n");
    package.push_str(&format!("pkgdesc='{}'\n", single_quoted(&spec.description)));
    package.push_str("url=''\n");
    package.push_str("license=('custom')\n");
    package.push_str(&format!("arch=('{}')\n", spec.architecture));
    package.push_str("options=('!debug')\n");
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
        let path = install_relative_path(&file.path)?;
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

fn srcinfo(spec: &AurSpec, entries: &[SourceEntry]) -> String {
    let sources = entries
        .iter()
        .map(|entry| format!("\tsource = {}", entry.archive_path))
        .collect::<Vec<_>>()
        .join("\n");
    let checksums = entries
        .iter()
        .map(|entry| format!("\tsha1sums = {}", hex(&sha1(&entry.bytes))))
        .collect::<Vec<_>>()
        .join("\n");
    let description = spec.description.replace('\n', " ");

    format!(
        "pkgbase = {package}\n\tpkgdesc = {description}\n\tpkgver = {pkgver}\n\tpkgrel = 1\n\turl = \n\tarch = {architecture}\n\tlicense = custom\n\toptions = !debug\n{sources}\n{checksums}\n\npkgname = {package}\n",
        package = spec.package,
        pkgver = pkgver(&spec.version),
        architecture = spec.architecture
    )
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
    use flate2::read::GzDecoder;
    use std::fs;
    use tar::Archive;

    #[test]
    fn aur_archive_contains_pkgbuild_and_payload() {
        let temp_dir =
            std::env::temp_dir().join(format!("cargo-crapapp-aur-{}", std::process::id()));
        let source = temp_dir.join("example");
        let output = temp_dir.join("example.aur");

        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        fs::write(&source, b"bin").expect("source should be written");

        build(
            &AurSpec {
                package: "example".to_owned(),
                version: "1.0.0".to_owned(),
                bundled_at: "2000-01-01T00:00:00Z".to_owned(),
                description: "Example".to_owned(),
                architecture: "x86_64".to_owned(),
                files: vec![PayloadFile::executable(
                    source.display().to_string(),
                    "/usr/bin/example".to_owned(),
                )],
                generated_files: Vec::new(),
                associated_files: Vec::new(),
                eulas: Vec::new(),
            },
            &output,
        )
        .expect("AUR package should be written");

        assert!(output.is_file());
        assert_eq!(
            archive_paths(&output),
            vec![
                "example/PKGBUILD".to_owned(),
                "example/.SRCINFO".to_owned(),
                "example/payload-0-example".to_owned(),
            ]
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn aur_archive_contains_srcinfo() {
        let temp_dir =
            std::env::temp_dir().join(format!("cargo-crapapp-aur-srcinfo-{}", std::process::id()));
        let source = temp_dir.join("example");
        let output = temp_dir.join("example.aur");

        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        fs::write(&source, b"bin").expect("source should be written");

        build(
            &AurSpec {
                package: "example".to_owned(),
                version: "1.0.0".to_owned(),
                bundled_at: "2000-01-01T00:00:00Z".to_owned(),
                description: "Example".to_owned(),
                architecture: "x86_64".to_owned(),
                files: vec![PayloadFile::executable(
                    source.display().to_string(),
                    "/usr/bin/example".to_owned(),
                )],
                generated_files: Vec::new(),
                associated_files: Vec::new(),
                eulas: Vec::new(),
            },
            &output,
        )
        .expect("AUR archive should be generated");

        let srcinfo = archive_file(&output, "example/.SRCINFO");
        assert!(srcinfo.contains("pkgbase = example"));
        assert!(srcinfo.contains("pkgname = example"));
        assert!(srcinfo.contains("options = !debug"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    fn archive_paths(path: &std::path::Path) -> Vec<String> {
        let file = fs::File::open(path).expect("AUR archive should open");
        let mut archive = Archive::new(GzDecoder::new(file));
        archive
            .entries()
            .expect("AUR archive entries should be read")
            .map(|entry| {
                entry
                    .expect("AUR archive entry should be valid")
                    .path()
                    .expect("AUR archive path should be valid")
                    .display()
                    .to_string()
            })
            .collect()
    }

    fn archive_file(path: &std::path::Path, name: &str) -> String {
        let file = fs::File::open(path).expect("AUR archive should open");
        let mut archive = Archive::new(GzDecoder::new(file));
        archive
            .entries()
            .expect("AUR archive entries should be read")
            .find_map(|entry| {
                let mut entry = entry.expect("AUR archive entry should be valid");
                (entry.path().ok().as_deref() == Some(std::path::Path::new(name))).then(|| {
                    let mut text = String::new();
                    std::io::Read::read_to_string(&mut entry, &mut text)
                        .expect("AUR archive file should be read");
                    text
                })
            })
            .expect("AUR archive file should exist")
    }
}
