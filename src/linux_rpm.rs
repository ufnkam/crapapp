use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use flate2::Compression;
use flate2::write::GzEncoder;
use sha1::{Digest, Sha1};

use crate::manifest_file::{AssociatedFile, AssociatedFileKind, EulaFile};
use crate::payload_file::PayloadFile;

const RPM_TIMESTAMP: u32 = 946_684_800;
const RPM_HEADER_MAGIC: [u8; 8] = [0x8e, 0xad, 0xe8, 0x01, 0, 0, 0, 0];

pub struct RpmSpec {
    pub package: String,
    pub version: String,
    pub release: String,
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

    fs::write(output, rpm_bytes(spec)?)
        .with_context(|| format!("failed to write {}", output.display()))?;

    Ok(())
}

fn validate(spec: &RpmSpec) -> anyhow::Result<()> {
    if spec.files.is_empty() {
        bail!("rpm package has no files to package");
    }

    Ok(())
}

fn rpm_bytes(spec: &RpmSpec) -> anyhow::Result<Vec<u8>> {
    let entries = package_entries(spec)?;
    let payload = gzip(&cpio_newc(&entries)?)?;
    let mut output = Vec::new();
    output.extend_from_slice(&lead(spec)?);
    output.extend_from_slice(&signature_header(payload.len() as u32)?);
    output.extend_from_slice(&main_header(spec, &entries)?);
    output.extend_from_slice(&payload);
    Ok(output)
}

#[derive(Clone)]
struct PackageEntry {
    path: String,
    bytes: Vec<u8>,
    mode: u16,
    digest: String,
}

