# crapapp 

Crapapp is a cargo plugin for bundling Rust desktop apps from `CRAP.toml` without external
dependencies like wixl or nsis.

This crate ships two things:

- `cargo-crapapp`, a Cargo subcommand for building app installers from
  `CRAP.toml`.
- `libcrapapp`, the shared implementation used by the CLI.

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

Use platform filters to build only selected bundle formats:

```sh
cargo crapapp bundle --no-build --linux deb aur --windows msi --macos app dmg pkg
```

When any platform filter is present, only those platforms are bundled. Requested
bundle formats must also be configured for that platform in `CRAP.toml`.

Bundler output currently lands in:

```text
.crapapp_build/windows/<target>/msi/<display-name-or-package-name>.msi
.crapapp_build/macos/<target>/app/<display-name-or-package-name>.app
.crapapp_build/macos/<target>/pkg/<display-name-or-package-name>.pkg
.crapapp_build/macos/<target>/dmg/<display-name-or-package-name>.dmg
.crapapp_build/linux/<target>/deb/<package-name>.deb
.crapapp_build/linux/<target>/rpm/<package-name>-<version>-1.rpm
.crapapp_build/linux/<target>/aur/<package-name>.aur
```

## CRAP.toml quick start

`CRAP.toml` declares what cargo-crapapp should build and package.

