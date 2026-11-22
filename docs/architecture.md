# Architecture

Each GNOME session runs one controller. Launching Ubrus again opens the existing editor. At login the controller can restore wallpapers without opening a window.

| Location | Responsibility |
| --- | --- |
| `src/app/` | Controller, commands, diagnostics and Shell D-Bus interface |
| `src/ui/` | Iced editor, gallery and previews |
| `src/library/` | Imports, thumbnails and saved profiles |
| `src/desktop/` | Desktop portals, autostart and update detection |
| `src/media/`, `src/renderer/` | Decoding, playback and per-monitor GTK windows |
| `extension/` | GNOME panel, monitor placement, visibility and fallback images |

## Wallpaper changes

The native app sends a single still image for all monitors through the wallpaper portal. Animation and per-monitor assignments use the Shell extension. The strict Snap prefers the extension for still images too.

Shell reads the proposed scene over `org.ubrus.Ubrus.Wallpaper1`. The controller saves the assignment after Shell confirms it. The Snap uses a watched file to notify Shell of changes.

Shell starts the renderer and places its windows. Monitors playing the same file at the same speed share a decoder. Each monitor keeps its own crop and brightness. Shell shows a poster until the first frame is ready and stops the renderer when the session locks.

## Storage and recovery

The library lives in `$XDG_DATA_HOME/ubrus` (usually `~/.local/share/ubrus`) or `$SNAP_USER_COMMON`. Only one session can write to it. Damaged profiles and profiles from newer versions open read-only.

- **Import.** A worker copies and hashes each file and prepares its thumbnail. Each file commits separately. After an interruption, `import-change.json` identifies newly created paths that can be removed.
- **Apply.** The controller saves the profile and paths to fallback images in one atomic rename. Before changing a portal wallpaper it stages the profile and writes `wallpaper-change.json`. The native app checks ownership before restoring GNOME's light and dark backgrounds. A confined Snap using only the portal cannot verify ownership after a crash.
- **Delete.** The controller saves the updated assignments before unlinking media. Pending deletions resume at startup.

If the controller exits, Shell shows cached posters while it reads the last committed scene. Its cache in `$XDG_DATA_HOME/ubrus-shell` also records connector identities for monitors with duplicate serials.
