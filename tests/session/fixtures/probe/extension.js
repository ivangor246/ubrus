import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';

export default class Probe extends Extension {
    enable() {
        if (!GLib.getenv('UBRUS_TEST_SESSION')) throw Error('Requires an isolated test session');
        this.object = Gio.DBusExportedObject.wrapJSObject(
            '<node><interface name="org.ubrus.Test"><method name="Panel"><arg type="s" direction="in"/><arg type="s" direction="out"/></method><method name="Capture"/><method name="Desktop"/><method name="Overview"><arg type="s" direction="out"/></method><method name="Import"/><method name="Arrange"/><method name="Windows"><arg type="s" direction="out"/></method><method name="Wallpapers"><arg type="s" direction="out"/></method></interface></node>',
            this,
        );
        this.object.export(Gio.DBus.session, '/org/ubrus/Test');
    }

    Panel(command) {
        const bridge = globalThis[Symbol.for('wallpaper@ubrus.org/desktop-bridge')];
        const menu = bridge?.panel.button?.menu;
        if (!menu) return '[]';
        const entries = menu._getMenuItems().filter(item => item.label);
        const labels = entries.map(item => item.label.text);
        if (command) {
            const entry = entries.find(item => item.label.text === command);
            if (!entry) throw Error('Missing panel action: ' + command);
            if (entry.toggle) entry.toggle();
            else entry.activate(null);
        }
        return JSON.stringify(labels);
    }

    Wallpapers() {
        const bridge = globalThis[Symbol.for('wallpaper@ubrus.org/desktop-bridge')];
        return JSON.stringify(bridge.desktop.paths());
    }

    Windows() {
        function surfaces(a) {
            if (!a) return [];
            const row = [a.constructor.name, a.is_obscured ?? null, a.mapped];
            try {
                const t = a.content?.get_texture?.();
                if (t)
                    row.push(
                        t.get_plane?.(0)?.get_components?.(),
                        t.get_plane?.(0)?.get_format?.(),
                        t.get_format?.(),
                        a.get_transformed_position(),
                        [t.get_width(), t.get_height()],
                    );
            } catch {}
            return [row, ...a.get_children().flatMap(surfaces)];
        }
        const ws = global.workspace_manager.get_active_workspace();
        return JSON.stringify({
            overview: Main.overview.visible,
            monitors: Main.layoutManager.monitors.map((m) => [m.x, m.y, m.width, m.height]),
            areas: Main.layoutManager.monitors.map((m) => {
                const r = Main.layoutManager.getWorkAreaForMonitor(m.index);
                return [r.x, r.y, r.width, r.height];
            }),
            windows: ws.list_windows().map((w) => {
                const r = w.get_frame_rect();
                return [
                    w.get_title(),
                    w.get_window_type(),
                    w.get_compositor_private()?.opacity,
                    w.showing_on_its_workspace(),
                    [r.x, r.y, r.width, r.height],
                    w.is_fullscreen(),
                    w.get_pid(),
                    surfaces(w.get_compositor_private()),
                ];
            }),
        });
    }

    Arrange() {
        for (const w of global.workspace_manager.get_active_workspace().list_windows()) {
            const match = /^Ubrus coverage fixture (\d+)$/.exec(w.get_title());
            if (match) {
                w.move_to_monitor(Number(match[1]));
                w.make_fullscreen();
            }
        }
    }

    Import() {
        const window = global.workspace_manager
            .get_active_workspace()
            .list_windows()
            .find((w) => w.get_title() === 'Ubrus');
        if (!window) throw Error('Settings window is closed');
        window.activate(global.get_current_time());
        const seat = global.backend.get_default_seat?.() ?? Clutter.get_default_backend().get_default_seat();
        this.keyboard ??= seat.create_virtual_device(Clutter.InputDeviceType.KEYBOARD_DEVICE);
        const send = (key, state) => this.keyboard.notify_keyval(GLib.get_monotonic_time(), key, state);
        // A headless seat advertises its keyboard capability when the first virtual device appears.
        GLib.timeout_add(GLib.PRIORITY_DEFAULT, 500, () => {
            send(Clutter.KEY_Control_L, Clutter.KeyState.PRESSED);
            send(Clutter.KEY_o, Clutter.KeyState.PRESSED);
            send(Clutter.KEY_o, Clutter.KeyState.RELEASED);
            send(Clutter.KEY_Control_L, Clutter.KeyState.RELEASED);
            return GLib.SOURCE_REMOVE;
        });
    }

    Desktop() {
        Main.overview.hide();
    }

    Overview() {
        Main.overview.show();
        if (Main.overview.animationInProgress) return '[]';
        const bridge = globalThis[Symbol.for('wallpaper@ubrus.org/desktop-bridge')];
        const sizes = [];
        for (const [viewport, index] of bridge.desktop.overviews) {
            const clone = viewport.get_first_child();
            if (!viewport.mapped || !clone?.source || !clone.has_allocation()) continue;
            const [width, height] = viewport.get_size();
            if (!width || !height) continue;
            const [stageWidth, stageHeight] = viewport.get_transformed_size();
            const [cw, ch] = clone.get_allocation_box().get_size();
            sizes.push({
                source: clone.source.get_size(),
                size: [cw * stageWidth / width, ch * stageHeight / height],
                viewport: [...viewport.get_transformed_position(), stageWidth, stageHeight],
                live: clone.source === bridge.rendererWindows.source(index),
            });
        }
        return JSON.stringify(sizes);
    }

    async CaptureAsync(_params, reply) {
        try {
            const file = Gio.File.new_for_path(
                GLib.build_filenamev([GLib.get_user_runtime_dir(), 'ubrus-screen.png']),
            );
            const stream = file.replace(null, false, Gio.FileCreateFlags.PRIVATE, null);
            await new Shell.Screenshot().screenshot(false, stream);
            stream.close(null);
            reply.return_value(null);
        } catch (e) {
            reply.return_dbus_error('org.ubrus.Test.Error', String(e));
        }
    }

    disable() {
        this.keyboard = null;
        this.object.unexport();
    }
}