```toml
[build]
publisher = "Acme"
display_name = "Acme Launcher"
description = "Desktop launcher for Acme tools"
homepage = "https://example.com/acme-launcher"
license = "Apache-2.0"
license_file = "LICENSE"
packages = ["acme-launcher"]
features = ["sqlite"]

[windows]
targets = ["x86_64-pc-windows-gnu"]
install_path = "$INSTALLPATH"
path_entries = ["$INSTALLPATH", "$INSTALLPATH/bin"]
bundle = "msi"
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
shortcuts = [
    { binary = "acme-launcher", name = "Acme Launcher" },
]
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
- `homepage`: optional URL. Used by Linux AppStream metadata to show an App
  Center Developer website link. If omitted, Cargo's package `homepage` is
  used when available.
- `license`: optional SPDX license expression such as `MIT` or `Apache-2.0`.
  Used in Linux AppStream metadata and RPM package metadata. If omitted,
  Cargo's package `license` is used when available.
- `license_file`: optional path to the license document to include verbatim in
  Linux packages. If omitted, Cargo's package `license-file` is used when
  available. Use this alongside `license`: the former supplies the exact legal
  text, while the latter supplies its SPDX identifier.
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
- `required`: optional boolean, default `true`. The Windows MSI uses it to
  decide whether the acceptance checkbox gates the Next button. macOS pkg and
  Linux packages preserve the license as package content; they do not expose
  the same optional-license wizard.

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
- `eula_report`: retained for manifest compatibility but not currently emitted
  by MSI, Linux, or macOS package writers. Do not rely on it for audit records.
  macOS pkg does not use `associated_files`.

### `[windows]`

`[windows]` configures Windows setup output.

Fields:

- `targets`: optional array of target triples. Supported values are
  `x86_64-pc-windows-gnu`, `x86_64-pc-windows-msvc`,
  `aarch64-pc-windows-gnullvm`, and `aarch64-pc-windows-msvc`.
- `bundle`: optional string or array. Defaults to `msi`; this is the only
  currently supported Windows bundle format. It generates a Windows Installer
  database with an embedded cabinet payload.
- `install_path`: optional string. If present, relative binary and payload
  destinations are prefixed with it in the build manifest. `$INSTALLPATH` is
  resolved by the generated installer at runtime.
- `bin_dir`: optional string. If omitted, Cargo binaries are installed directly
  under `install_path` when `install_path` is present. If present, Cargo
  binaries are installed under that directory relative to `install_path`, unless
  the path is already absolute/symbolic in the manifest.
- `path_entries`: optional array of directories the MSI adds to the current
  user's `PATH`. Each entry must start with `$INSTALLPATH`; use
  `$INSTALLPATH` for the installation root and `$INSTALLPATH/bin` for a
  subdirectory. It defaults to `["$INSTALLPATH"]` when omitted or empty. The
  installer shows an enabled **Add to PATH** checkbox so the user can opt out.
- `files`: optional array of payload file mappings.
- `associated_files`: optional array of associated file mappings.
- `eulas`: optional array of EULA files.
- `shortcuts`: optional array. Creates Start Menu `.lnk` files. Windows Search
  can find those shortcuts, but cargo-crapapp does not pin them.
- `display_icon`: optional PNG or ICO path. It is embedded as the Add/Remove
  Programs product icon and stamped into executable payloads before they are
  packaged.

MSI support covers payload files, Start Menu shortcuts, shortcut icons,
Add/Remove Programs metadata and product icons, EULAs, per-user `PATH` updates,
and `$INSTALLPATH`/`$HOMEPATH` associated files and directories through standard
Windows Installer tables. The MSI UI includes welcome, license, installation
directory, ready, progress, and completion dialogs; the directory page uses the
native Windows folder picker.

`shortcuts` fields:

- `binary`: required Cargo binary name without `.exe`.
- `name`: required Start Menu shortcut display name.
- `directory`: optional Start Menu directory.
- `icon`: optional icon source path. The installer copies it into the payload
  and points the shortcut at the installed icon. If omitted, the shortcut uses
  the target executable icon.

Windows Installer resolves `INSTALLPATH` through its installation-directory UI
and `HOMEPATH` to the current user's home directory. `ADD_TO_PATH` defaults to
`1`; the configured `path_entries` are appended to the current user's `PATH`
unless the user clears the installer checkbox or an MSI property transform or
command-line property disables it.

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
- The `[build].license_file` document is copied verbatim to
  `/usr/share/licenses/<package>/LICENSE` in deb, rpm, and AUR output. deb
  also writes `/usr/share/doc/<package>/copyright` with the SPDX identifier.
- `shortcuts`: optional array of desktop launchers. Each entry creates one
  `.desktop` entry using its `name`, `binary`, and optional `icon`. When this is
  set, only declared shortcuts appear in the app finder.
- `display_icon`: optional icon path. Linux package writers convert it to a
  256×256 PNG and install it under `/usr/share/icons/hicolor`; generated
  `.desktop` entries use that application icon unless a shortcut supplies one.

Linux `shortcuts` use the same fields as Windows: `binary` is the Cargo binary
name, `name` is the app-finder label, and `icon` is an optional icon source.

The deb, rpm, and AUR writers are implemented in Rust and do not call
`dpkg-deb`, `rpmbuild`, or `makepkg`. AUR output is a single gzip-compressed
`.aur` archive containing a top-level package directory with `PKGBUILD`,
`.SRCINFO`, and all local source files. Extract it, then run `makepkg` from
that directory.

Building Linux GNU targets from macOS requires a matching Linux GNU linker and
sysroot. The Rust target alone is not enough, because glibc targets still link
against Linux system libraries such as `libc`, `libpthread`, and `libdl`. For
example, `aarch64-unknown-linux-gnu` needs `aarch64-unknown-linux-gnu-gcc` or a
Cargo/Rust configuration that points at an equivalent Linux toolchain. If those
tools are not configured, build the Linux binaries on Linux and run
`cargo crapapp bundle --no-build`.

# libcrapapp

`libcrapapp` is the library crate behind the CLI.

Current public entrypoints:

- [`run_cli`], used by the `cargo-crapapp` binary.
- [`windows_installer`], Windows MSI authoring and bundling support.

The `cargo-crapapp` binary intentionally stays tiny:

```rust,ignore
use libcrapapp::run_cli;

fn main() -> anyhow::Result<()> {
    run_cli()
}
```

The crate does not expose generated Windows setup projects or Windows runtime
features; Windows output is authored directly as MSI.
