# Ubrus

Set a photo, looping video or GIF as your Ubuntu GNOME wallpaper. Choose one background for all monitors or a different one for each. Animation pauses on covered screens and keeps playing on visible ones.

## Install on Ubuntu

For Ubuntu 24.04 or 26.04 on amd64, open `ubrus_0.2.0_amd64.deb` in App Center. To install from a terminal, run this in the folder containing the package.

```sh
sudo apt install ./ubrus_0.2.0_amd64.deb
```

To reinstall a local build with the same version number, run `sudo dpkg -i ./ubrus_0.2.0_amd64.deb`.

Open **Ubrus** from the application menu. A still image for all monitors works immediately. For animation or separate monitor backgrounds, sign out and back in once, then enable the bundled extension.

```sh
gnome-extensions enable wallpaper@ubrus.org
```

<details>
<summary>Standalone executable and local Snap</summary>

**Executable.** Keep `ubrus` where you intend to run it and launch `./ubrus`. It uses the system GTK 4 and GStreamer libraries. Install the companion extension with `gnome-extensions install ./ubrus-extension.zip`.

**Local Snap.** Run `sudo snap install --dangerous ./ubrus_0.2.0_amd64.snap`. Install its companion extension with `gnome-extensions install /snap/ubrus/current/share/ubrus/ubrus-extension.zip`.

After installing the extension, sign out, sign back in and enable it with the command above. Use `gnome-extensions install --force` when replacing a previously installed copy.

</details>

## Set a wallpaper

1. Choose **Import file** (**Ctrl+O**) and select a thumbnail. Ubrus accepts PNG, JPEG, static and animated GIF or WebP, H.264 MP4 and VP8/VP9 WebM. The library owns a copy of each import.
2. Choose the target display and **Fit** or **Fill**. Drag inside the monitor preview to position the picture, or use **Shift+arrow keys**. Adjust zoom and brightness. Animated images and videos also have a silent preview and a playback speed control. When assigning to all monitors you can preview each display's proportions.
3. Choose **Apply** (**Ctrl+Enter**). Closing the window leaves the desktop running. The first animated assignment enables **Start at login** unless you have already set a preference.

**More actions** includes reset, hide/restore and deletion of the selected library copy. Hiding your wallpaper restores the system background and keeps your assignments and adjustments. The DEB checks ownership before restoring a portal wallpaper. The strict Snap uses the extension for still images when available. GNOME Settings can change a previous portal wallpaper whose ownership the Snap cannot verify.

The GNOME top-bar menu opens Ubrus, pauses or resumes animation, hides or restores your wallpaper and enables or disables the application. Disabling also turns off login startup. Clear **Show icon in top bar** to hide just the icon. Restore it from **Settings** (**Ctrl+,**).

Settings also controls the language, theme, power saving and overview playback. English and Russian are included. **Diagnostics** (**Ctrl+D**) supports text selection, copying and report export.

After installing a newer DEB, choose **Apply update** in Settings to restart the running application. Reinstalling a build with the same version works too. When the loaded GNOME Shell extension has changed, Settings indicates that the extension will load after signing in again.

## Build and license

Run `make release` to create the executable and extension in `dist/`. Build dependencies and package commands are in the [development guide](docs/development.md).

Licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE).
