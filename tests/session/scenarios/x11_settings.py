"""Drive the Iced settings window and check its visible state under X11."""

import json
import os
import re
import shutil
import subprocess
from collections import Counter
from itertools import groupby
from pathlib import Path
from subprocess import Popen

import gi
from desktop_session import APP_BUS, MEDIA_DIR, PROJECT_ROOT, DesktopSession, wait_for
from PIL import Image, ImageGrab

gi.require_version('Gtk', '4.0')
gi.require_version('Gdk', '4.0')
from gi.repository import Gdk, GLib, Gtk  # noqa: E402

SETTINGS_WINDOW = ('--onlyvisible', '--name', '^Ubrus$')
MESSAGES = json.loads((PROJECT_ROOT / 'extension/locale.json').read_text(encoding='utf-8'))
PICKER_ROW = 125


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
        wait_for('Iced to draw the Ubrus heading', self.heading_is_drawn)

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

    def click(self, x: int, y: int) -> None:
        xdotool('mousemove', '--window', self.id, str(x), str(y), 'click', '1')

    def capture(self, box=(0, 0, 550, 380)) -> Image.Image:
        self.geometry()
        x1, y1, x2, y2 = box
        bounds = (self.left + x1, self.top + y1, self.left + x2, self.top + y2)
        return ImageGrab.grab(bbox=bounds, xdisplay=os.environ['DISPLAY'])

    def heading_is_drawn(self) -> bool:
        # The Ubrus label in the navigation bar is visible in all themes and languages.
        label = self.capture().crop((20, 20, 105, 55)).convert('L')
        darkest, lightest = label.getextrema()
        return lightest - darkest > 70

    def dark(self) -> bool:
        return max(self.capture().getpixel((5, 100))[:3]) < 110

    def open_settings(self) -> None:
        width, _ = self.geometry()
        self.click(width - 45, 31)

        def pickers_visible():
            image = self.capture()
            y = PICKER_ROW + 11
            background = image.getpixel((5, y))
            return image.getpixel((85, y)) == image.getpixel((208, y)) != background

        wait_for('the theme and language pickers', pickers_visible)


def photo_tile(window):
    # Find the fixture inside the gallery, regardless of the height of the search and header rows.
    image = window.capture()
    for y in range(125, image.height - 8, 4):
        if all(image.getpixel((x, y))[:3] == (37, 91, 156) for x in (56, 72, 88)):
            return (72, y)
    return None


def check_responsive(session, window, variant):
    for width, height in [(1440, 900), (620, 430), (900, 600)]:
        xdotool('windowsize', '--sync', window.id, str(width), str(height))

        # A wide editor shares the row with the gallery. A narrow editor has its own page.
        def drawn():
            if window.geometry() != (width, height):
                return None
            screen = window.capture((0, 0, width, height))
            blue = (37, 91, 156)
            green = (32, 132, 61)
            x = width // 2 if width == 620 else width - 175
            colors = [screen.getpixel((x, y))[:3] for y in range(110, height - 90)]
            actual_height = colors.count(blue) + colors.count(green)
            if blue not in colors or green not in colors or abs(actual_height - height * 0.32) > 8:
                return None
            if screen.getpixel((x, height - 12))[:3] in (blue, green):
                raise AssertionError('The preview overlaps the editor footer')
            return screen

        # Reopening the gallery at a narrow size can require a deliberate selection.
        if width == 620 and not drawn():
            point = wait_for('the photo at the narrow window size', lambda: photo_tile(window))
            window.click(*point)
        screenshot = wait_for(f'{width}×{height} editor ({variant})', drawn, timeout=6)
        output = Path(os.getenv('UBRUS_TEST_LOG_DIR', 'tests/artifacts/session-tests'))
        screenshot.save(output / f'editor-{variant}-{width}.png')
    session.log(f'checked editor at three window sizes ({variant})')


