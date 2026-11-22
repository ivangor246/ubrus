import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';
import { DesktopLayers } from './desktop/layers.js';
import { monitorIdentities } from './desktop/monitors.js';
import { RendererWindows } from './desktop/windows.js';
import { workspaceCoverage, previewVisible } from './desktop/visibility.js';
import { pauseReason } from './desktop/geometry.js';
import { ControllerConnection, call } from './controller.js';
import { StillCache } from './stills.js';
import { WallpaperPanel } from './panel.js';
import { Renderer } from './renderer.js';

const DISPLAY = 'org.gnome.Mutter.DisplayConfig';
const OWNER = Symbol.for('wallpaper@ubrus.org/desktop-bridge');
export default class Wallpaper extends Extension {
    enable() {
        if (this.bridgeEnabled) return;
        globalThis[OWNER]?.disable();
        globalThis[OWNER] = this;
        this.bridgeEnabled = true;
        this.refreshing = false;
        this.refreshAgain = false;
        console.info('[Ubrus] Enabling desktop bridge');
        this.cancel = new Gio.Cancellable();
        this.lifetime = new Clutter.Actor();
        this.windows = new WeakSet();
        this.desktop = new DesktopLayers();
        this.rendererWindows = new RendererWindows(this.lifetime, () => this.queue());
        this.outputs = [];
        this.engine = {};
        this.sample = 0;
        this.error = '';
        this.attempts = 0;
        this.stills = new StillCache();
        this.scene = this.stills.available();
        this.panel = new WallpaperPanel(this.dir, command => this.panelAction(command));
        this.panel.sync(this.scene.panel);
        this.monitorIds = new Map();
        this.logicalMonitors = null;
        this.monitorHistoryFile = Gio.File.new_for_path(GLib.build_filenamev([
            GLib.get_user_data_dir(), 'ubrus-shell', 'monitor-identities.json',
        ]));
        try {
            this.savedMonitors = new TextDecoder().decode(this.monitorHistoryFile.load_contents(null)[1]);
            this.monitorHistory = new Map(JSON.parse(this.savedMonitors));
        } catch {
            this.monitorHistory = new Map();
        }
        this.watch(Main.layoutManager, 'monitors-changed', () => this.layout());
        this.watch(global.display, 'workareas-changed', () => this.queue());
        this.watch(global.display, 'restacked', () => this.queue());
        this.watch(global.display, 'window-created', (_, w) => this.watchWindow(w));
        this.watch(global.workspace_manager, 'active-workspace-changed', () => this.queue());
        this.watch(global.window_manager, 'map', (_, a) => {
            this.rendererWindows.claim(a, this.proc);
            if (a.meta_window.get_window_type() !== Meta.WindowType.DESKTOP)
                a.connectObject('transitions-completed', () => this.queue(), this.lifetime);
            this.queue();
        });
        for (const signal of ['size-changed', 'minimize', 'unminimize'])
            this.watch(global.window_manager, signal, () => this.queue());
        this.watch(Main.overview, 'showing', () => this.queue());
        this.watch(Main.overview, 'hidden', () => this.queue());
        for (const a of global.get_window_actors()) this.watchWindow(a.meta_window);
        this.power = Gio.PowerProfileMonitor.dup_default();
        if (this.power) this.watch(this.power, 'notify::power-saver-enabled', () => this.queue());
        this.controller = new ControllerConnection(this.cancel, {
            scene: (scene) => this.acceptScene(scene),
            disconnected: () => this.useStills(),
            inspect: () => {
                this.proc?.inspect();
                this.report(true);
            },
            error: (message) => {
                this.error = message;
                this.report();
            },
        });
        this.useStills();
        this.layout();
    }

    watch(obj, name, fn) {
        obj.connectObject(name, fn, this.lifetime);
    }

    watchWindow(w) {
        if (this.windows.has(w)) return;
        this.windows.add(w);
        for (const signal of [
            'position-changed',
            'size-changed',
            'workspace-changed',
            'notify::minimized',
            'unmanaged',
        ])
            w.connectObject(signal, () => this.queue(), this.lifetime);
        this.queue();
    }

