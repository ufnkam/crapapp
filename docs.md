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
.crapapp_build/windows/<target>/msi/<display-name-or-package-name>.msi
.crapapp_build/macos/<target>/app/<display-name-or-package-name>.app
.crapapp_build/macos/<target>/pkg/<display-name-or-package-name>.pkg
.crapapp_build/linux/<target>/deb/<package-name>.deb
.crapapp_build/linux/<target>/rpm/<package-name>-<version>-1.rpm
.crapapp_build/linux/<target>/aur/<package-name>.src.tar.gz
```

macOS DMG output is recognized in the manifest but its package writer is not
implemented yet.

## CRAP.toml quick start

`CRAP.toml` declares what cargo-crapapp should build and how Windows setup
should behave.

```toml
[build]
publisher = "Acme"
display_name = "Acme Launcher"
description = "Desktop launcher for Acme tools"
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
bundle = ["app", "pkg"]
display_icon = "assets/app.icns"
app_binary = "acme-launcher"
eulas = [
    { path = "EULA.txt" },
    { path = "THIRD_PARTY.txt", required = false },
]
files = [
    { source = "assets", destination = "Resources/assets" },
]

[macos.pkg]
identifier = "com.acme.launcher"
install_path = "/Applications"
bin_dir = "/usr/local/bin"
link_bins = true

