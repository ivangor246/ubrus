# Ubrus

[![CI](https://github.com/ivangor246/ubrus/actions/workflows/ci.yml/badge.svg)](https://github.com/ivangor246/ubrus/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/ivangor246/ubrus)](https://github.com/ivangor246/ubrus/releases/latest)
[![Ubuntu 24.04 and 26.04](https://img.shields.io/badge/Ubuntu-24.04%20%7C%2026.04-E95420)](#install-on-ubuntu)
[![GNOME Shell 46 / 50](https://img.shields.io/badge/GNOME%20Shell-46%20%7C%2050-4A86CF)](#install-on-ubuntu)
[![License](https://img.shields.io/badge/license-MIT%20or%20Apache--2.0-blue)](#license)

**English** · [Русский](docs/README.ru.md)

Set a photo, looping video or animated GIF as your Ubuntu GNOME wallpaper. Choose one background for all monitors or a different one for each. Animation pauses on covered screens and keeps playing on visible ones.

![The Ubrus library shows nine wallpapers, with Amber dunes selected in the monitor preview](docs/assets/editor-en.png)

## Install on Ubuntu

For Ubuntu 24.04 (GNOME Shell 46) or 26.04 (GNOME Shell 50) on amd64, [download the DEB](https://github.com/ivangor246/ubrus/releases/latest/download/ubrus-amd64.deb). It includes the companion extension. Open the file in App Center or run the following command in its folder.

```sh
sudo apt install ./ubrus-amd64.deb
```

Open **Ubrus** from the application menu. A still image for all monitors works immediately. For animation or separate backgrounds, sign out and back in once, then enable the bundled extension.

```sh
gnome-extensions enable wallpaper@ubrus.org
```

<details>
<summary>Standalone archive, Snap and checksums</summary>

The [standalone archive](https://github.com/ivangor246/ubrus/releases/latest/download/ubrus-linux-amd64.tar.gz) includes the executable, extension ZIP and licenses. Extract it, keep the directory where you intend to run Ubrus, then launch `./ubrus` from that directory. It uses the system GTK 4 and GStreamer libraries. Install the companion with `gnome-extensions install ./ubrus-extension.zip`.

For the strict Snap, check the available channels with `snap info ubrus`. Once a stable revision is published, install it with `sudo snap install ubrus`. Install its extension with `gnome-extensions install /snap/ubrus/current/share/ubrus/ubrus-extension.zip`. To replace an older copy after a Snap refresh, add `--force` after `install`. Sign out, sign back in and enable the extension as above.

The [release page](https://github.com/ivangor246/ubrus/releases/latest) also provides the extension separately. Download [SHA256SUMS](https://github.com/ivangor246/ubrus/releases/latest/download/SHA256SUMS) from the same release as your files. Run `sha256sum --check --ignore-missing SHA256SUMS` in their folder.

</details>

## Set a wallpaper

1. Choose **Import files** (**Ctrl+O**), select one or more files, then choose a thumbnail. Ubrus accepts PNG, JPEG, static and animated GIF or WebP, H.264 MP4 and VP8/VP9 WebM. The library keeps a copy of each import. Search by filename to find an item. If you stop a batch, files already added stay in the library.
2. Choose the display and **Fit** or **Fill**. Drag the picture in the monitor preview or use **Shift+arrow keys**. Adjust zoom and brightness. Videos and animated images also have a silent preview and playback speed control.
3. Choose **Apply** (**Ctrl+Enter**). Closing the window leaves the desktop running. The first animated assignment enables **Start at login** unless you have already set a preference.

**More actions** includes reset, hide/restore and deletion of the selected library copy. Hiding a wallpaper restores the system background and keeps your assignments and adjustments. The DEB checks ownership before restoring a portal wallpaper. The strict Snap uses the extension for still images when available. GNOME Settings can change a previous portal wallpaper whose ownership the Snap cannot verify.

The GNOME top-bar menu opens Ubrus, pauses or resumes animation, hides or restores the wallpaper and enables or disables the application. Disabling Ubrus also turns off login startup. Toggle **Show icon in top bar** in **Settings** (**Ctrl+,**) to hide or restore the icon.

Settings also controls the language, theme, power saving and overview playback. **Diagnostics** (**Ctrl+D**) supports text selection, copying and report export. After installing a newer DEB, choose **Apply update** in Settings. To reinstall a local DEB with the same version, run `sudo dpkg -i ./ubrus-amd64.deb`. GNOME loads a replaced extension at the next login.

## Development and support

**Topics** · [wallpaper-manager](https://github.com/topics/wallpaper-manager) · [animated-wallpaper](https://github.com/topics/animated-wallpaper) · [gnome-shell-extension](https://github.com/topics/gnome-shell-extension) · [rust](https://github.com/topics/rust)

Build the executable and extension with `make release`. The [development guide](docs/development.md) covers dependencies, packages and publication. The [test guide](tests/README.md) describes desktop sessions and playback measurements. [Report an issue](https://github.com/ivangor246/ubrus/issues) with your Ubuntu version, installation method and steps to reproduce. Choose **Copy public report** in Diagnostics to include playback and display statistics while keeping filenames, paths and activity logs private.

## License

Choose the [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE) license. Binary distributions include third-party notices.
