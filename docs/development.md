# Development

Build on Ubuntu 24.04 amd64 for Ubuntu 24.04 and 26.04. Install rustup, then install the native build and verification dependencies. The first build uses the Rust toolchain in `rust-toolchain.toml`.

```sh
sudo apt install build-essential pkg-config libgtk-4-dev libgstreamer-plugins-base1.0-dev libdrm-dev libx11-dev \
  gstreamer1.0-plugins-good gstreamer1.0-libav gstreamer1.0-gl librsvg2-common webp-pixbuf-loader \
  python3 python3-yaml unzip desktop-file-utils appstream nodejs gjs
```

| Command | Result |
| --- | --- |
| `make build` | Development executable at `target/debug/ubrus` |
| `make check` | Rust format, Clippy, unit tests and extension tests |
| `make release` | Release executable and GNOME extension ZIP in `dist/` |
| `make deb` | Installable DEB with the extension bundled |
| `make snap` | Strict core24 Snap |

Use the root Makefile to serialize Cargo and keep compilation to one job. The Snap target uses Snapcraft when available. Its local fallback uses snapd, squashfs-tools, Python YAML and the core24, gnome-46-2404 and mesa-2404 runtimes on Ubuntu 24.04. To install a local test build, use `sudo snap install --dangerous ./dist/ubrus_<version>_amd64.snap`.

[Architecture](architecture.md) describes the module layout and wallpaper protocol. [Tests](../tests/README.md) describes the isolated desktop sessions and installed-package checks.

## GitHub Releases

Pull requests and pushes to `main` run the [CI workflow](../.github/workflows/ci.yml). After the unit checks, CI exercises the controller, the renderer and the full native application in separate Xvfb sessions with software rendering and decoding. On failure, the Actions run provides the session logs and any captured screenshots for seven days.

When preparing a release, set the package version in `Cargo.toml`, update `Cargo.lock`, set the same Snap version and add the latest dated entry at the top of the AppStream release list. Bump the extension version in `extension/metadata.json` when publishing a new extension revision.

For example, verify a 0.2.0 build locally using the following commands in order.

```sh
/usr/bin/python3 packaging/check-release.py --tag v0.2.0
make check
make deb
bash packaging/github-assets.sh v0.2.0
```

The GitHub release workflow runs when the maintainer pushes a `v<version>` tag. It verifies the tag and metadata, builds a fresh DEB on Ubuntu 24.04, compares its executable and extension with the standalone versions, and checks the installed CLI version in clean Ubuntu 24.04 and 26.04 containers. Passing these checks publishes the DEB, a standalone tarball with the executable, companion ZIP, both project licenses and third-party notices, the companion ZIP by itself, and SHA256SUMS. Asset names stay the same across tags so both READMEs can link to the latest downloads. Each tag retains its own files. [GitHub release workflow](../.github/workflows/release.yml)

The package's Homepage field and the store's support link point to GitHub. Enable Issues in repository settings. Suggested GitHub topics for the About panel are `animated-wallpaper`, `wallpaper-manager`, `gnome-shell-extension`, `ubuntu`, `rust`, `gstreamer` and `snap`. Both READMEs show the supported Ubuntu versions and point to the same release assets. The desktop screenshots used in the READMEs and AppStream metadata are in `docs/assets/`.

Release uploads are staged in a draft. A retry resumes an existing draft and checks the downloadable assets when the tag has already been published.

## Snap Store publisher setup