fn package_entries(spec: &RpmSpec) -> anyhow::Result<Vec<PackageEntry>> {
    let mut entries = Vec::new();

    for file in &spec.files {
        let bytes =
            fs::read(&file.source).with_context(|| format!("failed to read {}", file.source))?;
        entries.push(PackageEntry {
            path: package_path(&file.destination)?,
            digest: hex(&sha1(&bytes)),
            bytes,
            mode: if file.executable { 0o755 } else { 0o644 },
        });
    }

    for file in &spec.associated_files {
        let path = package_path(&file.path)?;
        match file.kind {
            AssociatedFileKind::Directory => entries.push(PackageEntry {
                path,
                bytes: Vec::new(),
                mode: 0o755 | 0o040000,
                digest: String::new(),
            }),
            AssociatedFileKind::File => entries.push(PackageEntry {
                path,
                bytes: Vec::new(),
                mode: 0o644,
                digest: hex(&sha1(&[])),
            }),
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
        entries.push(PackageEntry {
            path: format!("usr/share/licenses/{}/{file_name}", spec.package),
            digest: hex(&sha1(&bytes)),
            bytes,
            mode: 0o644,
        });
    }

    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

fn lead(spec: &RpmSpec) -> anyhow::Result<Vec<u8>> {
    let mut lead = vec![0u8; 96];
    lead[0..4].copy_from_slice(&[0xed, 0xab, 0xee, 0xdb]);
    lead[4] = 3;
    lead[5] = 0;
    lead[6..8].copy_from_slice(&0u16.to_be_bytes());
    lead[8..10].copy_from_slice(&rpm_arch_number(&spec.architecture)?.to_be_bytes());
    write_fixed_string(
        &mut lead[10..76],
        &format!("{}-{}", spec.package, spec.version),
    );
    lead[76..78].copy_from_slice(&1u16.to_be_bytes());
    lead[78..80].copy_from_slice(&5u16.to_be_bytes());
    Ok(lead)
}

fn rpm_arch_number(arch: &str) -> anyhow::Result<u16> {
    match arch {
        "x86_64" => Ok(1),
        _ => bail!("rpm architecture id for {arch} is not supported yet"),
    }
}

fn write_fixed_string(field: &mut [u8], value: &str) {
    let bytes = value.as_bytes();
    let len = bytes.len().min(field.len().saturating_sub(1));
    field[..len].copy_from_slice(&bytes[..len]);
}

fn signature_header(payload_size: u32) -> anyhow::Result<Vec<u8>> {
    let mut header = HeaderBuilder::new();
    header.int32(1000, &[payload_size]);
    let mut bytes = header.finish()?;
    while bytes.len() % 8 != 0 {
        bytes.push(0);
    }
    Ok(bytes)
}

fn main_header(spec: &RpmSpec, entries: &[PackageEntry]) -> anyhow::Result<Vec<u8>> {
    let file_meta = FileMetadata::new(entries);
    let mut header = HeaderBuilder::new();
    header.string(1000, &spec.package);
    header.string(1001, &spec.version);
    header.string(1002, &spec.release);
    header.string(1004, &spec.summary);
    header.string(1005, &spec.description);
    header.int32(1006, &[RPM_TIMESTAMP]);
    header.string(1007, "cargo-crapapp");
    header.int32(
        1009,
        &[entries.iter().map(|entry| entry.bytes.len() as u32).sum()],
    );
    header.string(1014, &spec.license);
    header.string(1016, "Applications/System");
    header.string(1021, "linux");
    header.string(1022, &spec.architecture);
    header.int32(1028, &file_meta.sizes);
    header.int16(1030, &file_meta.modes);
    header.int16(1033, &vec![0; entries.len()]);
    header.int32(1034, &vec![RPM_TIMESTAMP; entries.len()]);
    header.string_array(1035, &file_meta.digests);
    header.string_array(1036, &vec![String::new(); entries.len()]);
    header.int32(1037, &vec![0; entries.len()]);
    header.string_array(1039, &vec!["root".to_owned(); entries.len()]);
    header.string_array(1040, &vec!["root".to_owned(); entries.len()]);
    header.int32(1095, &vec![0; entries.len()]);
    header.int32(1096, &(1..=entries.len() as u32).collect::<Vec<_>>());
    header.string_array(1097, &vec![String::new(); entries.len()]);
    header.string("1124", "cpio");
    header.string("1125", "gzip");
    header.string("1126", "9");
    header.int32(1116, &file_meta.dir_indexes);
    header.string_array(1117, &file_meta.base_names);
    header.string_array(1118, &file_meta.dir_names);
    header.int32(5011, &[1]);
    header.finish()
}

struct FileMetadata {
    sizes: Vec<u32>,
    modes: Vec<u16>,
    digests: Vec<String>,
    dir_indexes: Vec<u32>,
    base_names: Vec<String>,
    dir_names: Vec<String>,
}

impl FileMetadata {
    fn new(entries: &[PackageEntry]) -> Self {
        let mut dir_map = BTreeMap::<String, u32>::new();
        let mut dir_names = Vec::new();
        let mut dir_indexes = Vec::new();
        let mut base_names = Vec::new();

        for entry in entries {
            let (dir, base) = split_path(&entry.path);
            let index = if let Some(index) = dir_map.get(&dir) {
                *index
            } else {
                let index = dir_names.len() as u32;
                dir_map.insert(dir.clone(), index);
                dir_names.push(dir);
                index
            };
            dir_indexes.push(index);
            base_names.push(base);
        }

        Self {
            sizes: entries
                .iter()
                .map(|entry| entry.bytes.len() as u32)
                .collect(),
            modes: entries.iter().map(|entry| entry.mode).collect(),
            digests: entries.iter().map(|entry| entry.digest.clone()).collect(),
            dir_indexes,
            base_names,
            dir_names,
        }
    }
}

fn split_path(path: &str) -> (String, String) {
    let Some(index) = path.rfind('/') else {
        return ("/".to_owned(), path.to_owned());
    };

    (
        format!("/{}", &path[..=index]),
        path[index + 1..].to_owned(),
    )
}

struct HeaderBuilder {
    indexes: Vec<IndexEntry>,
    store: Vec<u8>,
}

struct IndexEntry {
    tag: u32,
    kind: u32,
    offset: u32,
    count: u32,
}

impl HeaderBuilder {
    fn new() -> Self {
        Self {
            indexes: Vec::new(),
            store: Vec::new(),
        }
    }

    fn string(&mut self, tag: impl IntoHeaderTag, value: &str) {
        self.align(1);
        let offset = self.store.len() as u32;
        self.store.extend_from_slice(value.as_bytes());
        self.store.push(0);
        self.indexes.push(IndexEntry {
            tag: tag.into_header_tag(),
            kind: 6,
            offset,
            count: 1,
        });
    }

    fn string_array(&mut self, tag: u32, values: &[String]) {
        self.align(1);
        let offset = self.store.len() as u32;
        for value in values {
            self.store.extend_from_slice(value.as_bytes());
            self.store.push(0);
        }
        self.indexes.push(IndexEntry {
            tag,
            kind: 8,
            offset,
            count: values.len() as u32,
        });
    }

    fn int16(&mut self, tag: u32, values: &[u16]) {
        self.align(2);
        let offset = self.store.len() as u32;
        for value in values {
            self.store.extend_from_slice(&value.to_be_bytes());
        }
        self.indexes.push(IndexEntry {
            tag,
            kind: 3,
            offset,
            count: values.len() as u32,
        });
    }

    fn int32(&mut self, tag: u32, values: &[u32]) {
        self.align(4);
        let offset = self.store.len() as u32;
        for value in values {
            self.store.extend_from_slice(&value.to_be_bytes());
        }
        self.indexes.push(IndexEntry {
            tag,
            kind: 4,
            offset,
            count: values.len() as u32,
        });
    }

    fn finish(self) -> anyhow::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&RPM_HEADER_MAGIC);
        bytes.extend_from_slice(&(self.indexes.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&(self.store.len() as u32).to_be_bytes());
        for index in self.indexes {
            bytes.extend_from_slice(&index.tag.to_be_bytes());
            bytes.extend_from_slice(&index.kind.to_be_bytes());
            bytes.extend_from_slice(&index.offset.to_be_bytes());
            bytes.extend_from_slice(&index.count.to_be_bytes());
        }
        bytes.extend_from_slice(&self.store);
        Ok(bytes)
    }

    fn align(&mut self, width: usize) {
        while !self.store.len().is_multiple_of(width) {
            self.store.push(0);
        }
    }
}

trait IntoHeaderTag {
    fn into_header_tag(self) -> u32;
}

impl IntoHeaderTag for u32 {
    fn into_header_tag(self) -> u32 {
        self
    }
}

impl IntoHeaderTag for &str {
    fn into_header_tag(self) -> u32 {
        self.parse().expect("static RPM tag should be numeric")
    }
}

fn cpio_newc(entries: &[PackageEntry]) -> anyhow::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut inode = 1;

    for entry in entries {
        inode += 1;
        write_cpio_entry(
            &mut output,
            inode,
            &entry.path,
            if entry.mode & 0o040000 != 0 {
                entry.mode as u32
            } else {
                0o100000 | entry.mode as u32
            },
            &entry.bytes,
        )?;
    }

    write_cpio_entry(&mut output, inode + 1, "TRAILER!!!", 0, &[])?;
    while output.len() % 4 != 0 {
        output.push(0);
    }
    Ok(output)
}

fn write_cpio_entry(
    output: &mut Vec<u8>,
    inode: u32,
    path: &str,
    mode: u32,
    bytes: &[u8],
) -> anyhow::Result<()> {
    write!(output, "070701")?;
    for value in [
        inode,
        mode,
        0,
        0,
        1,
        RPM_TIMESTAMP,
        bytes.len() as u32,
        0,
        0,
        0,
        0,
        path.len() as u32 + 1,
        0,
    ] {
        write!(output, "{value:08x}")?;
    }
    output.extend_from_slice(path.as_bytes());
    output.push(0);
    while output.len() % 4 != 0 {
        output.push(0);
    }
    output.extend_from_slice(bytes);
    while output.len() % 4 != 0 {
        output.push(0);
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

fn gzip(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes)?;
    Ok(encoder.finish()?)
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
