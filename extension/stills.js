import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

const empty = () => ({ version: 1, screens: {}, power: false, paused: false });
// A disabled extension can still have an asynchronous replacement in flight.
let pendingWrite = Promise.resolve();

function validStills(scene) {
    return (
        scene?.version === 1 &&
        !scene.serial &&
        scene.screens &&
        typeof scene.screens === 'object' &&
        !Array.isArray(scene.screens) &&
        Object.values(scene.screens).every(
            (s) => s?.animated === false && typeof s.path === 'string' && GLib.path_is_absolute(s.path),
        )
    );
}

function availableStills(scene) {
    // The companion extension can outlive a purged or moved media library.
    return {
        ...scene,
        screens: Object.fromEntries(
            Object.entries(scene.screens).filter(([, s]) => GLib.file_test(s.path, GLib.FileTest.IS_REGULAR)),
        ),
    };
}

export class StillCache {
    constructor(
        file = Gio.File.new_for_path(
            GLib.build_filenamev([GLib.get_user_data_dir(), 'ubrus-shell', 'still.json']),
        ),
    ) {
        this.saved = null;
        this.generation = 0;
        this.writes = Promise.resolve();
        this.profilePath = null;
        this.committed = empty();
        this.file = file;
        // Only static fallbacks survive without the Rust application. GNOME activates video at graphical login.
        try {
            const still = JSON.parse(new TextDecoder().decode(this.file.load_contents(null)[1]));
            if (validStills(still)) {
                this.committed = still;
                this.profilePath = still.profile_path;
            }
        } catch {}
    }

    available() {
        return availableStills(this.committed);
    }

    observe(scene) {
        this.generation++;
        this.profilePath = scene.profile_path;
        if (!scene.serial) this.remember(this.stillScene(scene));
    }

    async recover() {
        const generation = ++this.generation;
        // The controller can exit after its atomic commit and before Shell receives Changed.
        if (this.profilePath) {
            try {
                const file = Gio.File.new_for_path(this.profilePath);
                const contents = await new Promise((resolve, reject) =>
                    file.load_contents_async(null, (source, result) => {
                        try { resolve(source.load_contents_finish(result)[1]); }
                        catch (error) { reject(error); }
                    }));
                if (generation !== this.generation) return null;
                const { wallpaper_stills: still } = JSON.parse(new TextDecoder().decode(contents));
                if (validStills(still)) this.remember({ ...still, snap: this.committed.snap, profile_path: this.profilePath });
            } catch {} // Older or damaged profiles retain the last confirmed Shell cache.
        }
        return generation === this.generation ? this.available() : null;
    }

    stillScene(scene) {
        const still = { version: 1, panel: scene.panel, snap: scene.snap, profile_path: scene.profile_path, screens: {} };
        for (const [id, s] of Object.entries(scene.screens)) {
            still.screens[id] = s.animated ? { ...s, animated: false, path: s.poster } : s;
        }
        return still;
    }

    remember(still) {
        this.committed = still;
        const data = JSON.stringify(still);
        if (data === this.saved) return;
        this.saved = data;
        pendingWrite = this.writes = pendingWrite.then(async () => {
            if (data !== this.saved) return;
            GLib.mkdir_with_parents(this.file.get_parent().get_path(), 0o700);
            await new Promise((resolve, reject) =>
                this.file.replace_contents_bytes_async(new GLib.Bytes(new TextEncoder().encode(data)), null,
                    false, Gio.FileCreateFlags.PRIVATE, null, (file, result) => {
                        try { file.replace_contents_finish(result); resolve(); }
                        catch (error) { reject(error); }
                    }));
        }).catch(error => {
            if (this.saved === data) this.saved = null;
            console.warn(`[Ubrus] Cannot cache wallpaper: ${error.message}`);
        });
    }
}
