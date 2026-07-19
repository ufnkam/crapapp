use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use flate2::Compression;
use flate2::write::{GzEncoder, ZlibEncoder};
use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use sha1::{Digest, Sha1};

const XAR_MAGIC: u32 = 0x7861_7221;
const XAR_HEADER_SIZE: u16 = 28;
const XAR_VERSION: u16 = 1;
const XAR_SHA1: u32 = 1;
const SHA1_LEN: u64 = 20;
const TIMESTAMP: &str = "2000-01-01T00:00:00Z";
const CPIO_TIMESTAMP: u64 = 946_684_800;

#[derive(Debug, Clone)]
pub struct FileSpec {
    pub src: PathBuf,
    pub dest: String,
}

#[derive(Debug, Clone)]
pub struct PkgSpec {
    pub name: String,
    pub display_name: String,
    pub identifier: String,
    pub version: String,
    pub install_path: String,
    pub license: Option<Vec<u8>>,
    pub files: Vec<FileSpec>,
}

pub fn build(spec: &PkgSpec, output: &Path) -> anyhow::Result<()> {
    validate(spec)?;

    let nodes = payload_nodes(&spec.files)?;
    let payload = gzip(&cpio_payload(&nodes)?)?;
    let package_info = package_info(spec, &nodes)?;
    let distribution = distribution(spec, install_kbytes(&nodes))?;
    let bom = placeholder_bom();
    let component_name = format!("{}.pkg", spec.name);
    let mut entries = vec![
        XarEntry::directory(
            component_name,
            vec![
                XarEntry::file("Bom", bom),
                XarEntry::file("Payload", payload),
                XarEntry::file("PackageInfo", package_info),
            ],
        ),
        XarEntry::file("Distribution", distribution),
    ];
    if let Some(license) = &spec.license {
        entries.push(XarEntry::directory(
            "Resources",
            vec![XarEntry::file("License.txt", license.clone())],
        ));
    }

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(output, xar_archive(&entries)?)
        .with_context(|| format!("failed to write {}", output.display()))?;

    Ok(())
}

fn validate(spec: &PkgSpec) -> anyhow::Result<()> {
    if spec.files.is_empty() {
        bail!("macOS pkg has no files to package");
    }
    if !spec.install_path.starts_with('/') {
        bail!(
            "macOS pkg install_path {} must be absolute",
            spec.install_path
        );
    }

    let mut destinations = BTreeSet::new();
    for file in &spec.files {
        if file
            .dest
            .split('/')
            .any(|part| part.is_empty() || part == "..")
        {
            bail!(
                "macOS pkg destination {} must be a relative slash path",
                file.dest
            );
        }
        if !destinations.insert(file.dest.as_str()) {
            bail!("macOS pkg destination {} is duplicated", file.dest);
        }
    }

    Ok(())
}

#[derive(Debug)]
enum PayloadNode {
    Directory,
    File { bytes: Vec<u8>, mode: u32 },
}

fn payload_nodes(files: &[FileSpec]) -> anyhow::Result<BTreeMap<String, PayloadNode>> {
    let mut nodes = BTreeMap::new();

    for file in files {
        for ancestor in path_ancestors(&file.dest) {
            nodes.insert(ancestor.to_owned(), PayloadNode::Directory);
        }

        let bytes = fs::read(&file.src)
            .with_context(|| format!("failed to read {}", file.src.display()))?;
        let mode = if is_executable(&file.src)? {
            0o755
        } else {
            0o644
        };
        nodes.insert(file.dest.clone(), PayloadNode::File { bytes, mode });
    }

    Ok(nodes)
}

fn path_ancestors(path: &str) -> impl Iterator<Item = &str> {
    path.char_indices()
        .filter(|(_, character)| *character == '/')
        .map(|(index, _)| &path[..index])
}