    async layout() {
        const cancel = this.cancel;
        const sequence = (this.layoutSerial = (this.layoutSerial ?? 0) + 1);
        this.layoutReady = false;
        try {
            const [, , logical] = await call(
                DISPLAY,
                '/org/gnome/Mutter/DisplayConfig',
                DISPLAY,
                'GetCurrentState',
                null,
                cancel,
            );
            if (cancel.is_cancelled() || sequence !== this.layoutSerial) return;
            this.logicalMonitors = logical;
            this.identifyMonitors();
            this.layoutReady = true;
        } catch (e) {
            if (cancel.is_cancelled()) return;
            this.error = String(e);
        }
        this.queue();
    }

    identifyMonitors() {
        if (!this.logicalMonitors) return;
        this.monitorIds = monitorIdentities(this.logicalMonitors, this.monitorHistory, this.scene.screens);
        const serialized = JSON.stringify([...this.monitorHistory]);
        if (this.savedMonitors === serialized) return;
        try {
            GLib.mkdir_with_parents(this.monitorHistoryFile.get_parent().get_path(), 0o700);
            this.monitorHistoryFile.replace_contents(serialized, null, false, Gio.FileCreateFlags.PRIVATE, null);
            this.savedMonitors = serialized;
        } catch (error) {
            console.warn(`[Ubrus] Cannot save monitor identities: ${error.message}`);
        }
    }

    panelAction(command) {
        if (!this.controller.online) {
            const snapIds = ['ubrus_ubrus.desktop', 'ubrus_org.ubrus.Ubrus.desktop'];
            const nativeId = 'org.ubrus.Ubrus.desktop';
            const ids = this.scene.snap ? [...snapIds, nativeId] : [nativeId, ...snapIds];
            const app = ids.map(id => Gio.DesktopAppInfo.new(id)).find(Boolean);
            if (!app) return;
            this.pendingPanel = command === 'open' ? null : command;
            app.launch([],null);
            return;
        }
        call('org.ubrus.Ubrus','/org/ubrus/Ubrus/Wallpaper','org.ubrus.Ubrus.Wallpaper1',
            'Panel',new GLib.Variant('(s)',[command]),this.cancel).catch(e => console.warn(e.message));
    }

    acceptScene(scene) {
        this.stills.observe(scene);
        this.scene = scene;
        this.identifyMonitors();
        this.panel.sync(scene.panel);
        if (this.pendingPanel) {
            const command = this.pendingPanel;
            this.pendingPanel = null;
            this.panelAction(command);
        }
        this.error = '';
        this.failedSerial = 0;
        this.restartError = '';
        this.applied = 0;
        this.attempts = 0;
        this.queue();
    }

    useStills() {
        const cancel = this.cancel;
        this.scene = this.stills.available();
        this.panel.sync(this.scene.panel);
        const recovery = this.stills.recover();
        const generation = this.stills.generation;
        recovery.then(scene => {
            if (!scene || cancel.is_cancelled() || generation !== this.stills.generation) return;
            this.scene = scene;
            this.identifyMonitors();
            this.panel.sync(scene.panel);
            this.queue();
        });
        this.applied = 0;
        this.failedSerial = 0;
        this.stop();
        this.queue();
    }

    queue() {
        if (!this.bridgeEnabled || this.cancel.is_cancelled() || this.idle) return;
        if (this.refreshing) {
            this.refreshAgain = true;
            return;
        }
        this.idle = GLib.idle_add(GLib.PRIORITY_DEFAULT_IDLE, () => {
            this.idle = 0;
            this.refreshing = true;
            try {
                this.refresh();
            } catch (e) {
                this.error = String(e);
                this.failedSerial = this.scene.serial ?? 0;
                this.report();
            } finally {
                this.refreshing = false;
                if (this.refreshAgain) {
                    this.refreshAgain = false;
                    this.queue();
                }
            }
            return GLib.SOURCE_REMOVE;
        });
    }

