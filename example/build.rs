use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build_assets/icon.ico");
    println!("cargo:rerun-if-changed=build_assets/first.ico");
    println!("cargo:rerun-if-changed=build_assets/second.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        compile_resource("first-bin", "build_assets/first.ico");
        compile_resource("second-bin", "build_assets/second.ico");
    }
}

fn compile_resource(bin_name: &str, icon: &str) {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR must be set"));
    let rc_path = out_dir.join(format!("{bin_name}.rc"));
    let resource_path = out_dir.join(format!("{bin_name}.o"));
    let windres = std::env::var("WINDRES").unwrap_or_else(|_| {
        let target = std::env::var("TARGET").unwrap_or_default();

        if target == "x86_64-pc-windows-gnu" {
            "x86_64-w64-mingw32-windres".to_owned()
        } else if target == "i686-pc-windows-gnu" {
            "i686-w64-mingw32-windres".to_owned()
        } else {
            "windres".to_owned()
        }
    });

    std::fs::write(
        &rc_path,
        format!(r#"1 ICON "{}""#, Path::new(icon).display()),
    )
    .expect("failed to write resource script");

    let mut command = Command::new(windres);
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("x86_64") {
        command.args(["--target", "pe-x86-64"]);
    }

    let status = command
        .arg(format!(
            "-I{}",
            std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set")
        ))
        .arg(&rc_path)
        .arg(&resource_path)
        .status()
        .expect("failed to run windres");

    if !status.success() {
        panic!("failed to compile Windows resource for {bin_name}");
    }

    println!("cargo:warning=embedding {icon} for {bin_name}");
    println!(
        "cargo:rustc-link-arg-bin={bin_name}={}",
        resource_path.display()
    );
}
