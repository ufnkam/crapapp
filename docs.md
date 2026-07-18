# crapapp 

Crapapp is a cargo plugin for bundling Rust desktop apps from `CRAP.toml` without external
dependencies like wixl or nsis.

This crate ships two things:

- `cargo-crapapp`, a Cargo subcommand for building app installers from
  `CRAP.toml`.
- `libcrapapp`, the library used by the CLI and by generated Windows installer
  projects.

The workspace `example` package is only a local fixture for development and
testing. It is not part of the public API.

# cargo-crapapp

Install the Cargo subcommand:

```sh
cargo install cargo-crapapp
```

Run it from an application crate that has a `CRAP.toml` next to its
`Cargo.toml`:

```sh
cargo crapapp inspect
cargo crapapp inspect --output json
cargo crapapp build
```

### `cargo crapapp inspect`

Reads `CRAP.toml`, validates the configuration, reads Cargo package metadata,
and prints the build manifest that `cargo crapapp build` will use.

Options:

- `--output text`, the default human-readable output.
- `--output json`, machine-readable build manifest output.
- `-o json`, short form for JSON output.

### `cargo crapapp build`

Reads `CRAP.toml`, builds configured Cargo packages for configured platform
targets, and writes the binaries to the configured Cargo target directory.

### `cargo crapapp bundle`

Reads `CRAP.toml`, builds configured Cargo packages for configured platform
targets, collects payload files, and writes generated output under
`.crapapp_build`.

Use `cargo crapapp bundle --no-build` or `cargo crapapp bundle -n` to skip
building configured Cargo packages before bundling existing binaries.

Bundler output currently lands in:

```text
.crapapp_build/windows/<target>/<bundle>/setup.exe
.crapapp_build/macos/<target>/<bundle>/<display-name-or-package-name>.app
```

Linux output and macOS DMG output are not supported yet.

## CRAP.toml quick start

`CRAP.toml` declares what cargo-crapapp should build and how Windows setup
should behave.

```toml
[build]
publisher = "Acme"
display_name = "Acme Launcher"
packages = ["acme-launcher"]
features = ["sqlite"]

[windows]
targets = ["x86_64-pc-windows-gnu"]
install_path = "$INSTALLPATH"
bundle = "gui"
display_icon = "assets/app.ico"
files = [
    { source = "assets", destination = "assets" },
]
eulas = [
    { path = "EULA.txt" },
    { path = "THIRD_PARTY.txt", required = false },
]
associated_files = [
    { path = "$HOMEPATH/Documents/Acme/saves", kind = "directory" },
    { path = "$HOMEPATH/Documents/Acme/eulas", kind = "directory", eula_report = true },
]
shortcuts = [
    { binary = "acme-launcher", name = "Acme Launcher", directory = "Acme" },
]

[macos]
targets = ["aarch64-apple-darwin"]
bundle = "app"
display_icon = "assets/app.icns"
app_binary = "acme-launcher"
files = [
    { source = "assets", destination = "Resources/assets" },
]
```

## Build section

The optional `[build]` section controls Cargo package selection and feature
selection.

```toml
[build]
publisher = "Acme"
display_name = "Acme Launcher"
packages = ["acme-launcher"]
features = ["sqlite", "native-tls"]
```

`packages` maps to Cargo package selection. If it is empty or missing,
cargo-crapapp does not pass package flags to Cargo.

`features` maps to Cargo feature selection. If it is empty or missing,
cargo-crapapp does not pass feature flags to Cargo.

`publisher` is optional. Windows setup writes the uninstall registry
`Publisher` value only when this key is present.

`display_name` is optional. Windows setup uses it for installer text and the
uninstall registry `DisplayName`. If it is missing, the Cargo package name is
used.

## Windows section

Windows is the only platform with generated installable output right now.

```toml
[windows]
targets = ["x86_64-pc-windows-gnu"]
install_path = "$INSTALLPATH"
bundle = "gui"
display_icon = "assets/app.ico"
files = [
    { source = "assets", destination = "assets" },
]
```

Supported Windows targets:

- `x86_64-pc-windows-gnu`
- `x86_64-pc-windows-msvc`
- `aarch64-pc-windows-gnullvm`
- `aarch64-pc-windows-msvc`

MSVC targets require a working MSVC-compatible Windows toolchain: linker,
Windows SDK libraries, and the usual Cargo target configuration. Windows has
that path by default through Visual Studio Build Tools. Linux/macOS can build
MSVC targets only when that toolchain is configured explicitly, for example with
an LLVM/lld-based setup and Windows SDK import libraries. cargo-crapapp does not
set up that cross toolchain for you.

### Windows bundle

`bundle` is optional and defaults to `cli`. It accepts either a single value
or a list.

```toml
[windows]
bundle = "gui"
```

Supported values:

- `cli`, generates a command-line setup executable.
- `gui`, generates a graphical setup executable.

You can build more than one bundle for the same target:

```toml
[windows]
bundle = ["cli", "gui"]
```

Outputs are grouped by bundle name:

```text
.crapapp_build/windows/<target>/cli/setup.exe
.crapapp_build/windows/<target>/gui/setup.exe
```

### Windows install path

`install_path` is optional. If present, relative payload destinations are
prefixed with it in the build manifest.

```toml
[windows]
install_path = "$INSTALLPATH"
```

Variables such as `$INSTALLPATH` stay symbolic in the build manifest and are
resolved by the generated installer at runtime.

Generated Windows setup accepts runtime variables with repeated `--args`
arguments:

