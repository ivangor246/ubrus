# Development

Build on Ubuntu 24.04 amd64 for both supported Ubuntu releases. Install rustup and the native dependencies, then run one Make target at a time from the repository root. The Makefile serializes Cargo and `rust-toolchain.toml` pins the toolchain.

```sh
sudo apt install build-essential pkg-config libgtk-4-dev libgstreamer-plugins-base1.0-dev libdrm-dev libx11-dev \
  gstreamer1.0-plugins-good gstreamer1.0-libav gstreamer1.0-gl librsvg2-common webp-pixbuf-loader \
  python3 python3-yaml unzip desktop-file-utils appstream nodejs gjs
```

| Command | Result |
| --- | --- |
| `make build` | Debug executable at `target/debug/ubrus` |
| `make check` | Formatting, Clippy, Rust and extension tests |
| `make release` | Optimized executable and extension ZIP in `dist/` |
| `make deb` | DEB with the extension bundled |
| `make snap` | Strict core24 Snap |

The Snap build uses Snapcraft when available. When updating `rust-toolchain.toml`, update the Rust archive URL and SHA-256 in `snap/snapcraft.yaml`. Snapcraft uses that toolchain only during the build. The Ubuntu 24.04 fallback needs snapd, squashfs-tools, Python YAML and the core24, gnome-46-2404 and mesa-2404 runtimes.

For a code change, start with the [architecture](architecture.md) and [test guide](../tests/README.md). The editor and extension share translations in `extension/locale.json`. Keep message keys and placeholders in sync across locales. Translated user guides are in `docs/readme/`.

## GitHub release

1. Set the release version in `Cargo.toml`, `Cargo.lock` and `snap/snapcraft.yaml`. Add a dated release to the top of the AppStream release list. Bump `extension/metadata.json` when releasing a new extension revision.
2. Check the tag and local packages. For example, to verify version 0.2.0, run these commands in order.

   ```sh
   /usr/bin/python3 packaging/check-release.py --tag v0.2.0
   make check
   make deb
   bash packaging/github-assets.sh v0.2.0
   ```

3. Tag the release commit and push the matching `v<version>` tag. GitHub runs CI, installs the DEB on Ubuntu 24.04 and 26.04, then publishes the DEB, standalone archive, extension ZIP and `SHA256SUMS`. A retry can resume a draft. [Release workflow](https://github.com/ivangor246/ubrus/blob/main/.github/workflows/release.yml)

## Snap Store

### Publisher setup

- Register `ubrus` with the publishing account if needed. Request Store approval for the `wallpaper` slot on the session bus (`org.ubrus.Ubrus`). The host GNOME Shell extension uses it to talk to the confined app. [D-Bus interface](https://snapcraft.io/docs/reference/interfaces/dbus-interface/)
- In the listing, specify Ubuntu GNOME 24.04 and 26.04 on amd64 and explain how to install the bundled extension. The localized screenshots are in `docs/assets/`.
- Create the GitHub environments below. In each, set `SNAPCRAFT_STORE_CREDENTIALS` using `snapcraft export-login` scoped to `--snaps=ubrus`, the channel and ACLs shown. Set an expiry and rotate the credentials before it. Restrict deployments to release tags, require a reviewer for stable and set the repository variable `SNAP_STORE_ENABLED=true`. [Snapcraft credentials](https://documentation.ubuntu.com/snapcraft/latest/reference/commands/export-login/)

| Environment | Channel | ACLs |
| --- | --- | --- |
| `snap-candidate` | `latest/candidate` | `package_access,package_push,package_update,package_release` |
| `snap-stable` | `latest/stable` | `package_access,package_release` |

### Test and promote a revision

After the GitHub release, Actions uploads a candidate, installs the signed revision, tests it in GNOME Wayland and checks refresh from stable when available. The Actions summary records the revision. To rerun for an existing release, use `gh workflow run snap.yml --ref v0.2.0 -f channel=candidate`.

Install or refresh from `latest/candidate` on both supported Ubuntu versions. Match `snap list ubrus` to the recorded revision. Check extension updates, saved assignments, playback on separate monitors, lock/unlock, suspend/resume and physical monitor reconnection. Record the OS, GPU and results in the release issue.

Promote the tested revision on the same tag. Replace the example tag and revision with the values you tested.

```sh
gh workflow run snap.yml --ref v0.2.0 -f channel=stable -f revision=42
```

The workflow verifies the candidate again before promoting that revision. Check the published channel with `snapcraft status ubrus`. [Snap workflow](https://github.com/ivangor246/ubrus/blob/main/.github/workflows/snap.yml)