    refresh() {
        const animated = Object.values(this.scene.screens).some((selection) => selection.animated);
        const saving = this.power?.power_saver_enabled;
        const isCovered = workspaceCoverage(
            animated && !this.scene.paused && !(this.scene.power && saving) && !Main.overview.visible,
        );
        const screens = [],
            keep = new Set();
        this.outputs = Main.layoutManager.monitors.map((m) => {
            this.rendererWindows.positionWindow(m);
            const ident = this.monitorIds.get(`${m.x},${m.y}`) ?? { id: '', name: `Monitor ${m.index + 1}` };
            const s = this.scene.screens[ident.id] ?? this.scene.screens['*'];
            // A fullscreen client owns the output even when its buffer carries an alpha channel.
            // For ordinary windows use the compositor's opaque region, preserving visible gaps.
            let reason = 'static';
            if (s?.animated) {
                const area = Main.layoutManager.getWorkAreaForMonitor(m.index);
                const hidden = isCovered(area, this.rendererWindows.obscured(m.index));
                reason = pauseReason(this.scene, saving, hidden, Main.overview.visible);
            }
            if (s) {
                const preparing =
                    this.scene.serial && s.animated && this.engine.displayed?.[m.index] !== s.path;
                if (!preparing)
                    this.desktop.show(m, s, s.animated ? this.rendererWindows.source(m.index) : null);
                keep.add(m.index);
                if (s.animated)
                    screens.push({
                        index: m.index,
                        path: s.path,
                        fit: s.fit,
                        frame: s.frame,
                        active: reason === 'visible',
                        width: m.width,
                        height: m.height,
                    });
            }
            return { ...ident, width: m.width, height: m.height, reason };
        });
        this.desktop.keep(keep);
        this.syncRenderer(screens);
        this.confirmAssignment(screens);
        this.report();
    }

    rendererStatus(status) {
        if (status.error && status.serial !== undefined && status.serial !== (this.scene.serial ?? 0)) return;
        this.engine = { ...status, sample: ++this.sample };
        if (status.error) {
            this.error = status.error;
            this.failedSerial = this.scene.serial ?? 0;
        } else if (this.restartError && this.error === this.restartError && status.windows > 0) {
            this.error = '';
            this.restartError = '';
        }
        this.queue();
        this.report();
    }

    syncRenderer(screens) {
        if (!screens.length) this.stop();
        else if (!this.retry && this.attempts < 4) {
            try {
                if (!this.proc)
                    this.proc = new Renderer(
                        this.scene,
                        (status) => this.rendererStatus(status),
                        () => this.failed('Ubrus renderer exited unexpectedly'),
                    );
                const key = JSON.stringify([screens, this.scene.serial]);
                if (key !== this.last) {
                    this.last = key;
                    this.proc.send(screens, this.scene.serial ?? 0);
                }
            } catch (e) {
                this.failed(String(e));
            }
        }
    }

    confirmAssignment(screens) {
        const target = this.scene.target;
        const targetPresent =
            this.outputs.length > 0 &&
            (!target || target === '*' || this.outputs.some((m) => m.id === target));
        if (this.scene.serial && target && target !== '*' && this.layoutReady && !targetPresent) {
            this.error = 'Selected monitor disconnected';
            this.failedSerial = this.scene.serial;
        }
        this.applied =
            this.scene.serial &&
            this.layoutReady &&
            targetPresent &&
            (!screens.length ||
                (this.engine.serial === this.scene.serial &&
                    screens.every(
                        (s) =>
                            this.rendererWindows.source(s.index) &&
                            this.engine.displayed?.[s.index] === s.path &&
                            (this.engine.speeds?.[s.index] ?? 100) === (s.frame?.speed ?? 100),
                    )))
                ? this.scene.serial
                : 0;
    }

    failed(message) {
        this.stop();
        this.error = message;
        this.restartError = message;
        this.failedSerial = this.scene.serial ?? 0;
        this.report();
        this.attempts++;
        if (this.attempts < 4)
            this.retry = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, 2 ** (this.attempts - 1), () => {
                this.retry = 0;
                this.queue();
                return GLib.SOURCE_REMOVE;
            });
    }

    stop() {
        this.proc?.close();
        this.proc = null;
        this.last = null;
        this.engine = {};
        this.rendererWindows.clear();
        if (this.retry) GLib.source_remove(this.retry);
        this.retry = 0;
    }

    report(force = false) {
        this.controller.report(
            {
                connected: true,
                framing: true,
                preview_visible: previewVisible(this.scene.settings_pid),
                extension_build: this.metadata["build-id"] ?? "development",
                outputs: this.outputs,
                engine: this.engine,
                applied: this.applied ?? 0,
                failed: this.failedSerial ?? 0,
                error: this.error,
            },
            force,
        );
    }

    disable() {
        if (!this.bridgeEnabled) return;
        this.bridgeEnabled = false;
        if (globalThis[OWNER] === this) delete globalThis[OWNER];
        console.info('[Ubrus] Disabling desktop bridge');
        this.panel.destroy();
        this.pendingPanel = null;
        this.controller.close();
        this.cancel.cancel();
        this.stop();
        if (this.idle) GLib.source_remove(this.idle);
        this.idle = 0;
        this.lifetime.destroy();
        this.desktop.destroy();
    }
}
