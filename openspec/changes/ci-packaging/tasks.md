## 1. Desktop Integration Files

- [x] 1.1 Create `nux-emulator.desktop` file conforming to freedesktop Desktop Entry Spec v1.5 with Type, Name, Exec, Icon, and Categories keys. Verify with `desktop-file-validate`.
- [x] 1.2 Create `nux-emulator.svg` application icon with square viewBox. Place at project root or assets directory for packaging.
- [x] 1.3 Create `nux-emulator.metainfo.xml` AppStream metainfo file with id, name, summary, description, launchable, homepage URL, project_license (GPL-3.0), and content_rating. Verify with `appstreamcli validate`.

## 2. Build Workflow — Deb Job

- [x] 2.1 Create `.github/workflows/build-linux.yml` with workflow triggers (`push`, `pull_request`, `workflow_call`) and the `build-deb` job running on `ubuntu-24.04`.
- [x] 2.2 Add steps to `build-deb`: checkout, install Rust toolchain via `dtolnay/rust-toolchain`, install GTK4/libadwaita dev packages via apt, restore Cargo cache via `actions/cache`.
- [x] 2.3 Add steps to `build-deb`: run `cargo build --release --workspace`, package binary + keymaps + desktop file + icon + metainfo into `.deb` using `dpkg-deb`, declaring GTK4/libadwaita runtime dependencies.
- [x] 2.4 Add step to `build-deb`: upload `.deb` via `actions/upload-artifact`.

## 3. Build Workflow — RPM Job

- [x] 3.1 Add `build-rpm` job to `build-linux.yml` running inside `container: fedora:41` on `ubuntu-latest`.
- [x] 3.2 Add steps to `build-rpm`: install Rust toolchain, GTK4/libadwaita dev packages via dnf, restore Cargo cache.
- [x] 3.3 Add steps to `build-rpm`: run `cargo build --release --workspace`, create inline RPM spec, build `.rpm` via `rpmbuild` with the same file set and FHS paths as the deb.
- [x] 3.4 Add step to `build-rpm`: upload `.rpm` via `actions/upload-artifact`.

## 4. Build Workflow — Tarball Job

- [x] 4.1 Add `build-binary` job to `build-linux.yml` running on `ubuntu-24.04`.
- [x] 4.2 Add steps to `build-binary`: checkout, install Rust toolchain, GTK4/libadwaita dev packages, restore Cargo cache, run `cargo build --release --workspace`.
- [x] 4.3 Add steps to `build-binary`: assemble tarball directory with binary, keymaps, desktop file, icon, metainfo, and `install.sh`; create `.tar.gz` archive.
- [x] 4.4 Create `scripts/install.sh` that copies files from extracted tarball to FHS-standard locations (`/usr/bin/`, `/usr/share/applications/`, etc.). Make executable.
- [x] 4.5 Add step to `build-binary`: upload `.tar.gz` via `actions/upload-artifact`.

## 5. Release Workflow

- [x] 5.1 Create `.github/workflows/release.yml` triggered on tag push matching `v*`.
- [x] 5.2 Add job that calls `build-linux.yml` via `workflow_call` and waits for completion.
- [x] 5.3 Add job that downloads all artifacts, computes SHA256 checksums, and creates a GitHub Release with the tag name as title, checksums in the body, and all packages attached as release assets.

## 6. Local Build Script

- [x] 6.1 Create `scripts/act-build.sh` with bash shebang and executable permission. Add check for `act` on PATH with helpful error message if missing.
- [x] 6.2 Add logic to create `./build/` directory if absent, invoke `act` with the build workflow and `--bind` mount of `./build/` as output volume.
- [x] 6.3 Add workflow steps (conditional on `ACT` env var) that copy built packages to the mounted `/build/` path so they appear on the host after act finishes.

## 7. Verification

- [ ] 7.1 Run `act` locally via `scripts/act-build.sh` and verify all three packages appear in `./build/`.
- [ ] 7.2 Inspect `.deb` contents with `dpkg-deb -c` to verify all required files are present at correct paths.
- [ ] 7.3 Inspect `.rpm` contents with `rpm -qlp` to verify all required files are present at correct paths.
- [ ] 7.4 Extract tarball, run `install.sh` in a test prefix, and verify files are placed correctly.
- [ ] 7.5 Push a test tag to a fork and verify the release workflow creates a GitHub Release with all assets and checksums.
