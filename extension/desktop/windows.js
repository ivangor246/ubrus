import GLib from 'gi://GLib';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

export class RendererWindows {
    constructor(lifetime, changed) {
        this.lifetime = lifetime;
        this.changed = changed;
        this.renderWindows = new Map();
        this.sources = new Map();
        this.surfaces = new Map();
    }

    source(index) {
        return this.sources.get(index);
    }

    obscured(index) {
        return this.surfaces.get(index)?.is_obscured === true;
    }

    claim(actor, renderer) {
        const w = actor.meta_window;
        if (!renderer?.owns(w)) return;
        const match = /^Ubrus surface (\d+)$/.exec(w.get_title());
        if (!match) return;
        const index = Number(match[1]);
        renderer.background(w);
        this.forgetWindow(index);
        this.renderWindows.set(index, { window: w });
        this.positionWindow(Main.layoutManager.monitors[index]);
        // Mutter culls the original desktop surface against opaque regions. Its clone paints at full opacity.
        const dim = () => {
            if (actor.opacity !== 1) actor.opacity = 1;
        };
        dim();
        actor.connectObject('notify::opacity', dim, this.lifetime);
        actor.reactive = false;
        const source = actor.get_first_child();
        if (!source) return;
        this.sources.set(index, source);
        const pending = [source];
        while (pending.length) {
            const child = pending.pop();
            if (typeof child.is_obscured === 'boolean') {
                this.surfaces.set(index, child);
                child.connectObject('notify::is-obscured', () => this.changed(), this.lifetime);
                break;
            }
            pending.push(...child.get_children());
        }
        source.connectObject(
            'destroy',
            () => {
                if (this.sources.get(index) === source) {
                    this.sources.delete(index);
                    this.surfaces.delete(index);
                    if (this.renderWindows.get(index)?.window === w) this.forgetWindow(index);
                }
                this.changed();
            },
            this.lifetime,
        );
        this.changed();
    }

    positionWindow(m) {
        const entry = m && this.renderWindows.get(m.index);
        if (!entry) return;
        const geometry = [m.x, m.y, m.width, m.height];
        if (!entry.geometry?.every((n, i) => n === geometry[i])) {
            if (entry.timer) GLib.source_remove(entry.timer);
            entry.timer = 0;
            entry.attempts = 0;
            entry.geometry = geometry;
        }
        if (entry.timer) return;
        const rect = entry.window.get_frame_rect();
        const currentMonitor = entry.window.get_monitor();
        if (
            currentMonitor === m.index &&
            rect.x === m.x && rect.y === m.y && rect.width === m.width && rect.height === m.height
        ) {
            entry.attempts = 0;
            return;
        }
        // After an output reorder Mutter keeps the window on its old physical monitor.
        // Select the new logical monitor before giving the Wayland client its frame.
        if (entry.attempts >= 5) return;
        if (currentMonitor >= 0 && currentMonitor !== m.index) entry.window.move_to_monitor(m.index);
        entry.window.move_resize_frame(false, ...geometry);
        entry.timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 100 * 2 ** entry.attempts++, () => {
            entry.timer = 0;
            this.changed();
            return GLib.SOURCE_REMOVE;
        });
    }

    forgetWindow(index) {
        const entry = this.renderWindows.get(index);
        if (entry?.timer) GLib.source_remove(entry.timer);
        this.renderWindows.delete(index);
    }

    clear() {
        for (const index of this.renderWindows.keys()) this.forgetWindow(index);
        this.sources.clear();
        this.surfaces.clear();
    }
}
