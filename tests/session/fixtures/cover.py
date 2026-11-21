"""Cover selected monitors with opaque GTK windows in the private test session."""

import argparse
import json

import cairo
import gi

gi.require_version('Gtk', '4.0')
gi.require_version('Gdk', '4.0')
from gi.repository import Gdk, Gio, GLib, Gtk  # noqa: E402


def paint_background(
    _area: Gtk.DrawingArea, context: cairo.Context, _width: int, _height: int
) -> None:
    context.set_source_rgb(0.15, 0.2, 0.3)
    context.paint()


def mark_opaque(surface: Gdk.Surface, width: int, height: int) -> None:
    surface.set_opaque_region(cairo.Region(cairo.RectangleInt(0, 0, width, height)))


def on_realize(window: Gtk.Window) -> None:
    window.get_surface().connect('layout', mark_opaque)


def cover_monitor(index: int, monitor: Gdk.Monitor) -> Gtk.Window:
    window = Gtk.Window(title=f'Ubrus coverage fixture {index}')
    window.set_decorated(False)
    canvas = Gtk.DrawingArea()
    canvas.set_draw_func(paint_background)
    window.set_child(canvas)
    window.connect('realize', on_realize)
    window.fullscreen_on_monitor(monitor)
    window.present()
    return window


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('monitors', type=int, nargs='+', metavar='INDEX')
    args = parser.parse_args()
    Gtk.init()
    bus = Gio.bus_get_sync(Gio.BusType.SESSION)
    reply = bus.call_sync(
        'org.gnome.Shell',
        '/org/ubrus/Test',
        'org.ubrus.Test',
        'Windows',
        None,
        None,
        Gio.DBusCallFlags.NO_AUTO_START,
        5000,
        None,
    )
    layout = json.loads(reply.unpack()[0])['monitors']
    monitors = {}
    for monitor in Gdk.Display.get_default().get_monitors():
        geometry = monitor.get_geometry()
        monitors[geometry.x, geometry.y] = monitor

    windows = []
    for index in args.monitors:
        x, y, _, _ = layout[index]
        windows.append(cover_monitor(index, monitors[x, y]))
    GLib.MainLoop().run()


if __name__ == '__main__':
    main()
