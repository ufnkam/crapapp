# CRAP.toml configuration

All platform sections are optional. An empty `CRAP.toml` is valid and produces
a build manifest with no platforms.

## Build selection

The optional `[build]` section controls which packages and features are passed
to `cargo build`.

```toml
[build]
publisher = "Example Publisher"
packages = ["example"]
features = ["sqlite", "native-tls"]
```

Both lists are optional. Empty or missing lists mean no package or feature flags
are added by crapapp.

`publisher` is an optional `[build]` key. Windows setup writes the uninstall
registry `Publisher` value only when this key is present. Signing certificate
metadata can provide a better default later, but the installer no longer guesses
publisher from the app name.

## Windows

Windows supports GNU, GNU LLVM, and MSVC targets. MSVC can only be built on
Windows.

```toml
[windows]
targets = [
    "x86_64-pc-windows-gnu",
]
install_path = "$INSTALLPATH"
files = [
    { source = "Cargo.toml", destination = "Cargo.toml" },
]
icons = [
    { binary = "example", source = "assets/example.ico" },
]
```

`install_path` is optional. If present, relative destinations are prefixed with
it in the build manifest. Variables such as `$INSTALLPATH` stay symbolic and are
resolved later by the installer.

The Windows setup executable accepts optional `--args ADD_TO_PATH=1` or
`--args ADD_TO_PATH=0`. It defaults to `1`, so executable payload directories
are added to the user `PATH` unless runtime setup args explicitly disable it.

`bin_dir` is optional for Windows. If omitted, Cargo binaries are installed
directly into `install_path` when `install_path` is present.

```toml
[windows]
targets = ["x86_64-pc-windows-gnu"]
install_path = "$INSTALLPATH"
bin_dir = "bin"
```

This places binaries under `$INSTALLPATH/bin`.

`icons` is optional on Windows. Each icon is attached only to the Cargo binary
target named by `binary`. The installer writes the uninstall registry
`DisplayIcon` value to the installed binary matching the icon mapping. Generated
`setup.exe` and `uninstall.exe` use template-owned Microsoft Fluent icons, not
the configured app icon. SVG icons must be square and use one of the standard
Windows icon sizes: 16, 24, 32, 48, 64, 128, or 256 px.

Supported targets:

- `x86_64-pc-windows-gnu`
- `x86_64-pc-windows-msvc`
- `aarch64-pc-windows-gnullvm`
- `aarch64-pc-windows-msvc`

## Linux

Linux self-contained apps always place Cargo binaries in `bin`.

```toml
[linux]
targets = [
    "x86_64-unknown-linux-gnu",
]
files = [
    { source = "Cargo.toml", destination = "etc/example/Cargo.toml" },
]
```

Supported targets:

- `x86_64-unknown-linux-gnu`
- `x86_64-unknown-linux-musl`

## macOS

macOS currently uses the same self-contained app layout as Linux. We are not
building `.app` bundles yet, so Cargo binaries go in `bin`.

```toml
[macos]
targets = [
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
]
files = [
    { source = "Cargo.toml", destination = "etc/example/Cargo.toml" },
]
```

Supported targets:

- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
