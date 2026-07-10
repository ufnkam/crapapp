use anyhow::{Context, Result, bail};
use cargo_metadata::{MetadataCommand, TargetKind};
use std::path::PathBuf;

pub struct CargoPackage {
    pub name: String,
    pub version: String,
    pub binaries: Vec<String>,
}

impl CargoPackage {
    pub fn load() -> Result<Self> {
        let manifest_path = PathBuf::from("Cargo.toml");
        let mut command = MetadataCommand::new();
        command.no_deps();

        let manifest_path = if manifest_path.is_file() {
            let manifest_path =
                std::fs::canonicalize(&manifest_path).context("failed to resolve Cargo.toml")?;
            command.manifest_path(&manifest_path);
            Some(manifest_path)
        } else {
            None
        };

        let metadata = command.exec().context("failed to read cargo metadata")?;
        let root_package = match manifest_path {
            Some(manifest_path) => metadata
                .packages
                .iter()
                .find(|package| package.manifest_path.as_std_path() == manifest_path)
                .context("failed to find current cargo package")?,
            None => metadata
                .root_package()
                .context("failed to find root cargo package")?,
        };

        let binaries = root_package
            .targets
            .iter()
            .filter(|target| target.kind.contains(&TargetKind::Bin))
            .map(|target| target.name.to_string())
            .collect::<Vec<_>>();

        if binaries.is_empty() {
            bail!("cargo package does not define any binary targets");
        }

        Ok(Self {
            name: root_package.name.to_string(),
            version: root_package.version.to_string(),
            binaries,
        })
    }
}