[linux]
targets = ["x86_64-unknown-linux-gnu"]
bundle = "deb"
bin_dir = "/usr/bin"
display_icon = "assets/app.png"
files = [
    { source = "assets", destination = "/usr/share/acme-launcher/assets" },
]
associated_files = [
    { path = "/var/lib/acme-launcher", kind = "directory" },
]
eulas = [
    { path = "EULA.txt" },
    { path = "THIRD_PARTY.txt", required = false },
]
```

## CRAP.toml reference

`CRAP.toml` is strict: unknown fields are rejected. The supported top-level
sections are `[build]`, `[windows]`, `[macos]`, `[macos.pkg]`, and `[linux]`.

### `[build]`

`[build]` is optional. It controls Cargo package selection, feature selection,
and display metadata reused by platform bundlers.

Fields:

- `publisher`: optional string. Used as the Windows uninstall registry
  `Publisher`, as part of the generated macOS package identifier, and as package
  maintainer metadata for deb output. If omitted, platform outputs use their own
  fallback.
- `display_name`: optional string. User-facing app name. If omitted, the Cargo
  package name is used. macOS app/pkg names and installer UI labels use this
  value when present.
- `description`: optional string. Package description used by Linux package
  metadata. If omitted, the Cargo package description is used, then
  `display_name`, then the Cargo package name.
- `packages`: optional array of strings. Each entry is passed to Cargo as a
  selected package. If missing or empty, cargo-crapapp does not pass package
  selection flags.
- `features`: optional array of strings. Entries are passed to Cargo as enabled
  features. If missing or empty, cargo-crapapp does not pass feature flags.

### Common field types

`files` entries copy extra payload data in addition to Cargo binaries:

```toml
files = [
    { source = "assets", destination = "assets" },
    { source = "config/default.json", destination = "config/default.json" },
]
```

Fields:

- `source`: required path string. It must exist when cargo-crapapp reads the
  manifest. A file is copied as one payload file. A directory is walked
  recursively and copied in deterministic path order.
- `destination`: required path string. Meaning depends on platform. Windows and
  Linux use package/install paths directly. macOS app/pkg paths are relative to
  `.app/Contents` unless the destination starts with `Contents/`.

`eulas` entries describe license files:

```toml
eulas = [
    "EULA.txt",
    { path = "THIRD_PARTY.txt", required = false },
]
```

An entry can be a plain path string or an object.

- `path`: required for object form. Source text file path.
- `required`: optional boolean, default `true`. Windows GUI setup uses it to
  decide whether the acceptance checkbox gates the Next button. macOS pkg and
  Linux deb preserve the information only as text/package content because those
  formats do not expose the same custom optional-EULA wizard.

`associated_files` entries create app-owned empty files or directories:

```toml
associated_files = [
    { path = "$HOMEPATH/Documents/Acme/saves", kind = "directory" },
    { path = "$INSTALLPATH/settings.json", kind = "file", eula_report = true },
]
```

Fields:

- `path`: required path string.
- `kind`: required, either `file` or `directory`.
- `eula_report`: optional boolean, default `false`. Windows writes a JSON EULA
  acceptance report to this file path, or to `eulas.json` inside this directory.
  Linux deb currently creates the associated file/directory but does not write a
  runtime report. macOS pkg does not use `associated_files`.

### `[windows]`

`[windows]` configures Windows setup output.

Fields:

- `targets`: optional array of target triples. Supported values are
  `x86_64-pc-windows-gnu`, `x86_64-pc-windows-msvc`,
  `aarch64-pc-windows-gnullvm`, and `aarch64-pc-windows-msvc`.
- `bundle`: optional string or array. Defaults to `cli`. Values are `cli`,
  `gui`, and `msi`. `cli` and `gui` generate `setup.exe`; `msi` generates a
  Windows Installer database with an embedded cabinet payload.
- `install_path`: optional string. If present, relative binary and payload
  destinations are prefixed with it in the build manifest. `$INSTALLPATH` is
  resolved by the generated installer at runtime.
- `bin_dir`: optional string. If omitted, Cargo binaries are installed directly
  under `install_path` when `install_path` is present. If present, Cargo
  binaries are installed under that directory relative to `install_path`, unless
  the path is already absolute/symbolic in the manifest.
- `files`: optional array of payload file mappings.
- `associated_files`: optional array of associated file mappings.
- `eulas`: optional array of EULA files.
- `shortcuts`: optional array. Creates Start Menu `.lnk` files. Windows Search
  can find those shortcuts, but cargo-crapapp does not pin them.
- `display_icon`: optional PNG or ICO path. The GUI installer uses it in its
  header. The uninstall registry `DisplayIcon` points at the installed app
  executable. cargo-crapapp does not modify your built application executable
  icon.

MSI support covers payload files, Start Menu shortcuts, shortcut icons,
Add/Remove Programs product icons, and `$INSTALLPATH`/`$HOMEPATH` associated
files and directories through standard Windows Installer tables. PATH mutation
and EULA UI are still handled by the generated `cli`/`gui` setup executables.

`shortcuts` fields:

- `binary`: required Cargo binary name without `.exe`.
- `name`: required Start Menu shortcut display name.
- `directory`: optional Start Menu directory.
- `icon`: optional icon source path. The installer copies it into the payload
  and points the shortcut at the installed icon. If omitted, the shortcut uses
  the target executable icon.

Windows runtime variables:

- `ADD_TO_PATH` defaults to `1`; executable payload directories are added to the
  current user's `PATH` unless disabled.
- `INSTALLPATH` is provided by the GUI installer or by CLI `--args`.
- `HOMEPATH` is resolved to the current user's home directory by the installer.

CLI setup variables are passed as repeated `--args` values:

```sh
setup.exe --args INSTALLPATH=C:\Users\me\AppData\Local\Acme
setup.exe --args INSTALLPATH=C:\Users\me\AppData\Local\Acme --args ADD_TO_PATH=0
```

### `[macos]`

`[macos]` configures `.app`, `.pkg`, and `.dmg` output.

Fields:

- `targets`: optional array of target triples. Supported values are
  `x86_64-apple-darwin` and `aarch64-apple-darwin`.
- `bundle`: optional string or array. Defaults to `app`. Values are `app`,
  `pkg`, and `dmg`.
- `display_icon`: optional icon path. The source is copied into
  `Contents/Resources`. When the source is `.icns`, `Info.plist` also sets
  `CFBundleIconFile`.
- `app_binary`: optional Cargo binary name. It becomes `CFBundleExecutable`.
  If omitted or empty, cargo-crapapp falls back to the first executable payload.
- `files`: optional array of payload file mappings. Destinations are relative to
  `.app/Contents` unless they start with `Contents/`.
- `eulas`: optional array of EULA files. For pkg output, files are merged into
  top-level `Resources/License.txt`, and the Distribution points Apple Installer
  at that license resource.
- `pkg`: optional table configured through `[macos.pkg]`.

Finder-launched `.app` bundles should point `app_binary` at a GUI binary. macOS
does not open Terminal for a CLI executable launched from Finder, so stdout,
stderr, and stdin are not visible as an interactive terminal session.

### `[macos.pkg]`

`[macos.pkg]` configures `bundle = "pkg"`.

Fields:

- `identifier`: optional reverse-DNS package receipt id. If omitted,
  cargo-crapapp derives one from `publisher` and the Cargo package name.
- `install_path`: optional absolute path, default `/Applications`. The app
  installs as `<install_path>/<display-name-or-package-name>.app`.
- `bin_dir`: optional absolute path, default `/usr/local/bin`. Used only when
  `link_bins = true`.
- `link_bins`: optional boolean, default `true`. When enabled, each executable
  payload gets a shell shim in `bin_dir` that forwards to the matching binary in
  `<app>.app/Contents/MacOS`.

The pkg writer is implemented in Rust and does not call `pkgbuild`,
`productbuild`, or other system packaging tools.

### `[linux]`

`[linux]` configures Linux package output.

Fields:

- `targets`: optional array of target triples. Supported values are
  `x86_64-unknown-linux-gnu`, `x86_64-unknown-linux-musl`,
  `aarch64-unknown-linux-gnu`, and `aarch64-unknown-linux-musl`.
- `bundle`: optional string or array. Defaults to `deb`. Values are `deb`,
  `rpm`, and `aur`.
- `install_path`: optional string. If present, relative binary and payload
  destinations are prefixed with it in the build manifest.
- `bin_dir`: optional string, default `/usr/bin`. Cargo binaries are installed
  there, or under `install_path/bin_dir` when `install_path` is present and the
  destination is not already prefixed.
- `files`: optional array of payload file mappings.
- `associated_files`: optional array of associated file mappings. Linux package
  output creates these as empty files or directories in the package payload.
- `eulas`: optional array of EULA files. deb output installs them under
  `/usr/share/doc/<package>/licenses`; rpm and AUR output install them under
  `/usr/share/licenses/<package>`.
- `display_icon`: optional path. Parsed and shown by `inspect`; Linux package
  writers do not install desktop integration from it yet.

The deb, rpm, and AUR writers are implemented in Rust and do not call
`dpkg-deb`, `rpmbuild`, or `makepkg`.

Building Linux GNU targets from macOS requires a matching Linux GNU linker and
sysroot. The Rust target alone is not enough, because glibc targets still link
against Linux system libraries such as `libc`, `libpthread`, and `libdl`. For
example, `aarch64-unknown-linux-gnu` needs `aarch64-unknown-linux-gnu-gcc` or a
Cargo/Rust configuration that points at an equivalent Linux toolchain. If those
tools are not configured, build the Linux binaries on Linux and run
`cargo crapapp bundle --no-build`.

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
