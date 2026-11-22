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

A still image for all monitors goes through the desktop wallpaper portal in the native app. A strict Snap uses the extension when connected to avoid replacing a shared system wallpaper. Other assignments use the extension. Shell reads a proposed scene over `org.ubrus.Ubrus.Wallpaper1`, matches its target to a monitor and reports whether it was applied. The controller commits the assignment after confirmation. The strict Snap wakes Shell through a watched file for scene changes and status requests.

Shell launches the Rust renderer and verifies ownership before placing its windows. Monitors with the same media and playback rate share a decoder. Each GTK surface applies its own crop, placement and brightness. The preview, portal image and Shell recovery poster use the same normalized framing. Desktop layers use posters until the corresponding frame is ready. Shell releases the renderer when the session locks.

Workspace cards clone the renderer surface directly during playback. They scale it uniformly and clip it to the card. The optional overview pause keeps the decoder and its current frame alive. Hiding or disabling Ubrus publishes an empty visible scene while retaining assignments. Shell also caches the committed panel state and language for periods when the controller is offline. For portal-applied stills, the native controller saves the GNOME light and dark wallpaper settings and checks the portal image before restoring them.

## Persistent data

The library lives under `$XDG_DATA_HOME/ubrus` (usually `~/.local/share/ubrus`) or `$SNAP_USER_COMMON`. One login session holds the library's writer lock during recovery and editing. A second session can inspect the saved library. Damaged profiles and profiles from newer versions open read-only.

Imports copy, hash and prepare a thumbnail in a worker. Each file in a multi-file selection commits separately. Cancellation keeps files already committed. Before publishing new media or thumbnails, the controller records their paths in `import-change.json`. Recovery can then remove interrupted imports without touching media from an older or missing profile. The gallery decodes thumbnails for the visible rows and the selected editor item on demand.

The controller writes the profile and its static fallbacks in one atomic rename. For global stills it stages the profile before contacting the wallpaper portal. The native controller journals the change in `wallpaper-change.json` and checks the installed image before restoring either GNOME theme's background. A strict Snap without its companion extension can apply a still through the portal, but the portal gives it no way to identify or restore the previous host image after a crash.

Deleting a library item first commits its removal and affected assignments. The controller then unlinks the managed media and thumbnail. Pending deletions survive a restart.

If the controller disappears during a change, Shell first displays its cached posters, then asynchronously reads the last committed fallback from the profile. Under `$XDG_DATA_HOME/ubrus-shell` it also remembers which connector distinguished two displays with the same hardware serial. The activity log retains two files of up to 512 KiB each. The full diagnostic report includes local details. The public report constructs its text from an allowlist of application and playback fields.