#[cfg(unix)]
fn is_executable(path: &Path) -> anyhow::Result<bool> {
    use std::os::unix::fs::PermissionsExt;

    let mode = fs::metadata(path)
        .with_context(|| format!("failed to read permissions for {}", path.display()))?
        .permissions()
        .mode();

    Ok(mode & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> anyhow::Result<bool> {
    Ok(false)
}

fn cpio_payload(nodes: &BTreeMap<String, PayloadNode>) -> anyhow::Result<Vec<u8>> {
    let mut payload = Vec::new();
    let mut inode = 1;

    for (path, node) in nodes {
        inode += 1;
        match node {
            PayloadNode::Directory => {
                write_cpio_entry(&mut payload, inode, path, 0o040755, 2, CPIO_TIMESTAMP, &[])?
            }
            PayloadNode::File { bytes, mode } => write_cpio_entry(
                &mut payload,
                inode,
                path,
                0o100000 | mode,
                1,
                CPIO_TIMESTAMP,
                bytes,
            )?,
        }
    }

    write_cpio_entry(
        &mut payload,
        inode + 1,
        "TRAILER!!!",
        0,
        1,
        CPIO_TIMESTAMP,
        &[],
    )?;
    while payload.len() % 512 != 0 {
        payload.push(0);
    }

    Ok(payload)
}

fn write_cpio_entry(
    output: &mut Vec<u8>,
    inode: u32,
    name: &str,
    mode: u32,
    links: u32,
    mtime: u64,
    data: &[u8],
) -> anyhow::Result<()> {
    if name.as_bytes().len() + 1 > 0o777777 {
        bail!("cpio path {name} is too long");
    }
    if data.len() > 0o77777777777 {
        bail!("cpio file {name} is too large");
    }

    write!(output, "070707")?;
    write_octal(output, 0, 6)?;
    write_octal(output, inode as u64, 6)?;
    write_octal(output, mode as u64, 6)?;
    write_octal(output, 0, 6)?;
    write_octal(output, 0, 6)?;
    write_octal(output, links as u64, 6)?;
    write_octal(output, 0, 6)?;
    write_octal(output, mtime, 11)?;
    write_octal(output, (name.as_bytes().len() + 1) as u64, 6)?;
    write_octal(output, data.len() as u64, 11)?;
    output.extend_from_slice(name.as_bytes());
    output.push(0);
    output.extend_from_slice(data);

    Ok(())
}

fn write_octal(output: &mut Vec<u8>, value: u64, width: usize) -> anyhow::Result<()> {
    let octal = format!("{value:0width$o}");
    if octal.len() > width {
        bail!("octal value {value} does not fit in {width} bytes");
    }
    output.extend_from_slice(octal.as_bytes());
    Ok(())
}

fn gzip(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes)?;
    Ok(encoder.finish()?)
}

fn zlib(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes)?;
    Ok(encoder.finish()?)
}

fn package_info(spec: &PkgSpec, nodes: &BTreeMap<String, PayloadNode>) -> anyhow::Result<Vec<u8>> {
    let install_kbytes = install_kbytes(nodes);
    let number_of_files = nodes.len() + 1;
    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 4);
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;

    let mut pkg_info = BytesStart::new("pkg-info");
    pkg_info.push_attribute(("format-version", "2"));
    pkg_info.push_attribute(("identifier", spec.identifier.as_str()));
    pkg_info.push_attribute(("version", spec.version.as_str()));
    pkg_info.push_attribute(("install-location", spec.install_path.as_str()));
    pkg_info.push_attribute(("auth", "root"));
    pkg_info.push_attribute(("relocatable", "false"));
    pkg_info.push_attribute(("overwrite-permissions", "true"));
    pkg_info.push_attribute(("postinstall-action", "none"));
    writer.write_event(Event::Start(pkg_info))?;

    let mut payload = BytesStart::new("payload");
    let number_of_files = number_of_files.to_string();
    let install_kbytes = install_kbytes.to_string();
    payload.push_attribute(("numberOfFiles", number_of_files.as_str()));
    payload.push_attribute(("installKBytes", install_kbytes.as_str()));
    writer.write_event(Event::Empty(payload))?;

    for element in [
        "bundle-version",
        "upgrade-bundle",
        "update-bundle",
        "atomic-update-bundle",
        "strict-identifier",
        "relocate",
    ] {
        writer.write_event(Event::Empty(BytesStart::new(element)))?;
    }

    writer.write_event(Event::End(BytesEnd::new("pkg-info")))?;
    Ok(writer.into_inner())
}