def select_option(session, window, setting, value, row):
    # Each menu places its first item underneath the field.
    x = 72 if setting == 'theme' else 208
    choices = ['system', 'light', 'dark'] if setting == 'theme' else [
        'system', 'english', 'russian', 'cs', 'de', 'id', 'it', 'nl',
        'pt-PT', 'tr', 'ja', 'ko', 'zh-Hans', 'zh-Hant',
    ]

    def menu_open():
        image = window.capture()
        y = row + 40
        return image.getpixel((x + 7, y)) != image.getpixel((5, y))

    window.click(x, row)
    wait_for(f'the {setting} menu to open', menu_open)
    window.click(x, row + 31 + 31 * choices.index(value))
    wait_for(f'{setting} = {value}', lambda: session.profile()[setting] == value)
    wait_for(f'the {setting} menu to close', lambda: not menu_open())


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


def check_panel_contrast(session, save_examples=False):
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
                if save_examples and mode.startswith('Yaru'):
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


def status_text_contrast(window):
    # Sample the status label independently of the logo, app title and button borders.
    pixels = Counter(window.capture().crop((156, 22, 275, 43)).convert('RGB').getdata())
    background = pixels.most_common(1)[0][0]

    def luminance(pixel):
        srgb = [value / 255 for value in pixel]
        linear = [v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4 for v in srgb]
        return sum(v * weight for v, weight in zip(linear, (0.2126, 0.7152, 0.0722)))

    levels = [luminance(pixel) + 0.05 for pixel, count in pixels.items() if count >= 3]
    surface = luminance(background) + 0.05
    return max(max(surface / level, level / surface) for level in levels)


def check_themes(session: DesktopSession, window: SettingsWindow, row: int) -> None:
    for scheme, dark in [('prefer-dark', True), ('default', False)]:
        subprocess.run(
            ['gsettings', 'set', 'org.gnome.desktop.interface', 'color-scheme', scheme], check=True
        )
        wait_for(
            f"window to follow GNOME's {scheme} scheme", lambda dark=dark: window.dark() == dark
        )
        assert status_text_contrast(window) >= 4.5
    for theme, dark in [('dark', True), ('light', False), ('system', False)]:
        select_option(session, window, 'theme', theme, row)
        wait_for(f'the {theme} theme to be visible', lambda dark=dark: window.dark() == dark)

    # Accessibility contrast is independent of the application's light/dark override.
    for theme, background in [('dark', (0, 0, 0)), ('light', (255, 255, 255))]:
        select_option(session, window, 'theme', theme, row)
        subprocess.run(
            ['gsettings', 'set', 'org.gnome.desktop.a11y.interface', 'high-contrast', 'true'],
            check=True,
        )
        wait_for(
            'high contrast in ' + theme,
            lambda: window.capture().getpixel((5, 100))[:3] == background,
        )
        assert status_text_contrast(window) >= 7
        subprocess.run(
            ['gsettings', 'set', 'org.gnome.desktop.a11y.interface', 'high-contrast', 'false'],
            check=True,
        )
        wait_for(
            'restore normal contrast', lambda: window.capture().getpixel((5, 100))[:3] != background
        )
    select_option(session, window, 'theme', 'system', row)


