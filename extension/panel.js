import Gio from 'gi://Gio';
import St from 'gi://St';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

const stateIcons = {
    Disabled: 'status-disabled',
    Hidden: 'status-hidden',
    Paused: 'pause',
    Running: 'status-running',
};

export class WallpaperPanel {
    constructor(dir, activate) {
        this.dir = dir;
        this.activate = activate;
        this.catalog = JSON.parse(
            new TextDecoder().decode(dir.get_child('locale.json').load_contents(null)[1]),
        );
    }

    icon(name) {
        // Shell recolors SVG fills. Draw panel symbols with filled shapes so the whole glyph follows the theme.
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
            box.add_child(
                new St.Icon({
                    gicon: this.icon('ubrus'),
                    style_class: 'system-status-icon',
                    style: '-st-icon-style: symbolic;',
                }),
            );
            this.stateIcon = new St.Icon({ style: 'icon-size: 0.95em; -st-icon-style: symbolic;' });
            box.add_child(this.stateIcon);
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
        this.stateIcon.gicon = this.icon(stateIcons[state]);
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
        add('hide-icon', 'HidePanelIcon', 'panel');
    }

    destroy() {
        this.button?.destroy();
    }
}
