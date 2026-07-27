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
It can:

- build configured Cargo packages and collect payload files;
- generate Windows `.msi` installers with standard installer UI, Start Menu
  shortcuts, Add/Remove Programs metadata, EULAs, associated files, and
  per-user `PATH` entries;
- create macOS `.app`, `.pkg`, and `.dmg` bundles; and
- create Linux `.deb`, `.rpm`, and AUR package sources with desktop entries,
  AppStream metadata, icons, EULAs, and associated files.

## Should You Use It?

Probably not. The only reason to use it is that your life circles in a corporate
trap and normal packaging tools are politically unavailable.

## Commands

```sh
cargo crapapp inspect
cargo crapapp inspect --output json
cargo crapapp build
cargo crapapp bundle
cargo crapapp bundle --no-build
cargo crapapp bundle --no-build --linux deb aur --windows msi --macos app dmg pkg
```

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

Full CLI, `CRAP.toml`, and `libcrapapp` documentation lives in [docs.md](docs.md)
and is rendered as the crate documentation on docs.rs.
