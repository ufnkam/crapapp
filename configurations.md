# CRAP.toml configuration

All platform sections are optional. An empty `CRAP.toml` is valid and produces
a build manifest with no platforms.

## Cargo build selection

The optional `[cargo]` section controls which packages and features are passed
to `cargo build`.

```toml
[cargo]
packages = ["example"]
features = ["sqlite", "native-tls"]
```

Both lists are optional. Empty or missing lists mean no package or feature flags
are added by crapapp.

## Windows

Windows supports GNU, GNU LLVM, and MSVC toolchains. MSVC can only be built on
Windows.

```toml
[windows]
toolchains = [
    "x86_64-pc-windows-gnu",
]
install_path = "$INSTALLPATH"
files = [
    { source = "Cargo.toml", destination = "Cargo.toml" },
]
```

`install_path` is optional. If present, relative destinations are prefixed with
it in the build manifest. Variables such as `$INSTALLPATH` stay symbolic and are
resolved later by the installer.

`bin_dir` is optional for Windows. If omitted, Cargo binaries are installed
directly into `install_path` when `install_path` is present.

```toml
[windows]
toolchains = ["x86_64-pc-windows-gnu"]
install_path = "$INSTALLPATH"
bin_dir = "bin"
```

This places binaries under `$INSTALLPATH/bin`.

Supported toolchains:

- `x86_64-pc-windows-gnu`
- `x86_64-pc-windows-msvc`
- `aarch64-pc-windows-gnullvm`
- `aarch64-pc-windows-msvc`

## Linux

Linux self-contained apps always place Cargo binaries in `bin`.

```toml
[linux]
toolchains = [
    "x86_64-unknown-linux-gnu",
]
files = [
    { source = "Cargo.toml", destination = "etc/example/Cargo.toml" },
]
```

Supported toolchains:

- `x86_64-unknown-linux-gnu`
- `x86_64-unknown-linux-musl`

## macOS

macOS currently uses the same self-contained app layout as Linux. We are not
building `.app` bundles yet, so Cargo binaries go in `bin`.

```toml
[macos]
toolchains = [
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
]
files = [
    { source = "Cargo.toml", destination = "etc/example/Cargo.toml" },
]
```

Supported toolchains:

- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
