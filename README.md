# Ubrus

Set a photo, looping video or GIF as your Ubuntu GNOME wallpaper. Choose one background for all monitors or a different one for each. Animation pauses on covered screens and keeps playing on visible ones.

## Install on Ubuntu

For Ubuntu 24.04 or 26.04 on amd64, open `ubrus_0.1.0_amd64.deb` in App Center. To install from a terminal, run this in the folder containing the package.

```sh
sudo apt install ./ubrus_0.1.0_amd64.deb
```

To reinstall a local build with the same version number, run `sudo dpkg -i ./ubrus_0.1.0_amd64.deb`.

Open **Ubrus** from the application menu. A still image for all monitors works immediately. For animation or separate monitor backgrounds, sign out and back in once, then enable the bundled extension.

```sh
gnome-extensions enable wallpaper@ubrus.org
```

<details>
<summary>Standalone executable and local Snap</summary>

**Executable.** Keep `ubrus` where you intend to run it and launch `./ubrus`. It uses the system GTK 4 and GStreamer libraries. Install the companion extension with `gnome-extensions install ./ubrus-extension.zip`.

**Local Snap.** Run `sudo snap install --dangerous ./ubrus_0.1.0_amd64.snap`. Install its companion extension with `gnome-extensions install /snap/ubrus/current/share/ubrus/ubrus-extension.zip`.

After installing the extension, sign out, sign back in and enable it with the command above. Use `gnome-extensions install --force` when replacing a previously installed copy.

</details>

## Set a wallpaper

1. Choose **Import file** (**Ctrl+O**). Ubrus copies PNG, JPEG, WebP, GIF, H.264 MP4 and VP8/VP9 WebM files into its library.
2. Select an item and a monitor. **Fit** shows the whole image. **Fill** crops it to the screen.
3. Choose **Apply** (**Ctrl+Enter**). You can close the window. The first animation enables **Restore animation at login**.

**Settings** (**Ctrl+,**) controls login, power saving, appearance and language. Ubrus follows your system theme and language by default and includes English and Russian. **Diagnostics** shows why a monitor is paused. Select text and press **Ctrl+C**, or use **Copy all**. **Export report** saves a new text file and shows its path. **Save as…** lets you choose a location.

Animation continues in the workspace overview. Enable **Pause animation in the GNOME overview** in Settings to hold the current frame until you return.

To delete one item, select it and choose **Delete from library…** beside **Apply**. Confirm to remove the imported copy and its monitor assignments. The original file stays where you imported it from.

Choose **Remove wallpaper from all monitors** in the library to stop playback and remove every assignment. Your files stay in the library. The DEB restores your previous GNOME background after a still image, or the Ubuntu default for assignments made by older builds. After a portal-applied still, the strict Snap uses the default image available to it, or a plain background.

## Build and license

Run `make release` to create the executable and extension in `dist/`. Build dependencies and package commands are in the [development guide](docs/development.md).

Licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE).
