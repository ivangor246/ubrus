#!/usr/bin/python3
"""Protocol peer for exercising the controller without starting a second Rust executable."""

import json
import os
import sys
import gi

gi.require_version('Gtk', '4.0')
from gi.repository import GLib, Gtk

Gtk.init()
windows = {}
layout = {'screens': [], 'serial': 0}


def receive(*_):
    global layout
    line = sys.stdin.readline()
    if not line:
        loop.quit()
        return False
    if line.strip() != 'status':
        layout = json.loads(line)
        wanted = {s['index'] for s in layout['screens']}
        for i in set(windows) - wanted:
            windows.pop(i).close()
        for s in layout['screens']:
            i = s['index']
            if i not in windows:
                windows[i] = Gtk.Window(
                    title=f'Ubrus surface {i}', decorated=False, focusable=False
                )
            windows[i].set_default_size(s['width'], s['height'])
            windows[i].set_visible(True)
    screens = layout['screens']
    status = {
        'pid': os.getpid(),
        'windows': len(windows),
        'serial': layout['serial'],
        'displayed': {s['index']: s['path'] for s in screens},
        'speeds': {s['index']: s.get('frame', {}).get('speed', 100) for s in screens},
        'streams': [{'decoder': 'fixture GIF', 'playing': any(s['active'] for s in screens)}],
    }
    print('UBRUS/1 ' + json.dumps(status), flush=True)
    return True


loop = GLib.MainLoop()
GLib.io_add_watch(sys.stdin, GLib.PRIORITY_DEFAULT, GLib.IO_IN | GLib.IO_HUP, receive)
loop.run()
