use anyhow::{Context, Result, bail};
use cargo_metadata::{MetadataCommand, TargetKind};
use std::path::Path;

pub struct CargoPackage {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub license_file: Option<String>,
    pub binaries: Vec<String>,
}

impl CargoPackage {
    pub fn load(selected_packages: &[String]) -> Result<Self> {
        Self::load_from_manifest_path(Path::new("Cargo.toml"), selected_packages)
    }

    fn load_from_manifest_path(manifest_path: &Path, selected_packages: &[String]) -> Result<Self> {
        if selected_packages.len() > 1 {
            bail!(
                "CRAP.toml build.packages must select exactly one app package; found {}",
                selected_packages.len()
            );
        }

        let mut command = MetadataCommand::new();
        command.no_deps();

        let manifest_path = if manifest_path.is_file() {
            let manifest_path =
                std::fs::canonicalize(manifest_path).context("failed to resolve Cargo.toml")?;
            command.manifest_path(&manifest_path);
            Some(manifest_path)
        } else {
            None
        };

        let metadata = command.exec().context("failed to read cargo metadata")?;
        let root_package = match selected_packages.first() {
            Some(selected_package) => metadata
                .packages
                .iter()
                .find(|package| package.name == selected_package.as_str())
                .with_context(|| {
                    format!(
                        "failed to find selected cargo package {selected_package}; \
                         ensure build.packages names a package in this workspace"
                    )
                })?,
            None => match manifest_path {
                Some(manifest_path) => metadata
                    .packages
                    .iter()
                    .find(|package| package.manifest_path.as_std_path() == manifest_path)
                    .context("failed to find current cargo package")?,
                None => metadata
                    .root_package()
                    .context("failed to find root cargo package; set build.packages when bundling from a virtual workspace")?,
            },
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
            description: root_package.description.clone(),
            homepage: root_package.homepage.clone(),
            license: root_package.license.clone(),
            license_file: root_package.license_file.as_ref().map(ToString::to_string),
            binaries,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::CargoPackage;
    use std::path::Path;

    #[test]
    fn loads_selected_package_from_virtual_workspace() {
        let package = CargoPackage::load_from_manifest_path(
            Path::new("example/Cargo.toml"),
            &["example".to_owned()],
        )
        .expect("selected workspace package should load");

        assert_eq!(package.name, "example");
        assert!(package.binaries.contains(&"example".to_owned()));
    }
}
