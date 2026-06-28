use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IconMapping {
    pub binary: String,
    pub source: String,
}

pub fn validate_icons(
    platform: &str,
    icons: &[IconMapping],
    binary_names: &[String],
) -> Result<()> {
    if platform != "windows" && !icons.is_empty() {
        bail!("{platform} icons are not supported");
    }

    for icon in icons {
        validate_icon(icon, binary_names)?;
    }

    Ok(())
}

fn validate_icon(icon: &IconMapping, binary_names: &[String]) -> Result<()> {
    if !binary_names.iter().any(|binary| binary == &icon.binary) {
        bail!(
            "windows icon references unknown binary {}. Known binaries: {}",
            icon.binary,
            binary_names.join(", ")
        );
    }

    let source = Path::new(&icon.source);

    if !source.is_file() {
        bail!("windows icon source {} does not exist", source.display());
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
    let (width, height) = svg_size(&contents).ok_or_else(|| {
        anyhow::anyhow!(
            "windows svg icon {} must define width/height or viewBox",
            source.display()
        )
    })?;

    if (width - height).abs() > f64::EPSILON {
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

fn svg_size(contents: &str) -> Option<(f64, f64)> {
    let view_box =
        svg_attribute(contents, "viewBox").or_else(|| svg_attribute(contents, "viewbox"));

    if let Some(view_box) = view_box {
        let values = view_box
            .split(|character: char| character.is_ascii_whitespace() || character == ',')
            .filter(|value| !value.is_empty())
            .map(str::parse::<f64>)
            .collect::<Result<Vec<_>, _>>()
            .ok()?;

        if values.len() == 4 {
            return Some((values[2], values[3]));
        }
    }

    let width = svg_length(svg_attribute(contents, "width")?)?;
    let height = svg_length(svg_attribute(contents, "height")?)?;
    Some((width, height))
}

fn svg_attribute<'a>(contents: &'a str, name: &str) -> Option<&'a str> {
    let start = contents.find(&format!("{name}="))? + name.len() + 1;
    let quote = contents[start..].chars().next()?;

    if quote != '"' && quote != '\'' {
        return None;
    }

    let value_start = start + quote.len_utf8();
    let value_end = contents[value_start..].find(quote)?;
    Some(&contents[value_start..value_start + value_end])
}

fn svg_length(value: &str) -> Option<f64> {
    value.trim().trim_end_matches("px").parse().ok()
}

fn is_windows_icon_size(size: f64) -> bool {
    [16.0, 24.0, 32.0, 48.0, 64.0, 128.0, 256.0]
        .iter()
        .any(|allowed| (size - allowed).abs() < f64::EPSILON)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_icon_is_valid_for_a_known_binary() {
        let icon_path = std::env::temp_dir().join(format!(
            "cargo-crapapp-test-icon-{}.svg",
            std::process::id()
        ));
        fs::write(&icon_path, r#"<svg viewBox="0 0 256 256" />"#)
            .expect("failed to write test icon");

        let icon = IconMapping {
            binary: "example".to_owned(),
            source: icon_path.display().to_string(),
        };
        validate_icon(&icon, &["example".to_owned()]).expect("failed to validate icon");

        let _ = fs::remove_file(icon_path);
    }

    #[test]
    fn windows_icon_rejects_unknown_binary() {
        let icon = IconMapping {
            binary: "missing".to_owned(),
            source: "assets/icon.svg".to_owned(),
        };

        let error =
            validate_icon(&icon, &["example".to_owned()]).expect_err("unknown binary should fail");

        assert!(error.to_string().contains("unknown binary missing"));
    }

    #[test]
    fn windows_svg_icon_rejects_non_standard_size() {
        let icon_path = std::env::temp_dir().join(format!(
            "cargo-crapapp-test-icon-bad-size-{}.svg",
            std::process::id()
        ));
        fs::write(&icon_path, r#"<svg viewBox="0 0 403.48 403.48" />"#)
            .expect("failed to write test icon");

        let icon = IconMapping {
            binary: "example".to_owned(),
            source: icon_path.display().to_string(),
        };
        let error =
            validate_icon(&icon, &["example".to_owned()]).expect_err("bad size should fail");

        assert!(error.to_string().contains("must be one of"));

        let _ = fs::remove_file(icon_path);
    }
}
