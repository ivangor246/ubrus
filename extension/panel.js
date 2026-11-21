import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import St from 'gi://St';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

export class WallpaperPanel {
    constructor(dir, activate) {
        this.dir = dir;
        this.activate = activate;
        this.catalog = JSON.parse(
            new TextDecoder().decode(dir.get_child('locale.json').load_contents(null)[1]),
        );
    }

    icon(name) {
        return new Gio.FileIcon({ file: this.dir.get_child('icons').get_child(name + '-symbolic.svg') });
    }

    sync(p) {
        const key = JSON.stringify(p ?? null);
        if (this.key === key) return;
        this.key = key;
        if (!p?.visible) {
            this.button?.destroy();
            this.button = null;
            return;
        }
        if (!this.button) {
            this.button = new PanelMenu.Button(0, 'Ubrus');
            const box = new St.BoxLayout();
            box.add_child(new St.Icon({ gicon: this.icon('ubrus'), style_class: 'system-status-icon' }));
            this.light = new St.Label({ y_align: Clutter.ActorAlign.CENTER });
            box.add_child(this.light);
            this.button.add_child(box);
            Main.panel.addToStatusArea('ubrus', this.button);
        }
        const t = this.catalog[p.language] ?? this.catalog.en;
        const state = !p.enabled
            ? 'Disabled'
            : p.hidden || !p.assigned
              ? 'Hidden'
              : p.paused && p.animated
                ? 'Paused'
                : 'Running';
        this.light.text = { Disabled: '○', Hidden: '–', Paused: 'Ⅱ', Running: '●' }[state];
        this.light.style =
            'font-size: 9pt;' + ({ Running: 'color: #7dd8b5;', Paused: 'color: #f1bd79;' }[state] ?? '');
        this.button.accessible_name = 'Ubrus · ' + t[state];
        this.button.menu.removeAll();
        const add = (command, label, icon, active = true) => {
            const entry = new PopupMenu.PopupImageMenuItem(t[label], this.icon(icon));
            entry.setSensitive(active);
            entry.connect('activate', () => this.activate(command));
            this.button.menu.addMenuItem(entry);
        };
        add('open', 'OpenApp', 'open');
        add(
            p.paused ? 'resume' : 'pause',
            p.paused ? 'Resume' : 'Pause',
            p.paused ? 'play' : 'pause',
            p.enabled && !p.hidden && p.animated,
        );
        add(
            p.hidden ? 'show' : 'hide',
            p.hidden ? 'ShowWallpaper' : 'ClearWallpapers',
            p.hidden ? 'show' : 'hide',
            p.enabled && p.assigned,
        );
        this.button.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        add(p.enabled ? 'disable' : 'enable', p.enabled ? 'Disable' : 'Enable', 'power');
        const indicator = new PopupMenu.PopupSwitchMenuItem(t.Panel, true);
        indicator.connect('toggled', () => this.activate('hide-icon'));
        this.button.menu.addMenuItem(indicator);
    }

    destroy() {
        this.button?.destroy();
    }
}
