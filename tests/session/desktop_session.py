"""Process ownership, files and D-Bus endpoints of one isolated GNOME session."""

import json
import os
import subprocess
import time
from collections.abc import Callable
from pathlib import Path
from typing import TypeVar

from gi.repository import Gio, GLib

SESSION_DIR = Path(__file__).resolve().parent
PROJECT_ROOT = SESSION_DIR.parents[1]
MEDIA_DIR = SESSION_DIR.parent / 'media'
EXTENSION = 'wallpaper@ubrus.org'
PROBE_EXTENSION = 'probe@ubrus.test'
APP_BUS = 'org.ubrus.Ubrus'
APP_PATH = '/org/ubrus/Ubrus/Wallpaper'
APP_INTERFACE = 'org.ubrus.Ubrus.Wallpaper1'
BUS_SERVICE = 'org.freedesktop.DBus'
BUS_PATH = '/org/freedesktop/DBus'
T = TypeVar('T')


def wait_for(description: str, condition: Callable[[], T | None], timeout: float = 20) -> T:
    deadline = time.monotonic() + timeout
    last_error = None
    while time.monotonic() < deadline:
        try:
            result = condition()
            if result:
                return result
        except GLib.Error as error:
            # A service may be acquiring its name or exporting its objects.
            last_error = error
        time.sleep(0.12)
    raise TimeoutError(f'Timed out waiting for {description}') from last_error


def read_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding='utf-8'))


class DesktopSession:
    def __init__(self, mode: str) -> None:
        self.mode = mode
        self.data_dir = Path(os.environ['XDG_DATA_HOME'])
        self.runtime_dir = Path(os.environ['XDG_RUNTIME_DIR'])
        self.library = Path(os.environ.get('UBRUS_TEST_LIBRARY', self.data_dir / 'ubrus'))
        self.profile_path = self.library / 'profile.json'
        self.selection = self.runtime_dir / 'ubrus-test-selection'
        self.binary = os.environ['UBRUS_TEST_BINARY']
        self.installed = bool(os.environ.get('UBRUS_TEST_INSTALLED'))
        self.bus = Gio.bus_get_sync(Gio.BusType.SESSION)
        self.children: list[subprocess.Popen] = []

    def spawn(self, *command: str, env: dict[str, str] | None = None) -> subprocess.Popen:
        process = subprocess.Popen(command, stdin=subprocess.DEVNULL, env=env)
        self.children.append(process)
        return process

    def close(self) -> None:
        for process in reversed(self.children):
            if process.poll() is None:
                process.terminate()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()

    def log(self, message: str) -> None:
        print(f'{self.mode}: {message}', flush=True)

    def call(
        self, service: str, path: str, interface: str, method: str, args: GLib.Variant | None = None
    ) -> tuple:
        reply = self.bus.call_sync(
            service,
            path,
            interface,
            method,
            args,
            None,
            Gio.DBusCallFlags.NO_AUTO_START,
            12000,
            None,
        )
        return reply.unpack()

    def owns_name(self, name: str) -> bool:
        return self.call(
            BUS_SERVICE, BUS_PATH, BUS_SERVICE, 'NameHasOwner', GLib.Variant('(s)', (name,))
        )[0]

    def extension(self, method: str, uuid: str = EXTENSION) -> tuple:
        return self.call(
            'org.gnome.Shell',
            '/org/gnome/Shell',
            'org.gnome.Shell.Extensions',
            method,
            GLib.Variant('(s)', (uuid,)),
        )

    def probe(self, method: str) -> tuple:
        return self.call('org.gnome.Shell', '/org/ubrus/Test', 'org.ubrus.Test', method)

    def window_layout(self) -> dict:
        return json.loads(self.probe('Windows')[0])

    def status(self) -> dict:
        return json.loads(self.call(APP_BUS, APP_PATH, APP_INTERFACE, 'Status')[0])

    def snapshot(self) -> dict:
        previous_sample = (self.status().get('engine') or {}).get('sample')
        self.call(APP_BUS, APP_PATH, APP_INTERFACE, 'Refresh')

        def fresh_status() -> dict | None:
            status = self.status()
            sample = (status.get('engine') or {}).get('sample')
            if sample is not None and sample != previous_sample:
                return status
            return None

        return wait_for('a fresh renderer sample', fresh_status, timeout=8)

    def profile(self) -> dict:
        return read_json(self.profile_path)