def check_ui_language(window: SettingsWindow, locale: str) -> None:
    # Read the translated text displayed in Diagnostics, including after a window move or theme change.
    expected = f'{MESSAGES[locale]["ReportSession"]} x11'
    width, height = window.geometry()
    Gtk.init()
    Gdk.Display.get_default().get_clipboard().set_content(None)
    window.key('ctrl+d')

    def copy_report():
        window.click(width - 60, height // 2)
        window.key('ctrl+a')
        window.key('ctrl+c')
        return expected in clipboard_text().splitlines()

    wait_for(f'the displayed diagnostics line {expected!r}', copy_report)
    window.open_settings()


def check_preferences(
    session: DesktopSession, app: Popen, window: SettingsWindow
) -> tuple[Popen, SettingsWindow]:
    window.open_settings()
    theme_row = language_row = PICKER_ROW
    check_themes(session, window, theme_row)
    for choice, locale in [
        ('russian', 'ru'), ('pt-PT', 'pt-PT'), ('ja', 'ja'), ('ko', 'ko'),
        ('zh-Hans', 'zh-Hans'), ('zh-Hant', 'zh-Hant'), ('english', 'en'), ('system', 'en'),
    ]:
        select_option(session, window, 'language', choice, language_row)
        check_ui_language(window, locale)

    select_option(session, window, 'language', 'english', language_row)
    select_option(session, window, 'theme', 'dark', theme_row)
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
    check_ui_language(window, 'en')
    select_option(session, window, 'theme', 'system', theme_row)
    wait_for('the system light theme', lambda: not window.dark())
    select_option(session, window, 'language', 'system', language_row)
    check_ui_language(window, 'ru')

    if os.getenv('UBRUS_TEST_COMPONENT') == 'controller':
        select_option(session, window, 'theme', 'dark', theme_row)
        wait_for('dark settings to finish drawing', window.dark)
        window.key('Escape')
        point = wait_for('gallery after leaving Settings', lambda: photo_tile(window))
        window.click(*point)
        check_responsive(session, window, 'ru-dark')
        window.open_settings()
        select_option(session, window, 'theme', 'system', theme_row)
        wait_for('the restored system light theme', lambda: not window.dark())
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

    def select_word():
        xdotool(
            'mousemove',
            '--window',
            window.id,
            '42',
            '175',
            'click',
            '--repeat',
            '2',
            '--delay',
            '100',
            '1',
        )
        window.key('ctrl+c')
        return clipboard_text() == 'Сеанс'

    wait_for('the mouse-selected word', select_word)
    window.key('ctrl+Home')
    window.key('shift+End')
    window.key('ctrl+c')
    wait_for('one selected diagnostics line', lambda: clipboard_text() == 'Сеанс x11')
    window.key('ctrl+a')
    window.key('ctrl+c')
    wait_for('the full diagnostics selection', lambda: 'Расширение GNOME' in clipboard_text())

    # The four Russian action buttons fit on a single row at the test window's width.
    width, _ = window.geometry()
    ink = (230, 238, 231) if window.dark() else (32, 51, 46)

    def focused_action(action):
        toolbar = ['Refresh', 'Copy all', 'Export', 'Save as']
        top = 112 if action in toolbar else 70

        def focused():
            bar = window.capture((0, top, width, top + 18)).convert('RGB')
            if action in toolbar:
                # Measure the button interiors below the top border, clear of the text.
                row = bar.crop((0, 10, width, 11)).getdata()
                background = bar.getpixel((0, 10))
                centers = []
                for filled, group in groupby(enumerate(row), lambda sample: sample[1] != background):
                    if filled:
                        xs = [x for x, _ in group]
                        if len(xs) > 30:
                            centers.append(xs[len(xs) // 2])
                if len(centers) != len(toolbar):
                    return False
                center = centers[toolbar.index(action)]
            else:
                center = width - 60
            return any(
                all(bar.getpixel((x, y)) == ink for x in (center - 15, center, center + 15))
                for y in range(bar.height)
            )

        wait_for(f'visible focus on {action}', focused, timeout=4)

    def tab_to(action, reverse=False):
        window.key('shift+Tab' if reverse else 'Tab')
        focused_action(action)

    # Anchor focus on Refresh, then exercise both directions without clicking another button.
    window.click(48, 132)
    focused_action('Refresh')
    tab_to('public report', reverse=True)
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
    tab_to('Refresh')
    window.key('Return')  # Refresh loads the new local log entry.
    tab_to('Copy all')

    def copy_private_report():
        window.key('Return')
        return private_marker in clipboard_text()

    wait_for('the full local report after keyboard refresh', copy_private_report)
    tab_to('Refresh', reverse=True)
    tab_to('public report', reverse=True)
    window.key('Return')
    wait_for('the sanitized copy after a private log was read', lambda: clipboard_text() == public)

    # Advance over Refresh and Copy all to Export, which needs no portal.
    for action in ('Refresh', 'Copy all', 'Export'):
        tab_to(action)
    session.selection.unlink(missing_ok=True)
    window.key('Return')
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
    focused_action('Export')
    tab_to('Save as')
    window.key('Return')
    wait_for(
        'a keyboard-activated report saved through the chooser',
        lambda: chosen.is_file() and private_marker in chosen.read_text(encoding='utf-8'),
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
        lambda: (
            session.profile_path.is_file() and len(session.profile()['items']) == expected_count
        ),
    )
    window.focus()


def applied_format(session: DesktopSession) -> str | None:
    profile = session.profile()
    selected = profile['screens'].get('*', {}).get('id')
    return profile['items'].get(selected, {}).get('ext')


def frame_static(window):
    # Sample the preview as well as the saved composition. Dragging and keyboard
    # positioning use the same normalized coordinates on each display.
    def editable():
        return window.capture((700, 162, 701, 163)).getpixel((0, 0))[:3] == (37, 91, 156)

    wait_for('the editable photo', editable)
    window.click(650, 410)
    window.click(650, 459)
    window.key('shift+Down')

    def dimmed():
        color = window.capture((700, 162, 701, 163)).getpixel((0, 0))
        return 65 < color[2] < 90

    wait_for('the dimmed preview', dimmed)


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
    check_panel_contrast(session)
    window.focus()
    photo = session.runtime_dir / ('Landscape panorama with a long gallery title ' * 3 + '.png')
    shutil.copyfile(MEDIA_DIR / 'landscape.png', photo)
    import_file(session, window, photo, 1)
    if os.getenv('UBRUS_TEST_COMPONENT') == 'controller':
        check_responsive(session, window, 'en-light')
    profile = session.profile()
    assert (profile['theme'], profile['language']) == ('system', 'system')
    app, window = check_preferences(session, app, window)
    if os.getenv('UBRUS_TEST_COMPONENT') == 'controller':
        frame_static(window)
    apply_static(session, window)
    if os.getenv('UBRUS_TEST_COMPONENT') == 'controller':
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
    check_panel_contrast(session, save_examples=True)
    panel('Приостановить анимацию')
    wait_for(
        'paused from panel',
        lambda: (
            session.profile()['paused'] and session.status()['outputs'][0]['reason'] == 'manual'
        ),
    )
    wait_for('resume action', lambda: 'Продолжить анимацию' in panel())
    check_panel_contrast(session)
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
    check_panel_contrast(session)
    panel('Вернуть фон Ubrus')
    wait_for('the GIF to resume', gif_is_playing)
    panel('Выключить Ubrus')
    wait_for('Ubrus disabled', lambda: session.profile()['disabled'] and not login_entry.exists())
    wait_for('enable action', lambda: 'Включить Ubrus' in panel())
    check_panel_contrast(session)
    panel('Включить Ubrus')
    wait_for('Ubrus enabled', lambda: not session.profile()['disabled'] and login_entry.exists())
    wait_for('the GIF to resume', gif_is_playing)
    panel('Убрать значок с панели')
    wait_for('icon removed', lambda: not panel())
    assert session.profile()['screens'] == saved

    window.focus()
    check_ui_language(window, 'ru')

    def checkbox_ready(y, checked):
        # The controller saves first. Wait for Iced to redraw the enabled label and check mark.
        line = window.capture((32, y - 8, 210, y + 9)).convert('RGB')
        enabled = (32, 51, 46) in line.crop((24, 0, line.width, line.height)).getdata()
        mark = (255, 255, 255) in line.crop((0, 0, 16, line.height)).getdata()
        return enabled and mark == checked

    wait_for('the hidden indicator checkbox', lambda: checkbox_ready(295, False))
    window.click(42, 295)
    wait_for(
        'indicator restored from settings',
        lambda: not session.profile()['hide_indicator'] and panel() and checkbox_ready(295, True),
    )
    window.click(42, 264)
    wait_for(
        'login switched off in settings',
        lambda: (
            session.profile()['autostart'] is False
            and not login_entry.exists()
            and checkbox_ready(264, False)
        ),
    )
    assert not session.profile()['disabled']
    window.click(42, 264)
    wait_for(
        'login switched on in settings', lambda: login_entry.exists() and checkbox_ready(264, True)
    )
    window.key('Escape')

    # Delete is an explicit, confirmed library action.
    window.key('Delete')
    window.key('Escape')
    assert applied_format(session) == 'gif'
    deleted_id = saved['*']['id']
    window.key('Delete')
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
    return app
