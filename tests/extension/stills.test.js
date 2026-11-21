import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import { StillCache } from '../../extension/stills.js';

const root = Gio.File.new_for_path(GLib.dir_make_tmp('ubrus-stills-XXXXXX'));
const file = (name) => root.get_child(name);
const cache = file('still.json');
const profile = file('profile.json');
const poster = file('poster.png');
const next = file('next.png');
const committedScreens = (path) => ({ '*': { animated: false, path } });
const scene = (screens) => ({ version: 1, profile_path: profile.get_path(), screens });
function expectPath(actual, expected) {
    if (actual.screens['*']?.path !== expected) throw Error(`Unexpected fallback: ${JSON.stringify(actual)}`);
}

try {
    for (const image of [poster, next]) GLib.file_set_contents(image.get_path(), '');
    const live = new StillCache(cache);
    live.observe(scene({ '*': { animated: true, path: '/library/loop.webm', poster: poster.get_path() } }));
    expectPath(new StillCache(cache).available(), poster.get_path());

    live.observe({ ...scene(committedScreens(next.get_path())), serial: 42 });
    expectPath(live.recover(), poster.get_path());
    GLib.file_set_contents(
        profile.get_path(),
        JSON.stringify({ wallpaper_stills: { version: 1, screens: { '*': null } } }),
    );
    expectPath(live.recover(), poster.get_path());

    GLib.file_set_contents(
        profile.get_path(),
        JSON.stringify({
            wallpaper_stills: { version: 1, screens: committedScreens(next.get_path()) },
        }),
    );
    expectPath(live.recover(), next.get_path());
    next.delete(null);
    expectPath(new StillCache(cache).available(), undefined);
    GLib.file_set_contents(next.get_path(), '');
    GLib.file_set_contents(
        profile.get_path(),
        JSON.stringify({ wallpaper_stills: { version: 1, screens: {} } }),
    );
    expectPath(new StillCache(cache).recover(), undefined);
    expectPath(new StillCache(cache).available(), undefined);
    print('Wallpaper recovery before and after a profile commit passed');
} finally {
    for (const entry of [poster, next, profile, cache]) {
        if (entry.query_exists(null)) entry.delete(null);
    }
    root.delete(null);
}
