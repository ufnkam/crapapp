#[cfg(unix)]
use std::fs;
use std::path::Path;

use anyhow::{Context, bail};

pub fn icon_file_name(source: &str) -> anyhow::Result<&str> {
    let source_path = Path::new(source);
    let Some(file_name) = source_path.file_name() else {
        bail!(
            "display_icon source {} must point to a file",
            source_path.display()
        );
    };
    let Some(file_name) = file_name.to_str() else {
        bail!(
            "display_icon source {} must use a UTF-8 file name",
            source_path.display()
        );
    };

    Ok(file_name)
}

pub fn path_has_extension(path: &Path, allowed_extensions: &[&str]) -> bool {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };

    for allowed_extension in allowed_extensions {
        if extension.eq_ignore_ascii_case(allowed_extension) {
            return true;
        }
    }

    false
}

#[cfg(unix)]
pub fn set_executable_permissions(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .with_context(|| format!("failed to read permissions for {}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .with_context(|| format!("failed to set executable permissions on {}", path.display()))?;

    Ok(())
}

#[cfg(not(unix))]
pub fn set_executable_permissions(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}
