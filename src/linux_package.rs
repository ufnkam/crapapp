use anyhow::bail;

pub fn package_name(name: &str) -> String {
    let mut package = String::new();
    for character in name.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.') {
            package.push(character);
        } else {
            package.push('-');
        }
    }

    let package = package.trim_matches(['-', '.']).to_owned();
    if package.is_empty() {
        "app".to_owned()
    } else {
        package
    }
}

pub fn deb_architecture(target: &str) -> anyhow::Result<&'static str> {
    match target {
        "x86_64-unknown-linux-gnu" => Ok("amd64"),
        _ => bail!("deb architecture mapping for target {target} is not supported yet"),
    }
}

pub fn rpm_architecture(target: &str) -> anyhow::Result<&'static str> {
    match target {
        "x86_64-unknown-linux-gnu" => Ok("x86_64"),
        _ => bail!("rpm architecture mapping for target {target} is not supported yet"),
    }
}

pub fn aur_architecture(target: &str) -> anyhow::Result<&'static str> {
    match target {
        "x86_64-unknown-linux-gnu" => Ok("x86_64"),
        _ => bail!("AUR architecture mapping for target {target} is not supported yet"),
    }
}
