# cargo-crapapp

Cargo plugin for bundling multiplatform Rust apps from `CRAP.toml`.

It exists because some enterprise environments, especially in banking, are very
good at inventing security rituals and very bad at understanding what is
actually secure. Sometimes Artifactory is missing, nobody owns Linux package
distribution, Windows packaging tools are blocked or impractical, and the only
thing left is a small tool that does the boring thing directly.

The goal is simple: read a manifest, build Rust binaries for configured
targets, and produce something a user can install without negotiating with five
teams and a spreadsheet.

## Status

### Finished

- Basic `setup.exe` generation for Windows from the CLI.
- Cargo package and feature selection from `CRAP.toml`.
- Windows payload embedding into `setup.exe`.
- Embedded `uninstall.exe`.
- Per-user Windows uninstall registry entry.
- Per-user `PATH` updates through `HKCU\Environment`.
- `inspect` command with text and JSON output.

### Unfinished

- Building self-contained apps for macOS.
- Building self-contained apps for Linux.
- Real packaging formats for Linux/macOS.
- Proper end-to-end tests across all supported targets.

### Planned

- Installation and uninstallation GUI for Windows.
- Windows icons.
- Signing Windows apps.
- Better output directory structure.
- More validation around installer paths and payload layout.

## Example

```toml
[cargo]
packages = ["example"]
features = []

[windows]
toolchains = [
    "x86_64-pc-windows-gnu",
]
install_path = "$INSTALLPATH"
files = [
    { source = "Cargo.toml", destination = "manifests/Cargo.toml" },
]
```

Inspect the generated build manifest:

```sh
cargo crapapp inspect
cargo crapapp inspect --output json
```

Build configured targets:

```sh
cargo crapapp build
```

Windows output currently lands in:

```text
.crapapp_build/windows/<toolchain>/setup.exe
```

## Why So Small?

Because this tool is not trying to become another sacred enterprise platform.
It should stay boring, inspectable, and easy to delete when something better is
available.

The design bias is:

- no mystery services
- no central server
- no required admin registry writes
- no shell profile hacks
- no pretending that packaging is more magical than copying files carefully

## Should You Use It?

Probably not.

This is young software with a stupid name and sharp edges. If you use it in
production and it ruins your day, that is between you, your incident process,
and whatever cursed change-management ritual your company worships.

But if you are trapped in a place where normal packaging options are blocked,
broken, or guarded by people who think WiX is a security vulnerability because
it has an `x` in the name, then maybe this little bastard is useful.
