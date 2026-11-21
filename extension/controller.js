import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

const BUS = 'org.ubrus.Ubrus';
const PATH = '/org/ubrus/Ubrus/Wallpaper';
const IFACE = 'org.ubrus.Ubrus.Wallpaper1';

export function call(name, path, iface, method, args, cancel) {
    return new Promise((ok, fail) =>
        Gio.DBus.session.call(
            name,
            path,
            iface,
            method,
            args,
            null,
            Gio.DBusCallFlags.NO_AUTO_START,
            5000,
            cancel,
            (bus, res) => {
                try {
                    ok(bus.call_finish(res).deepUnpack());
                } catch (e) {
                    fail(new Error(`${method}: ${e.message}`));
                }
            },
        ),
    );
}

export class ControllerConnection {
    constructor(cancel, events) {
        this.cancel = cancel;
        this.events = events;
        this.generation = 0;
        this.fetchAttempts = 0;
        this.sub = Gio.DBus.session.signal_subscribe(
            BUS,
            IFACE,
            null,
            PATH,
            null,
            Gio.DBusSignalFlags.NONE,
            (_c, _s, _p, _i, name) => {
                if (name === 'Changed') this.fetch();
                if (name === 'Inspect') {
                    this.events.inspect();
                }
            },
        );
        this.owner = Gio.bus_watch_name_on_connection(
            Gio.DBus.session,
            BUS,
            Gio.BusNameWatcherFlags.NONE,
            () => {
                this.online = true;
                this.reported = null;
                this.fetch();
            },
            () => {
                this.online = false;
                this.generation++;
                this.watchWakeDirectory(null);
                this.events.disconnected();
            },
        );
    }

    async fetch() {
        if (this.fetchRetry) GLib.source_remove(this.fetchRetry);
        this.fetchRetry = 0;
        const generation = ++this.generation;
        const cancel = this.cancel;
        try {
            const [wire] = await call(BUS, PATH, IFACE, 'Scene', null, cancel);
            if (generation !== this.generation || cancel.is_cancelled()) return;
            const scene = JSON.parse(wire);
            if (scene.version !== 1) throw Error('Incompatible Ubrus protocol');
            this.watchWakeDirectory(scene.wake_dir);
            this.fetchAttempts = 0;
            this.events.scene(scene);
        } catch (e) {
            if (cancel.is_cancelled() || generation !== this.generation) return;
            this.events.error(String(e));
            if (this.online && this.fetchAttempts < 4) {
                const delay = 2 ** this.fetchAttempts++;
                this.fetchRetry = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, delay, () => {
                    this.fetchRetry = 0;
                    if (this.online) this.fetch();
                    return GLib.SOURCE_REMOVE;
                });
            }
        }
    }

    watchWakeDirectory(path) {
        if (this.wakePath === path) return;
        this.wakeMonitor?.cancel();
        this.wakeMonitor = null;
        this.wakePath = null;
        if (!path) return;
        this.wakeMonitor = Gio.File.new_for_path(path).monitor_directory(
            Gio.FileMonitorFlags.NONE,
            this.cancel,
        );
        this.wakePath = path;
        this.wakeMonitor.connect('changed', (_monitor, file, _other, event) => {
            if (!this.online || event !== Gio.FileMonitorEvent.CHANGES_DONE_HINT) return;
            if (file.get_basename() === 'shell-changed') this.fetch();
            if (file.get_basename() === 'shell-inspect') {
                this.events.inspect();
            }
        });
    }

    report(status, force) {
        if (!this.online) return;
        const wire = JSON.stringify(status);
        if (!force && this.reported === wire) return;
        this.reported = wire;
        call(BUS, PATH, IFACE, 'Report', new GLib.Variant('(s)', [wire]), this.cancel).catch(() => {});
    }

    close() {
        this.watchWakeDirectory(null);
        if (this.online)
            Gio.DBus.session.call(
                BUS,
                PATH,
                IFACE,
                'Report',
                new GLib.Variant('(s)', ['{"connected":false}']),
                null,
                Gio.DBusCallFlags.NO_AUTO_START,
                1000,
                null,
                null,
            );
        this.online = false;
        this.reported = null;
        if (this.fetchRetry) GLib.source_remove(this.fetchRetry);
        this.fetchRetry = 0;
        Gio.bus_unwatch_name(this.owner);
        Gio.DBus.session.signal_unsubscribe(this.sub);
    }
}
