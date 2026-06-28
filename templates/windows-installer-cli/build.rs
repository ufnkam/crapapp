#![allow(unused_macros)]

use std::{env, fs, path::Path};

#[allow(dead_code)]
const PAYLOAD_KEY: u8 = 0xa5;

macro_rules! crapapp_template_payload_source {
    () => {
        "static PAYLOAD: &[PayloadEntry] = &[];\n"
    };
}

const SETUP_ICON: &str = "assets/install.ico";
const SETUP_MANIFEST: &str = r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
    <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
        <security>
            <requestedPrivileges>
                <requestedExecutionLevel level="asInvoker" uiAccess="false" />
            </requestedPrivileges>
        </security>
    </trustInfo>
</assembly>
"#;

fn main() {
    println!("cargo:rerun-if-changed={SETUP_ICON}");
    println!("cargo:rerun-if-changed=../assets/install.ico");

    let out_dir = env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);
    fs::write(
        out_dir.join("payload.rs"),
        crapapp_template_payload_source!(),
    )
    .unwrap();

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let icon_path = {
            let generated_icon = Path::new(&manifest_dir).join(SETUP_ICON);

            if generated_icon.is_file() {
                generated_icon
            } else {
                Path::new(&manifest_dir).join("../assets/install.ico")
            }
        };
        let icon_path = icon_path
            .to_str()
            .expect("installer icon path must be valid UTF-8");

        if !Path::new(icon_path).is_file() {
            panic!("installer icon does not exist: {icon_path}");
        }

        println!("cargo:warning=embedding installer icon from {icon_path}");

        let resource_path = out_dir.join("setup.rc");
        fs::write(
            &resource_path,
            format!(
                "#pragma code_page(65001)\n1 ICON \"{}\"\n1 24\n{{\n{}\n}}\n",
                rc_string(icon_path),
                SETUP_MANIFEST
                    .lines()
                    .map(|line| format!("\" {} \"", rc_string(line.trim())))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
        )
        .unwrap();

        let resource_path = resource_path
            .to_str()
            .expect("setup resource path must be valid UTF-8");
        let mut resource = winresource::WindowsResource::new();
        resource.set_resource_file(resource_path);
        resource.compile().unwrap();
    }
}

fn rc_string(value: &str) -> String {
    let mut escaped = String::new();

    for chr in value.chars() {
        match chr {
            '"' => escaped.push_str("\"\""),
            '\'' => escaped.push_str("\\'"),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\t' => escaped.push_str("\\t"),
            '\r' => escaped.push_str("\\r"),
            _ => escaped.push(chr),
        }
    }

    escaped
}

#[allow(dead_code)]
fn encode_payload(bytes: &[u8]) -> Vec<u8> {
    bytes.iter().map(|byte| byte ^ PAYLOAD_KEY).collect()
}
