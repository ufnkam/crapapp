# cargo-crapapp

Cargo plugin for bundling Rust desktop apps from `CRAP.toml` without external
dependencies like wixl or nsis.

It exists because some enterprise environments, especially in banking, are very
good at inventing security rituals and very bad at understanding what is
actually secure. Sometimes Artifactory metadata is missing, nobody owns Linux
package distribution, or Windows packaging tools are blocked.

The goal is simple: read a manifest, build Rust binaries for configured targets,
and produce something a user can install without negotiating with five teams and
producing 2534 Jira tickets.

## What It Does
It can build configured Cargo packages, collect payload files, generate a
`setup.exe`, embed the payload, write uninstall metadata, create optional GUI
installer screens, show EULAs, create associated app files, write EULA
acceptance reports, create Start Menu search shortcuts, and update the current
user's `PATH`.

The generated `setup.exe` always uses the cargo-crapapp crab icon. Application
`display_icon` is used by the visual installer and Windows uninstall metadata;
the application executable still owns its own icon.

Linux and macOS output are not supported yet.

## Should You Use It?

Probably not. The only reason to use it is that your life circles in a corporate
trap and normal packaging tools are politically unavailable.

## Commands

```sh
cargo crapapp inspect
cargo crapapp inspect --output json
cargo crapapp build
```

Windows output currently lands in:

```text
.crapapp_build/windows/<target>/setup.exe
```

Full CLI, `CRAP.toml`, and `libcrapapp` documentation lives in [docs.md](docs.md)
and is rendered as the crate documentation on docs.rs.