```sh
setup.exe --args INSTALLPATH=C:\Users\me\AppData\Local\Acme
setup.exe --args INSTALLPATH=C:\Users\me\AppData\Local\Acme --args ADD_TO_PATH=0
```

`ADD_TO_PATH` defaults to `1`, so executable payload directories are added to the
current user's `PATH` unless explicitly disabled.

### Windows binary directory

`bin_dir` is optional. If omitted, Cargo binaries are installed directly into
`install_path` when `install_path` is present.

```toml
[windows]
install_path = "$INSTALLPATH"
bin_dir = "bin"
```

### Windows payload files

`files` is optional. Each entry copies an extra file or directory into the
payload.

```toml
[windows]
files = [
    { source = "assets", destination = "assets" },
    { source = "config/default.json", destination = "config/default.json" },
]
```

Cargo binaries are added automatically from the selected Cargo package metadata.

### Windows display icon

`display_icon` is optional. It names a PNG or ICO file.

```toml
[windows]
display_icon = "assets/app.ico"
```

The GUI installer uses this icon in its header. Windows uninstall registry
`DisplayIcon` points to the installed app executable. The generated `setup.exe`
itself always uses the cargo-crapapp crab icon.

cargo-crapapp does not modify built application binaries. If the application
executable should have this icon in Explorer or Windows Search, the application
crate must embed it itself, for example from its own `build.rs`.

### Windows EULAs

`eulas` is optional. Each entry can be a path string or an object.

```toml
[windows]
eulas = [
    "EULA.txt",
    { path = "THIRD_PARTY.txt", required = false },
]
```

`required` defaults to `true`. GUI installation and reinstallation show each
EULA on its own screen before settings.

### Windows associated files

`associated_files` is optional. It creates app-owned files or directories during
installation.

```toml
[windows]
associated_files = [
    { path = "$HOMEPATH/Documents/Acme/saves", kind = "directory" },
    { path = "$INSTALLPATH/settings.json", kind = "file" },
]
```

Supported `kind` values:

- `directory`
- `file`

Associated file paths support:

- `$INSTALLPATH`, resolved from the install path selected at runtime.
- `$HOMEPATH`, resolved to the current user's home directory at installer
  runtime.

Associated files are not payload files. They are empty files or directories that
the installed app can use later. The uninstaller can remove them when the user
chooses to remove associated files.

Set `eula_report = true` on an associated file or directory to write a JSON EULA
acceptance report after installation.

```toml
[windows]
associated_files = [
    { path = "$HOMEPATH/Documents/Acme/eulas", kind = "directory", eula_report = true },
]
```

For a directory, the installer writes `eulas.json` inside it. For a file, the
installer writes the report to that file path.

### Windows shortcuts

`shortcuts` is optional. It creates Start Menu `.lnk` files so Windows Search
can find the app by display name. It does not pin anything.

```toml
[windows]
shortcuts = [
    { binary = "acme-launcher", name = "Acme Launcher", directory = "Acme", icon = "build_assets/acme.ico" },
]
```

Fields:

- `binary`, the Cargo binary name without `.exe`.
- `name`, the Start Menu shortcut display name.
- `directory`, optional Start Menu directory.
- `icon`, optional icon file source. The installer copies it next to the app
  binaries and points the shortcut at the installed icon file. If omitted, the
  shortcut uses the target executable icon.

## macOS

The `[macos]` section creates `.app` bundles. Each configured target produces:

```text
.crapapp_build/macos/<target>/<bundle>/<display-name-or-package-name>.app
```

`bundle` is optional and defaults to `app`. It accepts either a single value
or a list, matching the Windows bundle field. The only supported macOS bundle
kind today is `app`.

Application executables are copied into `Contents/MacOS` for the `.app` bundle.
Extra `files` entries are copied to their configured destination relative to
`Contents`, so `destination = "Resources/assets"` writes to
`Example App.app/Contents/Resources/assets`.

`app_binary` is optional. When set, it names the Cargo binary used as the
`.app` launcher through `CFBundleExecutable`. If omitted or empty, cargo-crapapp
falls back to the first executable payload.

The `.app` launcher should be a GUI binary. macOS does not open Terminal when a
Finder-launched app runs a CLI executable, so stdout/stderr are not visible and
stdin is not an interactive prompt. Package CLI tools inside `Contents/MacOS`
only when they are meant to be run directly from a shell, for example:

```sh
"/Applications/Example App.app/Contents/MacOS/example"
```

`display_icon` is optional. The source file is copied into `Contents/Resources`.
When the source is an `.icns` file, `Info.plist` also sets `CFBundleIconFile` so
macOS can use it as the app icon.

DMG output is not supported yet.

## Linux

Linux installable output is not supported yet. The manifest parser accepts a
`[linux]` section so the build manifest shape can evolve across platforms, but
`cargo crapapp bundle` does not produce Linux packages yet.

# libcrapapp

`libcrapapp` is the library crate behind the CLI and the generated Windows setup
projects.

Current public entrypoints:

- [`run_cli`], used by the `cargo-crapapp` binary.
- [`windows_installer`], runtime installer and uninstaller code embedded into
  generated Windows setup projects.

The `cargo-crapapp` binary intentionally stays tiny:

```rust,ignore
use libcrapapp::run_cli;

fn main() -> anyhow::Result<()> {
    run_cli()
}
```

Generated Windows setup projects depend on `libcrapapp` with one runtime feature:

- `windows`, for shared Windows installer config and build-support types.
- `windows-cli`, for generated command-line Windows installers.
- `windows-gui`, for generated graphical Windows installers.

Future application-facing helpers, for example Windows icon/resource helpers,
should live under `libcrapapp` and be documented here.
