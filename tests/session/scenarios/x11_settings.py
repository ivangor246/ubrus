"""Drive the Iced settings window and check its visible state under X11."""

import hashlib
import json
import os
import re
import shutil
import subprocess
import time
from collections import Counter
from pathlib import Path
from subprocess import Popen

import gi
from desktop_session import APP_BUS, MEDIA_DIR, PROJECT_ROOT, DesktopSession, wait_for
from PIL import Image, ImageGrab

gi.require_version('Gtk', '4.0')
gi.require_version('Gdk', '4.0')
from gi.repository import Gdk, Gio, GLib, Gtk  # noqa: E402

SETTINGS_WINDOW = ('--onlyvisible', '--name', '^Ubrus$')
MESSAGES = json.loads((PROJECT_ROOT / 'extension/locale.json').read_text(encoding='utf-8'))


def xdotool(*args: str) -> str:
    return subprocess.check_output(['xdotool', *args], text=True, timeout=6).strip()


class SettingsWindow:
    def __init__(self) -> None:
        def find_window():
            result = subprocess.run(
                ['xdotool', 'search', *SETTINGS_WINDOW], capture_output=True, text=True
            )
            ids = result.stdout.splitlines()
            return ids[-1] if result.returncode == 0 and ids else None

        self.id = wait_for('mapped settings window', find_window)
        self.focus()
        if 'UBRUS_TEST_WIDGETS' in os.environ:
            self.wait('the library window', lambda state: 'settings' in state['widgets']['controls'])
        else:
            wait_for('Iced to paint the library', self.library_is_drawn)

    def geometry(self):
        info = subprocess.check_output(['xwininfo', '-id', self.id], text=True, timeout=6)
        coordinates = dict(re.findall(r'Absolute upper-left ([XY]):\s*(-?\d+)', info))
        if coordinates.keys() != {'X', 'Y'}:
            raise RuntimeError(f'Could not read settings window position from xwininfo:\n{info}')
        self.left = int(coordinates['X'])
        self.top = int(coordinates['Y'])
        size = re.search(r'Width:\s*(\d+)\s*Height:\s*(\d+)', info)
        return tuple(map(int, size.groups()))

    def focus(self) -> None:
        xdotool('windowactivate', '--sync', self.id)

    def key(self, shortcut: str) -> None:
        xdotool('key', '--clearmodifiers', shortcut)

    def capture(self, box=(0, 0, 550, 380)) -> Image.Image:
        """Capture window-relative pixels, or the full client area when box is None."""
        width, height = self.geometry()
        x1, y1, x2, y2 = box if box is not None else (0, 0, width, height)
        bounds = (self.left + x1, self.top + y1, self.left + x2, self.top + y2)
        return ImageGrab.grab(bbox=bounds, xdisplay=os.environ['DISPLAY'])

    def library_is_drawn(self) -> bool:
        # A mapped production window can still have a blank client area.
        image = self.capture(None).convert('L')
        w, h = image.size
        darkest, lightest = image.crop((w // 20, h // 8, w * 19 // 20, h * 7 // 8)).getextrema()
        return lightest - darkest > 70

    def dark(self) -> bool:
        return max(self.capture().getpixel((5, 100))[:3]) < 110

    def snapshot(self):
        directory = Path(os.environ['UBRUS_TEST_WIDGETS'])
        token = str(time.monotonic_ns())
        (directory / 'request').write_text(token)
        reply = directory / 'reply'
        next_request = 0.0

        def received():
            nonlocal next_request
            if reply.exists():
                state = json.loads(reply.read_text())
                if state['request'] == token:
                    return state
            # A mapped window can receive focus before Iced starts accepting input.
            if time.monotonic() >= next_request:
                # Clearing modifiers here would also release a held mouse button.
                xdotool('key', 'F12')
                next_request = time.monotonic() + 1
            return None

        return wait_for('a fresh Iced widget layout', received)

    def wait(self, description, predicate):
        last = {}

        def ready():
            nonlocal last
            last = self.snapshot()
            return last if not last['busy'] and predicate(last) else None

        try:
            return wait_for(description, ready)
        except TimeoutError as error:
            raise TimeoutError(f'{error}. Last Iced layout: {last}') from error

    def control(self, name):
        return self.wait(f'enabled {name} control', lambda s: name in s['widgets']['controls'])

    def bounds(self, state, name):
        width, height = self.geometry()
        sx, sy = width / state['size'][0], height / state['size'][1]
        x, y, w, h = state['widgets']['controls'][name]
        return round(x * sx), round(y * sy), round(w * sx), round(h * sy)

    def click_control(self, name, x_fraction=0.5, y_fraction=0.5):
        def pointer_at_target(state):
            control = state['widgets']['controls'].get(name)
            if control is None:
                return False
            x, y, w, h = control
            target_x, target_y = x + w * x_fraction, y + h * y_fraction
            pointer = state['pointer']
            if pointer['pressed']:
                return False
            if pointer['position'] is not None:
                px, py = pointer['position']
                if abs(px - target_x) <= 1 and abs(py - target_y) <= 1:
                    return True
            width, height = self.geometry()
            px = round(target_x * width / state['size'][0])
            py = round(target_y * height / state['size'][1])
            xdotool('mousemove', '--window', self.id, str(px), str(py))
            return False

        self.wait(f'Iced pointer at {name}', pointer_at_target)
        xdotool('mousedown', '1')
        try:
            self.wait(f'Iced to receive the press on {name}', lambda s: s['pointer']['pressed'])
        finally:
            xdotool('mouseup', '1')

    def open_settings(self):
        self.focus()
        self.key('ctrl+comma')
        self.wait('the theme and language pickers', lambda s: {'theme', 'language'} <= s['widgets']['controls'].keys())

    def focus_control(self, name):
        state = self.control(name)
        visited = set()
        while state['widgets']['focus'] != name:
            previous = state['widgets']['focus_index']
            if previous in visited:
                raise AssertionError(f'Tab skipped {name}: {visited}')
            visited.add(previous)
            self.key('Tab')
            state = self.wait('keyboard focus to advance', lambda s: s['widgets']['focus_index'] != previous)


def check_responsive(session, window):
    tile = 'tile-' + next(iter(session.profile()['items']))
    state = window.control('preview')
    physical = window.geometry()
    sx, sy = (pixels / logical for pixels, logical in zip(physical, state['size']))
    for width, height in [(1440, 900), (620, 430), (900, 600)]:
        xdotool('windowsize', '--sync', window.id, str(round(width * sx)), str(round(height * sy)))

        def editor(state):
            controls = state['widgets']['controls']
            if any(abs(a - b) > 1 for a, b in zip(state['size'], (width, height))):
                return False
            if not {'preview', 'apply'} <= controls.keys():
                return False
            if (tile in controls) != (width != 620) or ('back' in controls) != (width == 620):
                return False
            _, top, _, h = controls['preview']
            return abs(h - height * 0.32) <= 1 and top + h <= controls['apply'][1]

        state = window.wait(f'editor layout at {width}×{height}', editor)
        if width == 620:
            window.click_control('back')
            window.click_control(tile)
            state = window.wait('the reopened narrow editor', editor)
        x, y, w, h = window.bounds(state, 'preview')

        def painted():
            screen = window.capture(None)
            colors = [screen.getpixel((x + w // 2, y + h * n // 4))[:3] for n in (1, 3)]
            return screen if colors == [(37, 91, 156), (32, 132, 61)] else None

        screen = wait_for(f'painted photo at {width}×{height}', painted)
        output = Path(os.getenv('UBRUS_TEST_LOG_DIR', 'tests/artifacts/session-tests'))
        screen.save(output / f'editor-{width}.png')
    session.log('verified gallery, editor and rendered photo at three window sizes')


def select_option(session, window, setting, value, key='Right'):
    window.focus_control(setting)
    current = window.control(setting)[setting]
    visited = set()
    while current != value:
        if current in visited:
            raise AssertionError(f'{value} is missing from the {setting} picker: {visited}')
        visited.add(current)
        window.key(key)
        current = window.wait(f'{setting} to change from {current}', lambda s: s[setting] != current)[setting]
    assert session.profile()[setting] == value


def icon_colors_match(pixels, color, *, check_outline):
    background = pixels.most_common(1)[0][0]
    alpha = color[3] / 255
    ink = tuple(bg + (fg - bg) * alpha for bg, fg in zip(background, color))

    def distance(a, b):
        return max(abs(v - w) for v, w in zip(a, b))

    if distance(ink, background) < 32:
        return False
    solid = sum(count for pixel, count in pixels.items() if distance(pixel, ink) < 18)
    if not check_outline or solid < 3:
        return solid >= 3
    # Antialiased edges fall between the foreground and background. Catch dark
    # SVG strokes even if a small part of the shape was successfully recolored.
    limits = [(min(b, f) - 18, max(b, f) + 18) for b, f in zip(background, ink)]
    wrong = sum(
        count
        for pixel, count in pixels.items()
        if any(not lo <= v <= hi for v, (lo, hi) in zip(pixel, limits))
    )
    visible = sum(count for pixel, count in pixels.items() if distance(pixel, background) > 24)
    return wrong <= visible * 0.1


def check_panel_contrast(session):
    def panel(mode):
        return json.loads(
            session.call(
                'org.gnome.Shell',
                '/org/ubrus/Test',
                'org.ubrus.Test',
                'PanelContrast',
                GLib.Variant('(s)', (mode,)),
            )[0]
        )

    try:
        modes = ['light', 'dark', 'native']
        modes.extend(
            theme
            for theme in ('Yaru-blue-dark', 'Yaru-blue')
            if Path('/usr/share/gnome-shell/theme', theme, 'gnome-shell.css').is_file()
        )
        for mode in modes:
            panel(mode)
            samples = {}

            def painted():
                samples.clear()
                regions = panel('bounds')
                session.probe('Capture')
                with Image.open(session.runtime_dir / 'ubrus-screen.png') as image:
                    for name, region in regions.items():
                        bounds = region['bounds']
                        if not bounds or None in bounds:
                            samples[name] = 'missing'
                            continue
                        x, y, width, height = bounds
                        box = tuple(round(v) for v in (x, y, x + width, y + height))
                        colors = Counter(image.crop(box).convert('RGB').getdata())
                        # The bar actors also cover the corners of their rounded button.
                        if not icon_colors_match(
                            colors, region['color'], check_outline=name not in ('symbol', 'state')
                        ):
                            samples[name] = colors.most_common(3)
                return len(regions) == 12 and not samples

            try:
                wait_for(
                    f'{mode} bar and popup to paint the themed symbols and text', painted, timeout=8
                )
                if mode.startswith('Yaru'):
                    x, y, width, height = panel('menu-bounds')
                    output = Path(os.getenv('UBRUS_TEST_LOG_DIR', 'tests/artifacts/session-tests'))
                    with Image.open(session.runtime_dir / 'ubrus-screen.png') as image:
                        image.crop((round(x), round(y), round(x + width), round(y + height))).save(
                            output / f'panel-{mode}.png'
                        )
            except TimeoutError:
                session.log(f'Menu regions with missing or off-color glyphs: {samples}')
                shutil.copyfile(
                    session.runtime_dir / 'ubrus-screen.png',
                    Path(os.getenv('UBRUS_TEST_LOG_DIR', 'tests/artifacts/session-tests'))
                    / 'panel-contrast.png',
                )
                raise
    finally:
        panel('reset')
    session.log('verified rendered panel and menu icon colors against opposing Shell themes')


def check_themes(session: DesktopSession, window: SettingsWindow) -> None:
    for scheme, dark in [('prefer-dark', True), ('default', False)]:
        subprocess.run(
            ['gsettings', 'set', 'org.gnome.desktop.interface', 'color-scheme', scheme], check=True
        )
        wait_for(
            f"window to follow GNOME's {scheme} scheme", lambda dark=dark: window.dark() == dark
        )
    for theme, dark in [('dark', True), ('light', False), ('system', False)]:
        select_option(session, window, 'theme', theme)
        wait_for(f'the {theme} theme to be visible', lambda dark=dark: window.dark() == dark)

    # Accessibility contrast is independent of the application's light/dark override.
    for theme, background in [('dark', (0, 0, 0)), ('light', (255, 255, 255))]:
        select_option(session, window, 'theme', theme)
        subprocess.run(
            ['gsettings', 'set', 'org.gnome.desktop.a11y.interface', 'high-contrast', 'true'],
            check=True,
        )
        wait_for(
            'high contrast in ' + theme,
            lambda: window.capture().getpixel((5, 100))[:3] == background,
        )
        subprocess.run(
            ['gsettings', 'set', 'org.gnome.desktop.a11y.interface', 'high-contrast', 'false'],
            check=True,
        )
        wait_for(
            'restore normal contrast', lambda: window.capture().getpixel((5, 100))[:3] != background
        )
    select_option(session, window, 'theme', 'system')


def check_ui_language(window: SettingsWindow, locale: str) -> None:
    label = MESSAGES[locale]['Desktop']
    window.wait(f'translated settings label {label!r}', lambda s: label in s['widgets']['text'])


def check_preferences(
    session: DesktopSession, app: Popen, window: SettingsWindow
) -> tuple[Popen, SettingsWindow]:
    window.open_settings()
    check_themes(session, window)
    select_option(session, window, 'language', 'russian')
    check_ui_language(window, 'ru')

    select_option(session, window, 'language', 'english', key='Left')
    select_option(session, window, 'theme', 'dark')
    wait_for('dark theme before restarting', window.dark)
    window.key('alt+F4')
    wait_for(
        'settings window to close before restart',
        lambda: (
            not subprocess.run(['xdotool', 'search', *SETTINGS_WINDOW], capture_output=True).stdout
        ),
    )
    app.terminate()
    app.wait(timeout=8)
    wait_for('the controller to exit', lambda: not session.owns_name(APP_BUS))

    # A Russian login should preserve the explicit English override until System is selected.
    app = session.spawn(session.binary, env={**os.environ, 'LANGUAGE': 'ru'})
    window = SettingsWindow()
    wait_for('the restored dark theme', window.dark)
    window.open_settings()
    check_ui_language(window, 'en')
    select_option(session, window, 'theme', 'system')
    wait_for('the system light theme', lambda: not window.dark())
    select_option(session, window, 'language', 'system', key='Left')
    check_ui_language(window, 'ru')

    check_diagnostics(session, window)
    session.log(
        'followed live GNOME theme changes; switched languages and themes; reopened with saved overrides'
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


def check_diagnostics(session: DesktopSession, window: SettingsWindow) -> None:
    window.key('ctrl+d')
    Gtk.init()

    window.click_control('report')
    window.wait('focus in the report editor', lambda s: s['widgets']['focus'] == 'report')
    window.key('ctrl+Home')
    window.key('shift+End')
    window.wait('the first report line to be selected', lambda s: s['report_selection'] == 'Сеанс x11')
    window.key('ctrl+c')
    wait_for('one selected diagnostics line', lambda: clipboard_text() == 'Сеанс x11')
    window.key('ctrl+a')
    window.wait('the full report to be selected', lambda s: 'Расширение GNOME' in (s['report_selection'] or ''))
    window.key('ctrl+c')
    wait_for('full diagnostics selection', lambda: 'Расширение GNOME' in clipboard_text())

    def tab_to(action, reverse=False):
        window.key('shift+Tab' if reverse else 'Tab')
        window.wait(f'keyboard focus on {action}', lambda s: s['widgets']['focus'] == action)

    window.click_control('refresh')
    window.wait('keyboard focus on refresh', lambda s: s['widgets']['focus'] == 'refresh')
    tab_to('public', reverse=True)
    window.key('Return')

    def public_report():
        value = clipboard_text()
        return value if 'Экраны:' in value else None

    public = wait_for('the keyboard-activated public report', public_report)
    assert public.startswith('Ubrus ') and 'Сеанс: X11' in public
    assert str(session.library) not in public and 'Landscape panorama' not in public

    private_marker = f'Local-only path: {session.runtime_dir}/personal-picture.png'
    with (session.library / 'ubrus.log').open('a', encoding='utf-8') as log:
        log.write(private_marker + '\n')
    tab_to('refresh')
    window.key('Return')  # Refresh loads the new local log entry.
    tab_to('copy')

    def copy_private_report():
        window.key('Return')
        return private_marker in clipboard_text()

    wait_for('the full local report after keyboard refresh', copy_private_report)
    window.click_control('public')
    wait_for('the sanitized copy after a private log was read', lambda: clipboard_text() == public)
    session.selection.unlink(missing_ok=True)
    window.click_control('export')
    downloads = session.runtime_dir / 'downloads'
    wait_for(
        'a directly saved Russian report',
        lambda: any(
            private_marker in path.read_text(encoding='utf-8')
            for path in downloads.glob('ubrus-report-*.txt')
        ),
    )
    chosen = session.runtime_dir / 'report.txt'
    session.selection.write_text(str(chosen), encoding='utf-8')
    window.click_control('save-as')
    wait_for(
        'a report saved through the chooser',
        lambda: chosen.is_file() and private_marker in chosen.read_text(encoding='utf-8'),
    )
    window.key('Escape')
    session.log('copied selected diagnostics and saved reports directly and through the portal')


def import_file(
    session: DesktopSession, window: SettingsWindow, path: Path, expected_count: int
) -> None:
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    session.selection.write_text(str(path), encoding='utf-8')
    window.focus()
    window.control('import')
    window.key('ctrl+o')
    window.wait(
        f'{path.name} selected in the editor after import',
        lambda s: s['selected'] == digest and 'apply' in s['widgets']['controls'],
    )
    items = session.profile()['items']
    assert digest in items and len(items) == expected_count, items


def applied_format(session: DesktopSession) -> str | None:
    profile = session.profile()
    selected = profile['screens'].get('*', {}).get('id')
    return profile['items'].get(selected, {}).get('ext')


def frame_static(window):
    # Sample the preview as well as the saved composition. Dragging and keyboard
    # positioning use the same normalized coordinates on each display.
    window.click_control('zoom', 1 / 3, 0.85)
    window.click_control('brightness', 1 / 3, 0.85)
    window.key('shift+Down')
    x, y, w, h = window.bounds(window.control('preview'), 'preview')
    wait_for('the dimmed preview', lambda: 65 < window.capture(None).getpixel((x + w // 2, y + h // 4))[2] < 90)


def check_static_pixels(session):
    choice = session.profile()['screens']['*']['frame']
    assert (
        45 <= choice['brightness'] <= 55 and 185 <= choice['zoom'] <= 210 and choice['y'] == 52
    ), choice
    gain = choice['brightness'] / 100

    def matches(image):
        for y, rgb in [(image.height // 4, (37, 91, 156)), (3 * image.height // 4, (32, 132, 61))]:
            px = image.getpixel((10, y))[:3]
            if max(abs(v - round(ref * gain)) for v, ref in zip(px, rgb)) > 3:
                return False
        return True

    cache = Path(os.environ['XDG_CONFIG_HOME']) / 'background'
    with Image.open(cache) as picture:
        assert matches(picture), 'Wallpaper portal composition differs from the preview'

    def painted():
        session.probe('Capture')
        with Image.open(session.runtime_dir / 'ubrus-screen.png') as image:
            return matches(image)

    wait_for('Shell to paint the adjusted still', painted, timeout=6)
    session.log('compared brightness in the preview, portal PNG and actual desktop pixels')


def apply_static(session: DesktopSession, window: SettingsWindow) -> None:
    # Use the ID the private portal received from this application's file chooser.
    app_id = session.selection.with_suffix('.app-id').read_text(encoding='utf-8')
    interface = 'org.freedesktop.impl.portal.PermissionStore'
    session.bus.call_sync(
        interface, '/org/freedesktop/impl/portal/PermissionStore', interface, 'SetPermission',
        GLib.Variant('(sbssas)', ('wallpaper', True, 'wallpaper', app_id, ['yes'])),
        None, Gio.DBusCallFlags.NONE, 5000, None,
    )
    window.key('ctrl+Return')
    wait_for('the static wallpaper assignment', lambda: applied_format(session) == 'png')
    session.log('static image applied through the wallpaper portal')


def check_first_run(session: DesktopSession) -> None:
    app = session.spawn(session.binary)
    wait_for(
        'the new controller to connect',
        lambda: session.owns_name(APP_BUS) and session.status().get('connected'),
    )

    window = SettingsWindow()
    window.focus()
    photo = session.runtime_dir / ('Landscape panorama with a long gallery title ' * 3 + '.png')
    shutil.copyfile(MEDIA_DIR / 'landscape.png', photo)
    import_file(session, window, photo, 1)
    check_responsive(session, window)
    profile = session.profile()
    assert (profile['theme'], profile['language']) == ('system', 'system')
    app, window = check_preferences(session, app, window)
    frame_static(window)
    apply_static(session, window)
    check_static_pixels(session)

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

    def panel(label=''):
        return json.loads(
            session.call(
                'org.gnome.Shell',
                '/org/ubrus/Test',
                'org.ubrus.Test',
                'Panel',
                GLib.Variant('(s)', (label,)),
            )[0]
        )

    saved = session.profile()['screens']
    wait_for('translated panel actions', lambda: 'Убрать фон Ubrus' in panel())
    check_panel_contrast(session)
    panel('Приостановить анимацию')
    wait_for(
        'paused from panel',
        lambda: (
            session.profile()['paused'] and session.status()['outputs'][0]['reason'] == 'manual'
        ),
    )
    wait_for('resume action', lambda: 'Продолжить анимацию' in panel())
    panel('Продолжить анимацию')
    wait_for('resumed from panel', lambda: not session.profile()['paused'])
    panel('Убрать фон Ubrus')
    wait_for(
        'wallpaper hidden',
        lambda: (
            session.profile()['hidden']
            and not (session.status().get('engine') or {}).get('windows')
        ),
    )
    assert session.profile()['screens'] == saved
    wait_for('the restore menu item', lambda: 'Вернуть фон Ubrus' in panel())
    panel('Вернуть фон Ubrus')
    wait_for('the GIF to resume', gif_is_playing)
    panel('Выключить Ubrus')
    wait_for('Ubrus disabled', lambda: session.profile()['disabled'] and not login_entry.exists())
    wait_for('enable action', lambda: 'Включить Ubrus' in panel())
    panel('Включить Ubrus')
    wait_for('Ubrus enabled', lambda: not session.profile()['disabled'] and login_entry.exists())
    wait_for('the GIF to resume', gif_is_playing)
    panel('Убрать значок с панели')
    wait_for('icon removed', lambda: not panel())
    assert session.profile()['screens'] == saved

    window.focus()
    window.open_settings()
    check_ui_language(window, 'ru')

    window.click_control('indicator')
    window.wait('restored indicator', lambda s: s['indicator'])
    wait_for('the Shell indicator', panel)
    assert not session.profile()['hide_indicator']
    for enabled in (False, True):
        window.click_control('login')
        window.wait(f'login enabled = {enabled}', lambda s: s['login'] == enabled)
        assert session.profile()['autostart'] is enabled and login_entry.exists() == enabled
    assert not session.profile()['disabled']
    window.key('Escape')

    # Delete is an explicit, confirmed library action.
    window.key('Delete')
    window.control('confirm-delete')
    window.key('Escape')
    window.control('apply')
    assert applied_format(session) == 'gif'
    deleted_id = saved['*']['id']
    window.key('Delete')
    window.control('confirm-delete')
    window.key('Return')
    wait_for(
        'the selected GIF to leave the library',
        lambda: deleted_id not in session.profile()['items'],
    )
    assert not session.profile()['screens']
    managed_gif = session.library / 'media' / f'{deleted_id}.gif'
    wait_for('the managed GIF to be unlinked', lambda: not managed_gif.exists())
    assert not (session.library / 'thumbs' / f'{deleted_id}.png').exists()
    import_file(session, window, MEDIA_DIR / 'loop.gif', 2)
    window.key('ctrl+Return')
    wait_for('the reimported GIF assignment', lambda: applied_format(session) == 'gif')
    window.focus()
    window.key('alt+F4')
    wait_for(
        'settings to close',
        lambda: (
            not subprocess.run(['xdotool', 'search', *SETTINGS_WINDOW], capture_output=True).stdout
        ),
    )
    assert app.poll() is None
    session.log(
        'imported and applied GIF through the settings window, closed settings, renderer still active'
    )