fn distribution(spec: &PkgSpec, install_kbytes: u64) -> anyhow::Result<Vec<u8>> {
    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 4);
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;

    let mut root = BytesStart::new("installer-gui-script");
    root.push_attribute(("minSpecVersion", "2"));
    writer.write_event(Event::Start(root))?;
    text_element(&mut writer, "title", &spec.display_name)?;
    if spec.license.is_some() {
        let mut license = BytesStart::new("license");
        license.push_attribute(("file", "License.txt"));
        license.push_attribute(("mime-type", "text/plain"));
        writer.write_event(Event::Empty(license))?;
    }

    let mut domains = BytesStart::new("domains");
    domains.push_attribute(("enable_localSystem", "true"));
    writer.write_event(Event::Empty(domains))?;

    let mut options = BytesStart::new("options");
    options.push_attribute(("customize", "never"));
    options.push_attribute(("require-scripts", "false"));
    options.push_attribute(("hostArchitectures", "x86_64,arm64"));
    writer.write_event(Event::Empty(options))?;

    writer.write_event(Event::Start(BytesStart::new("choices-outline")))?;
    let mut default_line = BytesStart::new("line");
    default_line.push_attribute(("choice", "default"));
    writer.write_event(Event::Start(default_line))?;
    let mut package_line = BytesStart::new("line");
    package_line.push_attribute(("choice", spec.identifier.as_str()));
    writer.write_event(Event::Empty(package_line))?;
    writer.write_event(Event::End(BytesEnd::new("line")))?;
    writer.write_event(Event::End(BytesEnd::new("choices-outline")))?;

    let mut default_choice = BytesStart::new("choice");
    default_choice.push_attribute(("id", "default"));
    writer.write_event(Event::Empty(default_choice))?;

    let mut package_choice = BytesStart::new("choice");
    package_choice.push_attribute(("id", spec.identifier.as_str()));
    package_choice.push_attribute(("visible", "false"));
    writer.write_event(Event::Start(package_choice))?;
    let mut choice_ref = BytesStart::new("pkg-ref");
    choice_ref.push_attribute(("id", spec.identifier.as_str()));
    writer.write_event(Event::Empty(choice_ref))?;
    writer.write_event(Event::End(BytesEnd::new("choice")))?;

    let mut pkg_ref = BytesStart::new("pkg-ref");
    let install_kbytes = install_kbytes.to_string();
    pkg_ref.push_attribute(("id", spec.identifier.as_str()));
    pkg_ref.push_attribute(("version", spec.version.as_str()));
    pkg_ref.push_attribute(("onConclusion", "none"));
    pkg_ref.push_attribute(("installKBytes", install_kbytes.as_str()));
    pkg_ref.push_attribute(("updateKBytes", "0"));
    writer.write_event(Event::Start(pkg_ref))?;
    writer.write_event(Event::Text(BytesText::new(&format!("#{}.pkg", spec.name))))?;
    writer.write_event(Event::End(BytesEnd::new("pkg-ref")))?;

    writer.write_event(Event::End(BytesEnd::new("installer-gui-script")))?;
    Ok(writer.into_inner())
}

fn text_element(writer: &mut Writer<Vec<u8>>, name: &str, value: &str) -> anyhow::Result<()> {
    writer.write_event(Event::Start(BytesStart::new(name)))?;
    writer.write_event(Event::Text(BytesText::new(value)))?;
    writer.write_event(Event::End(BytesEnd::new(name)))?;
    Ok(())
}

fn install_kbytes(nodes: &BTreeMap<String, PayloadNode>) -> u64 {
    let mut blocks = 0;
    for node in nodes.values() {
        blocks += match node {
            PayloadNode::Directory => 1,
            PayloadNode::File { bytes, .. } => (bytes.len() as u64).div_ceil(512),
        };
    }
    blocks.div_ceil(2)
}

fn placeholder_bom() -> Vec<u8> {
    Vec::new()
}

struct XarEntry {
    name: String,
    kind: XarEntryKind,
}

enum XarEntryKind {
    Directory { children: Vec<XarEntry> },
    File { bytes: Vec<u8> },
}

impl XarEntry {
    fn directory(name: impl Into<String>, children: Vec<XarEntry>) -> Self {
        Self {
            name: name.into(),
            kind: XarEntryKind::Directory { children },
        }
    }

    fn file(name: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            kind: XarEntryKind::File { bytes },
        }
    }
}

struct XarFileData<'a> {
    bytes: &'a [u8],
    offset: u64,
}

fn xar_archive(entries: &[XarEntry]) -> anyhow::Result<Vec<u8>> {
    let mut file_data = Vec::new();
    collect_xar_file_data(entries, &mut file_data);

    let mut offset = SHA1_LEN;
    for data in &mut file_data {
        data.offset = offset;
        offset += data.bytes.len() as u64;
    }

    let toc = xar_toc(entries, &file_data)?;
    let toc_zlib = zlib(&toc)?;
    let toc_checksum = sha1(&toc_zlib);

    let mut archive = Vec::new();
    archive.extend_from_slice(&XAR_MAGIC.to_be_bytes());
    archive.extend_from_slice(&XAR_HEADER_SIZE.to_be_bytes());
    archive.extend_from_slice(&XAR_VERSION.to_be_bytes());
    archive.extend_from_slice(&(toc_zlib.len() as u64).to_be_bytes());
    archive.extend_from_slice(&(toc.len() as u64).to_be_bytes());
    archive.extend_from_slice(&XAR_SHA1.to_be_bytes());
    archive.extend_from_slice(&toc_zlib);
    archive.extend_from_slice(&toc_checksum);
    for data in file_data {
        archive.extend_from_slice(data.bytes);
    }

    Ok(archive)
}

