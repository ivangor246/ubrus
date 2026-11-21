# Development

Build on Ubuntu 24.04 amd64 for compatibility with Ubuntu 24.04 and 26.04. Install the pinned Rust toolchain with rustup and install the native dependencies below.

```sh
sudo apt install build-essential pkg-config libgtk-4-dev libgstreamer-plugins-base1.0-dev libdrm-dev libx11-dev \
  gstreamer1.0-plugins-good gstreamer1.0-libav gstreamer1.0-gl webp-pixbuf-loader \
  python3 python3-yaml unzip desktop-file-utils appstream
```

| Command | Result |
| --- | --- |
| `make build` | Development executable at `target/debug/ubrus` |
| `make release` | Release executable and GNOME extension ZIP in `dist/` |
| `make deb` | Installable DEB with the extension bundled |
| `make snap` | Strict core24 Snap |

Use the root Makefile to serialize Cargo and keep compilation to one job. The Snap target uses Snapcraft when available. Its local fallback uses snapd, squashfs-tools, Python YAML and the core24, gnome-46-2404 and mesa-2404 runtimes on Ubuntu 24.04.

For the module layout and wallpaper protocol, see [Architecture](architecture.md). For test commands and package checks, see [Tests](../tests/README.md).

Before a public release, add a homepage to the AppStream metadata, register the Snap name and request the session D-Bus slot declaration. Verify the signed candidate with its extension, capture screenshots, and check upgrades, login, lock, suspend and physical monitor changes on the target GPU.
