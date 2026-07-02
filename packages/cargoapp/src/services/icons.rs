use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

pub fn validate_display_icon(source: Option<&str>) -> Result<()> {
    let Some(source) = source else {
        return Ok(());
    };
    let source = Path::new(source);

    if !source.is_file() {
        bail!(
            "windows display_icon source {} does not exist",
            source.display()
        );
    }

    if source
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
    {
        validate_windows_svg_icon(source)?;
    }

    Ok(())
}

fn validate_windows_svg_icon(source: &Path) -> Result<()> {
    let contents = fs::read_to_string(source)
        .with_context(|| format!("failed to read svg icon {}", source.display()))?;
    let tree = usvg::Tree::from_str(&contents, &usvg::Options::default())
        .with_context(|| format!("failed to parse svg icon {}", source.display()))?;
    let size = tree.size();
    let width = size.width();
    let height = size.height();

    if (width - height).abs() > f32::EPSILON {
        bail!(
            "windows svg icon {} must be square, got {width}x{height}",
            source.display()
        );
    }

    if !is_windows_icon_size(width) {
        bail!(
            "windows svg icon {} must be one of 16, 24, 32, 48, 64, 128, or 256 px, got {width}",
            source.display()
        );
    }

    Ok(())
}

fn is_windows_icon_size(size: f32) -> bool {
    [16.0, 24.0, 32.0, 48.0, 64.0, 128.0, 256.0]
        .iter()
        .any(|allowed| (size - allowed).abs() < f32::EPSILON)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_icon_is_valid_for_existing_icon() {
        let icon_path = std::env::temp_dir().join(format!(
            "cargo-crapapp-test-icon-{}.svg",
            std::process::id()
        ));
        fs::write(&icon_path, r#"<svg viewBox="0 0 256 256" />"#)
            .expect("failed to write test icon");

        validate_display_icon(Some(&icon_path.display().to_string()))
            .expect("failed to validate display icon");

        let _ = fs::remove_file(icon_path);
    }

    #[test]
    fn display_icon_rejects_missing_source() {
        let error =
            validate_display_icon(Some("missing.svg")).expect_err("missing icon should fail");

        assert!(
            error
                .to_string()
                .contains("source missing.svg does not exist")
        );
    }

    #[test]
    fn missing_display_icon_is_valid() {
        validate_display_icon(None).expect("missing display icon should be valid");
    }

    #[test]
    fn display_icon_rejects_non_standard_svg_size() {
        let icon_path = std::env::temp_dir().join(format!(
            "cargo-crapapp-test-icon-bad-size-{}.svg",
            std::process::id()
        ));
        fs::write(&icon_path, r#"<svg viewBox="0 0 403.48 403.48" />"#)
            .expect("failed to write test icon");

        let error = validate_display_icon(Some(&icon_path.display().to_string()))
            .expect_err("bad size should fail");

        assert!(error.to_string().contains("must be one of"));

        let _ = fs::remove_file(icon_path);
    }
}