fn collect_xar_file_data<'a>(entries: &'a [XarEntry], data: &mut Vec<XarFileData<'a>>) {
    for entry in entries {
        match &entry.kind {
            XarEntryKind::Directory { children } => collect_xar_file_data(children, data),
            XarEntryKind::File { bytes } => data.push(XarFileData { bytes, offset: 0 }),
        }
    }
}

fn xar_toc(entries: &[XarEntry], file_data: &[XarFileData<'_>]) -> anyhow::Result<Vec<u8>> {
    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 4);
    let mut id = 1;
    let mut data_index = 0;

    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    writer.write_event(Event::Start(BytesStart::new("xar")))?;
    writer.write_event(Event::Start(BytesStart::new("toc")))?;
    text_element(&mut writer, "creation-time", TIMESTAMP)?;

    let mut checksum = BytesStart::new("checksum");
    checksum.push_attribute(("style", "sha1"));
    writer.write_event(Event::Start(checksum))?;
    text_element(&mut writer, "offset", "0")?;
    text_element(&mut writer, "size", &SHA1_LEN.to_string())?;
    writer.write_event(Event::End(BytesEnd::new("checksum")))?;

    for entry in entries {
        write_xar_entry(&mut writer, entry, file_data, &mut data_index, &mut id)?;
    }

    writer.write_event(Event::End(BytesEnd::new("toc")))?;
    writer.write_event(Event::End(BytesEnd::new("xar")))?;

    Ok(writer.into_inner())
}

fn write_xar_entry(
    writer: &mut Writer<Vec<u8>>,
    entry: &XarEntry,
    file_data: &[XarFileData<'_>],
    data_index: &mut usize,
    id: &mut u64,
) -> anyhow::Result<()> {
    let entry_id = id.to_string();
    *id += 1;
    let mut file = BytesStart::new("file");
    file.push_attribute(("id", entry_id.as_str()));
    writer.write_event(Event::Start(file))?;

    match &entry.kind {
        XarEntryKind::Directory { children } => {
            text_element(writer, "type", "directory")?;
            text_element(writer, "name", &entry.name)?;
            for child in children {
                write_xar_entry(writer, child, file_data, data_index, id)?;
            }
        }
        XarEntryKind::File { bytes } => {
            let data = &file_data[*data_index];
            *data_index += 1;
            writer.write_event(Event::Start(BytesStart::new("data")))?;
            text_element(writer, "length", &bytes.len().to_string())?;
            text_element(writer, "offset", &data.offset.to_string())?;
            text_element(writer, "size", &bytes.len().to_string())?;
            let mut encoding = BytesStart::new("encoding");
            encoding.push_attribute(("style", "application/octet-stream"));
            writer.write_event(Event::Empty(encoding))?;
            let checksum = hex(&sha1(bytes));
            checksum_element(writer, "extracted-checksum", &checksum)?;
            checksum_element(writer, "archived-checksum", &checksum)?;
            writer.write_event(Event::End(BytesEnd::new("data")))?;
            text_element(writer, "type", "file")?;
            text_element(writer, "name", &entry.name)?;
        }
    }

    writer.write_event(Event::End(BytesEnd::new("file")))?;
    Ok(())
}

fn checksum_element(writer: &mut Writer<Vec<u8>>, name: &str, value: &str) -> anyhow::Result<()> {
    let mut checksum = BytesStart::new(name);
    checksum.push_attribute(("style", "sha1"));
    writer.write_event(Event::Start(checksum))?;
    writer.write_event(Event::Text(BytesText::new(value)))?;
    writer.write_event(Event::End(BytesEnd::new(name)))?;
    Ok(())
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
    use super::{cpio_payload, payload_nodes};
    use crate::macos_pkg::FileSpec;
    use std::fs;

    #[test]
    fn cpio_payload_contains_files_and_trailer() {
        let temp_dir =
            std::env::temp_dir().join(format!("cargo-crapapp-cpio-{}", std::process::id()));
        let file = temp_dir.join("example");

        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        fs::write(&file, b"hello").expect("file should be written");

        let nodes = payload_nodes(&[FileSpec {
            src: file,
            dest: "usr/local/bin/example".to_owned(),
        }])
        .expect("nodes should be created");
        let payload = cpio_payload(&nodes).expect("payload should be created");

        assert!(
            payload
                .windows("usr/local/bin/example".len())
                .any(|window| { window == b"usr/local/bin/example" })
        );
        assert!(
            payload
                .windows("TRAILER!!!".len())
                .any(|window| { window == b"TRAILER!!!" })
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
