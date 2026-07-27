use std::fs;
use std::io::Cursor;

use anyhow::Context;

use super::builder::PackageFile;

pub(super) fn build(files: &[PackageFile]) -> anyhow::Result<Vec<u8>> {
    let mut builder = cab::CabinetBuilder::new();
    {
        let folder = builder.add_folder(cab::CompressionType::MsZip);
        for file in files {
            folder.add_file(file.cabinet_name.clone());
        }
    }

    let mut writer = builder
        .build(Cursor::new(Vec::new()))
        .context("failed to create MSI cabinet")?;
    while let Some(mut file_writer) = writer
        .next_file()
        .context("failed to create MSI cabinet file")?
    {
        let source = files
            .iter()
            .find(|file| file.cabinet_name == file_writer.file_name())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "failed to resolve MSI cabinet source {}",
                    file_writer.file_name()
                )
            })?;
        let mut input = fs::File::open(&source.source)
            .with_context(|| format!("failed to read {}", source.source.display()))?;
        std::io::copy(&mut input, &mut file_writer).with_context(|| {
            format!(
                "failed to write MSI cabinet file {}",
                file_writer.file_name()
            )
        })?;
    }

    Ok(writer.finish()?.into_inner())
}