1. Sign in with the publishing account using `snapcraft login`. Check ownership of the name with `snapcraft status ubrus`. For a new listing, register it with `snapcraft register ubrus`. The account running the workflows needs publisher or collaborator access.
2. The strict snap claims the well-known session D-Bus name `org.ubrus.Ubrus` through its `wallpaper` slot. On the first upload, request manual review for the required snap declaration in the Store dashboard. Provide the slot, its use by the host GNOME Shell extension and the public source link. Confirm that approval permits the signed revision to install through a Store channel. [Canonical's D-Bus interface documentation](https://snapcraft.io/docs/reference/interfaces/dbus-interface/)
3. In the Store dashboard, state the supported Ubuntu GNOME versions (24.04 and 26.04) and amd64 architecture in the listing, explain the extra extension installation for animated and per-monitor wallpapers, and upload the localized editor screenshots from `docs/assets/`. Use the project icon in `packaging/desktop/` and the repository's Issues page for support.
4. In GitHub Settings → Environments, create `snap-candidate` and `snap-stable`. Limit deployments to protected release tags and designate a reviewer for stable promotion. Give each environment its own secret named `SNAPCRAFT_STORE_CREDENTIALS`. From a trusted workstation, export credentials limited to this snap and the matching channel and load them into the environment secret. Replace the example expiration date with a date appropriate for your account.

   ```sh
   umask 077
   snapcraft export-login --snaps=ubrus --channels=latest/candidate \
     --acls=package_access,package_push,package_update,package_release --expires=2027-01-31 candidate.credentials
   gh secret set SNAPCRAFT_STORE_CREDENTIALS --env snap-candidate < candidate.credentials
   snapcraft export-login --snaps=ubrus --channels=latest/stable \
     --acls=package_access,package_release --expires=2027-01-31 stable.credentials
   gh secret set SNAPCRAFT_STORE_CREDENTIALS --env snap-stable < stable.credentials
   rm candidate.credentials stable.credentials
   ```

   Rotate these credentials before they expire. When the Store account and secrets are ready, set the GitHub Actions repository variable `SNAP_STORE_ENABLED` to `true`. [Snapcraft authentication](https://documentation.ubuntu.com/snapcraft/latest/reference/commands/export-login/)

## Signed Snap acceptance and promotion

After creating a GitHub release, the tag workflow can build the same tag with Snapcraft, validate the strict snap and publish it to `latest/candidate`. Once the Store serves the uploaded bytes, the Actions summary records the assigned revision. A clean Ubuntu runner installs it from the candidate channel, checks the installed revision, runs the executable and verifies the extension ZIP. When a stable revision already exists, the runner also installs stable, refreshes to candidate and verifies the resulting revision. For the first release the summary records a fresh installation only. The runner starts a separate GNOME Wayland login with two virtual monitors and exercises the signed controller, renderer and bundled extension together using the desktop-session tests.

To run the candidate workflow separately for an existing release, select its tag in Actions or use `gh workflow run snap.yml --ref v0.2.0 -f channel=candidate`. Use the revision number from its successful acceptance run for the remaining checks.

1. On Ubuntu 24.04 and 26.04, install with `sudo snap install ubrus --channel=latest/candidate` or refresh an existing install with `sudo snap refresh ubrus --channel=latest/candidate`. Check the revision in `snap list ubrus` against the Actions summary.
2. Install the companion from `/snap/ubrus/current/share/ubrus/ubrus-extension.zip`. For an upgrade, keep the previous extension initially, check the update hint in Settings, then replace it with `gnome-extensions install --force /snap/ubrus/current/share/ubrus/ubrus-extension.zip`. Sign out, sign back in, and enable `wallpaper@ubrus.org`.
3. Import a still and a video, apply them to separate displays, cover and uncover each output, then check login, lock/unlock, suspend/resume and a physical display connection change on the target GPU. On an upgrade, check that prior assignments, media and settings survive. Record the tested revision, OS, session, GPU and results in the release issue.
4. Promote the recorded revision by dispatching the workflow on the **same tag** with `channel=stable` and its numeric `revision`, for example `gh workflow run snap.yml --ref v0.2.0 -f channel=stable -f revision=42`. Before promotion, the workflow downloads the current signed candidate, verifies the requested revision and tagged contents, and repeats the installation checks. The stable job releases that exact revision after environment approval. Confirm the published channel with `snapcraft status ubrus`. [Snap workflow](../.github/workflows/snap.yml) · [Snap revisions](https://snapcraft.io/docs/explanation/how-snaps-work/revisions/)

## Translations

The window and panel share English and Russian strings in `extension/locale.json`. Add matching keys in every catalog and register additional languages in the preferences language selector and system-locale matcher. The extension package fingerprints its sources, icons and translations so the running window can identify an installed Shell update.
