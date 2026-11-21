import Clutter from 'gi://Clutter';
import Shell from 'gi://Shell';
import Cogl from 'gi://Cogl';
import St from 'gi://St';
import Gio from 'gi://Gio';
import GObject from 'gi://GObject';
import Meta from 'gi://Meta';
import GDesktopEnums from 'gi://GDesktopEnums';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import { InjectionManager } from 'resource:///org/gnome/shell/extensions/extension.js';
import { BackgroundManager } from 'resource:///org/gnome/shell/ui/background.js';
import { overviewCrop, frameBounds } from './geometry.js';

const Exposure = GObject.registerClass(
    class UbrusExposure extends Shell.GLSLEffect {
        vfunc_build_pipeline() {
            this.add_glsl_snippet(
                (Cogl.SnippetHook ?? Shell.SnippetHook).FRAGMENT,
                'uniform float gain;',
                'cogl_color_out.rgb *= gain;',
                false,
            );
        }
    },
);

const OverviewWallpaper = GObject.registerClass(
    class UbrusOverviewWallpaper extends Clutter.Actor {
        vfunc_allocate(box) {
            this.set_allocation(box);
            const clone = this.get_first_child();
            if (!clone?.source) return;
            const bounds = overviewCrop(...clone.source.get_size(), ...box.get_size());
            clone.allocate(new Clutter.ActorBox(bounds));
        }
    },
);

export class DesktopLayers {
    constructor() {
        this.layers = new Map();
        this.injections = new InjectionManager();
        this.overviews = new Map();
        const layers = this;
        this.injections.overrideMethod(
            BackgroundManager.prototype,
            '_createBackgroundActor',
            (original) =>
                function () {
                    const actor = original.call(this);
                    if (!this._controlPosition) layers.overviewClone(actor, this._monitorIndex);
                    return actor;
                },
        );
    }

    overviewClone(actor, index) {
        const layer = this.layers.get(index);
        const viewport = new OverviewWallpaper({
            reactive: false,
            visible: Boolean(layer),
            clip_to_allocation: true,
            request_mode: Clutter.RequestMode.CONTENT_SIZE,
        });
        viewport.add_constraint(
            new Clutter.BindConstraint({ source: actor, coordinate: Clutter.BindCoordinate.SIZE }),
        );
        viewport.add_child(
            new Clutter.Clone({ source: layer?.source ?? layer?.box ?? null, reactive: false }),
        );
        actor.add_child(viewport);
        this.overviews.set(viewport, index);
        viewport.connect('destroy', () => this.overviews.delete(viewport));
    }

    setOverviewSource(index, source) {
        for (const [viewport, monitor] of this.overviews) {
            if (monitor !== index) continue;
            viewport.get_first_child().source = source;
            viewport.visible = Boolean(source);
        }
    }

    show(m, selection, source) {
        let layer = this.layers.get(m.index);
        if (!layer) {
            const box = new St.Widget({
                reactive: false,
                clip_to_allocation: true,
                style: 'background-color: black;',
            });
            Main.layoutManager._backgroundGroup.add_child(box);
            layer = { box };
            this.layers.set(m.index, layer);
        }
        layer.box.set_position(m.x, m.y);
        layer.box.set_size(m.width, m.height);
        const path = selection.animated ? selection.poster : selection.path;
        const key = JSON.stringify([
            path,
            selection.fit,
            selection.frame,
            selection.dimensions,
            m.width,
            m.height,
        ]);
        if (key === layer.key && source === layer.source) return;
        layer.box.destroy_all_children();
        layer.key = key;
        layer.path = path;
        layer.source = source;
        if (source) {
            const clone = new Clutter.Clone({ source, reactive: false });
            clone.set_size(m.width, m.height);
            layer.box.add_child(clone);
        } else {
            const background = new Meta.Background({ meta_display: global.display });
            background.set_file(
                Gio.File.new_for_path(path),
                selection.dimensions
                    ? GDesktopEnums.BackgroundStyle.STRETCHED
                    : selection.fit
                      ? GDesktopEnums.BackgroundStyle.SCALED
                      : GDesktopEnums.BackgroundStyle.ZOOM,
            );
            const still = new Meta.BackgroundActor({
                meta_display: global.display,
                monitor: m.index,
                reactive: false,
            });
            still.content.background = background;
            still.set_size(m.width, m.height);
            if (selection.dimensions) {
                const [x, y, w, h] = frameBounds(selection, m.width, m.height);
                still.set_position(x, y);
                still.set_scale(w / m.width, h / m.height);
            }
            if (selection.frame && selection.frame.brightness !== 100) {
                const effect = new Exposure();
                still.add_effect(effect);
                effect.set_uniform_float(effect.get_uniform_location('gain'), 1, [
                    selection.frame.brightness / 100,
                ]);
            }
            layer.box.add_child(still);
        }
        // A mapped clone of the live surface keeps receiving frame damage even
        // when Shell unmaps the desktop group while showing workspaces.
        this.setOverviewSource(m.index, source ?? layer.box);
    }

    keep(indices) {
        for (const [index, layer] of this.layers) {
            if (indices.has(index)) continue;
            this.setOverviewSource(index, null);
            layer.box.destroy();
            this.layers.delete(index);
        }
    }

    paths() {
        return Array.from(this.layers.values(), (layer) => layer.path);
    }

    destroy() {
        this.injections.clear();
        for (const viewport of this.overviews.keys()) viewport.destroy();
        for (const layer of this.layers.values()) layer.box.destroy();
        this.layers.clear();
    }
}
