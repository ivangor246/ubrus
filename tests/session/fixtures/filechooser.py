"""Serve fixture paths through a private file chooser portal."""

import json
import os
from pathlib import Path

from gi.repository import Gio, GLib

BUS_NAME = 'org.freedesktop.impl.portal.desktop.ubrustest'
OBJECT_PATH = '/org/freedesktop/portal/desktop'
INTERFACE = 'org.freedesktop.impl.portal.FileChooser'
METHODS = ('OpenFile', 'SaveFile')


def interface_info() -> Gio.DBusInterfaceInfo:
    methods = ''.join(
        f"""<method name="{method}">
              <arg name="handle" type="o" direction="in"/>
              <arg name="app_id" type="s" direction="in"/>
              <arg name="parent_window" type="s" direction="in"/>
              <arg name="title" type="s" direction="in"/>
              <arg name="options" type="a{{sv}}" direction="in"/>
              <arg name="response" type="u" direction="out"/>
              <arg name="results" type="a{{sv}}" direction="out"/>
            </method>"""
        for method in METHODS
    )
    xml = f"""<node>
        <interface name="{INTERFACE}">
            {methods}
            <property name="version" type="u" access="read"/>
        </interface>
    </node>"""
    return Gio.DBusNodeInfo.new_for_xml(xml).interfaces[0]


def main() -> None:
    selection = Path(os.environ['XDG_RUNTIME_DIR']) / 'ubrus-test-selection'

    def choose(_bus, _sender, _path, _interface, _method, arguments, reply):
        _, app_id, parent_window, _, options = arguments.unpack()
        try:
            selection.with_suffix('.app-id').write_text(app_id, encoding='utf-8')
            selection.with_suffix('.parent').write_text(parent_window, encoding='utf-8')
            multiple = selection.with_suffix('.json')
            if multiple.is_file() and _method == 'OpenFile':
                if not options.get('multiple'):
                    raise ValueError('Expected a multi-file request')
                paths = json.loads(multiple.read_text(encoding='utf-8'))
            else:
                paths = [selection.read_text(encoding='utf-8')]
            uris = [Path(path).as_uri() for path in paths]
        except (OSError, ValueError) as error:
            reply.return_dbus_error('org.freedesktop.DBus.Error.Failed', str(error))
            return
        results = {'uris': GLib.Variant('as', uris)}
        reply.return_value(GLib.Variant('(ua{sv})', (0, results)))

    bus = Gio.bus_get_sync(Gio.BusType.SESSION)
    bus.register_object(
        OBJECT_PATH, interface_info(), choose, lambda *_: GLib.Variant('u', 3), None
    )
    Gio.bus_own_name_on_connection(bus, BUS_NAME, Gio.BusNameOwnerFlags.NONE, None, None)
    GLib.MainLoop().run()


if __name__ == '__main__':
    main()
