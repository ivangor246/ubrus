"""Exercise Shell recovery when the controller exits during a wallpaper change."""

import json
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path
from subprocess import Popen

from desktop_session import (
    APP_BUS,
    APP_INTERFACE,
    APP_PATH,
    BUS_PATH,
    BUS_SERVICE,
    DesktopSession,
    read_json,
    wait_for,
)
from gi.repository import Gio, GLib

CONTROLLER_XML = f"""<node>
  <interface name="{APP_INTERFACE}">
    <method name="Scene"><arg type="s" direction="out"/></method>
    <method name="Report"><arg type="s" direction="in"/></method>
  </interface>
</node>"""


class PendingScene:
    def __init__(self, committed: dict, candidate: Path, profile: Path) -> None:
        self.scene = {
            **committed,
            'serial': 42,
            'target': '*',
            'profile_path': str(profile),
            'screens': {'*': {'path': str(candidate), 'fit': False, 'animated': False}},
        }
        self.reports: list[dict] = []

    def handle_call(self, _bus, _sender, _path, _interface, method, arguments, reply):
        if method == 'Scene':
            reply.return_value(GLib.Variant('(s)', (json.dumps(self.scene),)))
        else:
            self.reports.append(json.loads(arguments.unpack()[0]))
            reply.return_value(None)

    def received(self, field: str) -> bool:
        context = GLib.MainContext.default()
        while context.pending():
            context.iteration(False)
        return any(report.get(field) == self.scene['serial'] for report in self.reports)

    @contextmanager
    def publish(self, session: DesktopSession, target: str) -> Iterator[None]:
        self.scene['target'] = target
        self.scene['serial'] += 1
        self.reports.clear()
        # DO_NOT_QUEUE must return PRIMARY_OWNER before Shell can query this scene.
        result = session.call(
            BUS_SERVICE, BUS_PATH, BUS_SERVICE, 'RequestName', GLib.Variant('(su)', (APP_BUS, 4))
        )
        assert result == (1,), f'Could not acquire {APP_BUS}: {result}'
        try:
            yield
        finally:
            session.call(
                BUS_SERVICE, BUS_PATH, BUS_SERVICE, 'ReleaseName', GLib.Variant('(s)', (APP_BUS,))
            )


def save_fallback(profile_path: Path, screens: dict) -> None:
    profile = read_json(profile_path)
    profile['wallpaper_stills'] = {'version': 1, 'screens': screens}
    temporary = profile_path.with_suffix('.tmp')
    temporary.write_text(json.dumps(profile), encoding='utf-8')
    temporary.replace(profile_path)


def check_controller_loss(session: DesktopSession, app: Popen) -> None:
    cache = session.data_dir / 'ubrus-shell/still.json'
    committed = read_json(cache)
    previous = committed['screens']['*']['path']

    def wallpapers() -> set[str]:
        return set(json.loads(session.probe('Wallpapers')[0]))

    app.terminate()
    app.wait(timeout=8)
    wait_for(
        'the last committed wallpaper',
        lambda: not session.owns_name(APP_BUS) and wallpapers() == {previous},
    )

    candidate = session.library / 'thumbs/candidate.png'
    candidate.write_bytes(Path(previous).read_bytes())
    pending = PendingScene(committed, candidate, session.profile_path)
    interface = Gio.DBusNodeInfo.new_for_xml(CONTROLLER_XML).interfaces[0]
    registration = session.bus.register_object(APP_PATH, interface, pending.handle_call, None, None)
    try:
        for target, commit in [('disconnected-output', False), ('*', False), ('*', True)]:
            with pending.publish(session, target):
                if target == '*':
                    wait_for('Shell to apply the candidate', lambda: pending.received('applied'))
                    assert wallpapers() == {str(candidate)}
                    if commit:
                        save_fallback(session.profile_path, pending.scene['screens'])
                else:
                    wait_for(
                        'Shell to reject the disconnected output',
                        lambda: pending.received('failed'),
                    )
                    assert not pending.received('applied')
                    # A malformed fallback must also leave the previously committed image intact.
                    save_fallback(session.profile_path, {'*': None})
            expected = str(candidate) if commit else previous
            wait_for(
                f'Shell to restore {expected}',
                lambda expected=expected: wallpapers() == {expected}
                and read_json(cache)['screens']['*']['path'] == expected,
            )
    finally:
        session.bus.unregister_object(registration)
        candidate.unlink()

    session.extension('DisableExtension')
    session.extension('EnableExtension')
    wait_for('Shell to discard the deleted fallback', lambda: not wallpapers())
    session.log('controller loss before and after commit restored the saved wallpaper')
