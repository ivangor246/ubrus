"""Drive the Iced settings window and check its visible state under X11."""

import json
import os
import re
import subprocess
from itertools import groupby
from pathlib import Path
from subprocess import Popen

import gi
from desktop_session import APP_BUS, MEDIA_DIR, DesktopSession, wait_for
from PIL import Image, ImageGrab

gi.require_version('Gtk', '4.0')
gi.require_version('Gdk', '4.0')
from gi.repository import Gdk, GLib, Gtk  # noqa: E402

SETTINGS_WINDOW = ('--onlyvisible', '--name', '^Ubrus$')
# Coordinates within the fixed-size integration window. Both radio groups use these columns.
RADIO_X = {'system': 28, 'light': 197, 'dark': 366, 'english': 197, 'russian': 366}
HEADING_BOX = (15, 80, 185, 130)


def xdotool(*args: str) -> str:
    return subprocess.check_output(['xdotool', *args], text=True, timeout=6).strip()


class SettingsWindow:
    def __init__(self) -> None:
        self.id = xdotool('search', '--sync', *SETTINGS_WINDOW).splitlines()[-1]
        self.focus()
        info = subprocess.check_output(['xwininfo', '-id', self.id], text=True, timeout=6)
        coordinates = dict(re.findall(r'Absolute upper-left ([XY]):\s*(-?\d+)', info))
        if coordinates.keys() != {'X', 'Y'}:
            raise RuntimeError(f'Could not read settings window position from xwininfo:\n{info}')
        self.left = int(coordinates['X'])
        self.top = int(coordinates['Y'])
        wait_for('Iced to draw the Ubrus heading', self.heading_is_drawn)

    def focus(self) -> None:
        xdotool('windowactivate', '--sync', self.id)

    def key(self, shortcut: str) -> None:
        xdotool('key', '--clearmodifiers', shortcut)

    def click(self, x: int, y: int) -> None:
        xdotool('mousemove', str(self.left + x), str(self.top + y), 'click', '1')

    def drag(self, start: tuple[int, int], end: tuple[int, int]) -> None:
        xdotool(
            'mousemove', str(self.left + start[0]), str(self.top + start[1]),
            'mousedown', '1',
            'mousemove', str(self.left + end[0]), str(self.top + end[1]),
            'mouseup', '1',
        )

    def capture(self) -> Image.Image:
        bounds = (self.left, self.top, self.left + 550, self.top + 380)
        return ImageGrab.grab(bbox=bounds, xdisplay=os.environ['DISPLAY'])

    def heading_is_drawn(self) -> bool:
        # The Ubrus label in the navigation bar is visible in both themes and languages.
        label = self.capture().crop((20, 20, 105, 55)).convert('L')
        darkest, lightest = label.getextrema()
        return lightest - darkest > 70

    def dark(self) -> bool:
        return max(self.capture().getpixel((5, 100))[:3]) < 110

    def heading(self) -> bytes:
        return self.capture().crop(HEADING_BOX).tobytes()

    def selected_radio_rows(self) -> tuple[int, int] | None:
        # Locate the filled centers of both System radio buttons, ignoring thin outlines.
        image = self.capture().convert('RGB')

        def is_accent_pixel(y: int) -> bool:
            red, _, blue = image.getpixel((RADIO_X['system'], y))
            return blue - red > 70

        centers = []
        for colored, run in groupby(range(105, 340), key=is_accent_pixel):
            if not colored:
                continue
            rows = list(run)
            if len(rows) >= 4:
                centers.append(sum(rows) // len(rows))
        if len(centers) == 2 and 60 <= centers[1] - centers[0] <= 85:
            return centers[0], centers[1]
        return None


def select_option(
    session: DesktopSession, window: SettingsWindow, setting: str, value: str, row: int
) -> None:
    window.click(RADIO_X[value], row)
    wait_for(
        f'{setting} preference to be saved as {value}', lambda: session.profile()[setting] == value
    )


def check_themes(session: DesktopSession, window: SettingsWindow, row: int) -> None:
    for scheme, dark in [('prefer-dark', True), ('default', False)]:
        subprocess.run(
            ['gsettings', 'set', 'org.gnome.desktop.interface', 'color-scheme', scheme], check=True
        )
        wait_for(
            f"window to follow GNOME's {scheme} scheme", lambda dark=dark: window.dark() == dark
        )
    for theme, dark in [('dark', True), ('light', False), ('system', False)]:
        select_option(session, window, 'theme', theme, row)
        wait_for(f'the {theme} theme to be visible', lambda dark=dark: window.dark() == dark)


def check_languages(session: DesktopSession, window: SettingsWindow, row: int) -> bytes:
    english = window.heading()
    select_option(session, window, 'language', 'russian', row)
    wait_for('a translated settings heading', lambda: window.heading() != english)
    russian = window.heading()
    select_option(session, window, 'language', 'english', row)
    wait_for('the English settings heading', lambda: window.heading() == english)
    select_option(session, window, 'language', 'system', row)
    return russian


def check_preferences(
    session: DesktopSession, app: Popen, window: SettingsWindow
) -> tuple[Popen, SettingsWindow]:
    window.key('ctrl+comma')
    theme_row, language_row = wait_for('both System radio buttons', window.selected_radio_rows)
    check_themes(session, window, theme_row)
    russian_heading = check_languages(session, window, language_row)

    select_option(session, window, 'language', 'english', language_row)
    select_option(session, window, 'theme', 'dark', theme_row)
    wait_for('dark theme before restarting', window.dark)
    english_dark = window.heading()
    window.key('alt+F4')
    app.wait(timeout=8)
    wait_for('the controller to exit', lambda: not session.owns_name(APP_BUS))

    # A Russian login should preserve the explicit English override until System is selected.
    app = session.spawn(session.binary, env={**os.environ, 'LANGUAGE': 'ru'})
    window = SettingsWindow()
    wait_for('the restored dark theme', window.dark)
    window.key('ctrl+comma')
    wait_for('the restored English heading', lambda: window.heading() == english_dark)
    select_option(session, window, 'theme', 'system', theme_row)
    wait_for('the system light theme', lambda: not window.dark())
    select_option(session, window, 'language', 'system', language_row)
    wait_for('the Russian system language', lambda: window.heading() == russian_heading)

    check_diagnostics(session, window, russian_heading)
    session.log(
        'followed live GNOME theme changes; switched both languages and themes; reopened with saved overrides'
    )
    return app, window


def clipboard_text() -> str:
    clipboard = Gdk.Display.get_default().get_clipboard()
    result = []

    def received(source, response):
        try:
            result.append(source.read_text_finish(response) or '')
        except GLib.Error:
            result.append('')

    clipboard.read_text_async(None, received)

    def ready() -> bool:
        while GLib.MainContext.default().iteration(False):
            pass
        return bool(result)

    wait_for('the clipboard owner to respond', ready, timeout=3)
    return result[0]


def check_diagnostics(
    session: DesktopSession, window: SettingsWindow, settings_heading: bytes
) -> None:
    window.click(830, 38)
    wait_for('the diagnostics page', lambda: window.heading() != settings_heading)
    Gtk.init()
    window.drag((25, 197), (67, 197))
    window.key('ctrl+c')
    wait_for('the mouse-selected word', lambda: clipboard_text() == 'Сеанс')
    window.key('ctrl+Home')
    window.key('shift+End')
    window.key('ctrl+c')
    wait_for('one selected diagnostics line', lambda: clipboard_text() == 'Сеанс x11')
    window.key('ctrl+a')
    window.key('ctrl+c')
    wait_for('the full diagnostics selection', lambda: 'Расширение GNOME' in clipboard_text())

    # The primary export must succeed without asking the file chooser for a path.
    session.selection.unlink(missing_ok=True)
    window.click(175, 150)
    downloads = session.runtime_dir / 'downloads'
    wait_for(
        'a directly saved Russian report',
        lambda: any(
            'Расширение GNOME' in path.read_text(encoding='utf-8')
            for path in downloads.glob('ubrus-report-*.txt')
        ),
    )
    chosen = session.runtime_dir / 'report.txt'
    session.selection.write_text(str(chosen), encoding='utf-8')
    window.click(315, 150)
    wait_for(
        'a report saved through the chooser',
        lambda: chosen.is_file() and 'Расширение GNOME' in chosen.read_text(encoding='utf-8'),
    )
    window.key('Escape')
    session.log('copied selected diagnostics and saved reports directly and through the portal')


def import_file(
    session: DesktopSession, window: SettingsWindow, path: Path, expected_count: int
) -> None:
    session.selection.write_text(str(path), encoding='utf-8')
    window.key('ctrl+o')
    wait_for(
        f'{path.name} in the library',
        lambda: session.profile_path.is_file()
        and len(session.profile()['items']) == expected_count,
    )
    window.focus()


def applied_format(session: DesktopSession) -> str | None:
    profile = session.profile()
    selected = profile['screens'].get('*', {}).get('id')
    return profile['items'].get(selected, {}).get('ext')


def apply_static(session: DesktopSession, window: SettingsWindow) -> None:
    def confirm_if_prompted() -> bool:
        if applied_format(session) == 'png':
            return True
        for title, kind, _alpha, _shown, rect, *_ in session.window_layout()['windows']:
            if kind in (3, 4) and title and 'set background' in title.lower():
                x, y, width, height = rect
                xdotool('mousemove', str(x + width * 3 // 4), str(y + height - 23), 'click', '1')
                return True
        return False

    window.key('ctrl+Return')
    wait_for('the wallpaper portal to apply or request permission', confirm_if_prompted)
    wait_for('the static wallpaper assignment', lambda: applied_format(session) == 'png')
    session.log('static image applied through the wallpaper portal')


def check_first_run(session: DesktopSession, app: Popen) -> Popen:
    app.terminate()
    app.wait(timeout=5)
    wait_for('the previous controller to exit', lambda: not session.owns_name(APP_BUS))
    session.profile_path.unlink()
    app = session.spawn(session.binary)
    wait_for(
        'the new controller to connect',
        lambda: session.owns_name(APP_BUS) and session.status().get('connected'),
    )

    window = SettingsWindow()
    import_file(session, window, MEDIA_DIR / 'landscape.png', 1)
    profile = session.profile()
    assert (profile['theme'], profile['language']) == ('system', 'system')
    app, window = check_preferences(session, app, window)
    apply_static(session, window)

    login_entry = Path(os.environ['XDG_CONFIG_HOME']) / 'autostart/ubrus-login.desktop'
    assert not login_entry.exists()
    import_file(session, window, MEDIA_DIR / 'loop.gif', 2)
    window.key('ctrl+Return')
    wait_for('the animated wallpaper assignment', lambda: applied_format(session) == 'gif')
    assert session.profile()['autostart'] is True
    assert '--background' in login_entry.read_text(encoding='utf-8')

    def gif_is_playing() -> bool:
        streams = session.snapshot()['engine'].get('streams', [])
        return any('gif' in stream.get('decoder', '').lower() for stream in streams)

    wait_for('GIF playback', gif_is_playing)
    # Select the second library row to dismiss apply feedback and reveal the full-height sidebar.
    window.click(70, 205)
    window.click(70, 565)
    wait_for('all wallpaper assignments to be cleared', lambda: not session.profile()['screens'])
    wait_for(
        'all desktop layers to disappear', lambda: json.loads(session.probe('Wallpapers')[0]) == []
    )
    wait_for(
        'the wallpaper renderer to stop',
        lambda: not (session.status().get('engine') or {}).get('windows'),
    )
    assert len(session.profile()['items']) == 2
    window.key('ctrl+Return')
    wait_for('the GIF assignment to be restored', lambda: applied_format(session) == 'gif')
    wait_for('the removed GIF to play again', gif_is_playing)

    # Open the confirmation for the selected GIF, cancel once, then confirm deletion.
    window.click(70, 205)
    window.click(510, 565)
    window.click(350, 565)
    assert applied_format(session) == 'gif'
    deleted_id = session.profile()['screens']['*']['id']
    window.click(510, 565)
    window.click(445, 565)
    wait_for(
        'the selected GIF to leave the library', lambda: deleted_id not in session.profile()['items']
    )
    assert not session.profile()['screens']
    managed_gif = session.library / 'media' / f'{deleted_id}.gif'
    wait_for('the managed GIF to be unlinked', lambda: not managed_gif.exists())
    assert not (session.library / 'thumbs' / f'{deleted_id}.png').exists()
    import_file(session, window, MEDIA_DIR / 'loop.gif', 2)
    window.key('ctrl+Return')
    wait_for('the reimported GIF assignment', lambda: applied_format(session) == 'gif')
    window.key('alt+F4')
    wait_for(
        'settings to close',
        lambda: not subprocess.run(
            ['xdotool', 'search', *SETTINGS_WINDOW], capture_output=True
        ).stdout,
    )
    assert app.poll() is None
    session.log(
        'imported and applied GIF through the settings window, closed settings, renderer still active'
    )
    return app
