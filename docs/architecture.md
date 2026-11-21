# Architecture

Ubrus runs one controller per login session. A second launch asks the controller to open its settings window. The controller can restore a wallpaper at login without initializing Iced.

| Module | Responsibility |
| --- | --- |
| `src/app/` | GIO application lifecycle, UI commands, diagnostics and the D-Bus protocol with GNOME Shell |
| `src/ui/` | Iced window, keyboard shortcuts, screen selection and cancellable previews |
| `src/library/` | Import, content-addressed media and thumbnails, profile storage and local logs |
| `src/desktop/` | Parented desktop portals and login entries |
| `src/media/` and `src/renderer/` | Image and video decoding, shared playback and per-monitor GTK surfaces |
| `extension/` | Controller connection, renderer process, window placement, desktop layers and fallback images |

## Applying a wallpaper

A still image for all monitors goes through the desktop wallpaper portal. Other assignments use the extension. Shell reads a proposed scene over `org.ubrus.Ubrus.Wallpaper1`, matches its target to a monitor and reports whether it was applied. The controller commits the assignment after confirmation. The strict Snap wakes Shell through a watched file for scene changes and status requests.

Shell launches the Rust renderer and verifies ownership before placing its windows. Each unique animation has one decoder shared by all its visible monitors. Desktop layers use posters until the corresponding frame is ready. Shell releases the renderer when the session locks.

Workspace cards clone the renderer surface directly during playback. They scale it uniformly and clip it to the card. The optional overview pause keeps the decoder and its current frame alive. Removing wallpapers clears both assignments and recovery posters. For portal-applied stills, the native controller saves the GNOME light and dark wallpaper settings and checks the portal image before restoring them.

## Persistent data

The library lives under `$XDG_DATA_HOME/ubrus` (usually `~/.local/share/ubrus`) or `$SNAP_USER_COMMON`. Imports are hashed and decoded before publication. The controller writes the profile and its static fallbacks in one atomic rename. A damaged profile opens read-only.

Deleting a library item first commits its removal and affected assignments. The controller then unlinks the managed media and thumbnail. Pending deletions survive a restart.

If the controller disappears during a change, Shell reads the last committed fallback from the profile. It keeps a cache in `$XDG_DATA_HOME/ubrus-shell` for older or unreadable profiles. The activity log retains two files of up to 512 KiB each.
