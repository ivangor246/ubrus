"""Run Ubrus against an isolated GNOME session with a private bus and XDG directories."""

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

from desktop_session import (
    APP_BUS,
    EXTENSION,
    MEDIA_DIR,
    PROBE_EXTENSION,
    PROJECT_ROOT,
    SESSION_DIR,
    DesktopSession,
    wait_for,
)
from gi.repository import GLib
from scenarios.controller_recovery import check_controller_loss
from scenarios.playback import (
    check_coverage,
    check_decoder,
    check_desktop_pixels,
    check_workspace_previews,
    rearrange_outputs,
)


def create_media(pipeline: str, destination: Path) -> None:
    subprocess.run(
        ['gst-launch-1.0', '-q', *pipeline.split(), '!', 'filesink', f'location={destination}'],
        check=True,
    )


def seed_library(session: DesktopSession) -> None:
    media = session.library / 'media'
    thumbnails = session.library / 'thumbs'
    media.mkdir(parents=True)
    thumbnails.mkdir()
    clip = media / 'test.webm'
    if os.getenv('UBRUS_TEST_COMPONENT') == 'controller':
        shutil.copyfile(MEDIA_DIR / 'loop.webm', clip)
    else:
        # A benchmark interval spans a single segment so a flushing loop cannot reset sink statistics.
        active_seconds = int(os.getenv('UBRUS_BENCH_ACTIVE_SECONDS', '0'))
        frames = (active_seconds + 10) * 30 if active_seconds else 90
        create_media(
            f'videotestsrc num-buffers={frames} pattern=smpte horizontal-speed=17 ! '
            'video/x-raw,width=1920,height=1080,framerate=30/1 ! vp8enc threads=1 deadline=1 ! webmmux',
            clip,
        )
    digest = hashlib.sha256(clip.read_bytes()).hexdigest()
    clip.rename(media / f'{digest}.webm')
    create_media('videotestsrc num-buffers=1 ! pngenc', thumbnails / f'{digest}.png')
    profile = {
        'items': {digest: {'name': 'Fixture', 'ext': 'webm'}},
        'screens': {'*': {'id': digest, 'fit': True}},
        'autostart': True,
        'power': False,
        'paused': False,
    }
    session.profile_path.write_text(json.dumps(profile), encoding='utf-8')


def start_shell(session: DesktopSession, extension_root: Path | None = None) -> None:
    extensions = session.data_dir / 'gnome-shell/extensions'
    extensions.mkdir(parents=True, exist_ok=True)
    if not session.installed:
        (extensions / EXTENSION).symlink_to(extension_root or PROJECT_ROOT / 'extension', target_is_directory=True)
    (extensions / PROBE_EXTENSION).symlink_to(
        SESSION_DIR / 'fixtures/probe', target_is_directory=True
    )

    if session.mode == 'wayland':
        os.environ.pop('DISPLAY', None)
        display = os.getenv('UBRUS_TEST_DISPLAY', 'ubrus-integration')
        os.environ['WAYLAND_DISPLAY'] = display
        session.spawn(
            'gnome-shell',
            '--headless',
            '--wayland',
            '--no-x11',
            f'--wayland-display={display}',
            '--virtual-monitor=800x600',
            '--virtual-monitor=640x480',
            env={**os.environ, 'UBRUS_TEST_SESSION': '1'},
        )
    else:
        session.spawn('gnome-shell', '--x11', env={**os.environ, 'UBRUS_TEST_SESSION': '1'})
    wait_for('GNOME Shell', lambda: session.owns_name('org.gnome.Shell'))
    wait_for('the wallpaper extension', lambda: session.extension('GetExtensionInfo')[0])
    session.extension('EnableExtension')
    session.extension('EnableExtension', PROBE_EXTENSION)
    session.show_desktop()


def start_controller(session: DesktopSession) -> subprocess.Popen:
    process = session.spawn(session.binary, '--background')

    def ready() -> bool:
        if process.poll() is not None:
            raise RuntimeError(f'Application exited with {process.returncode}')
        return session.owns_name(APP_BUS)

    wait_for('the controller to start', ready)
    expected = 2 if session.mode == 'wayland' else 1
    wait_for(
        'renderer windows',
        lambda: (session.status().get('engine') or {}).get('windows', 0) >= expected,
        timeout=35,
    )
    return process


def reopen_settings(session: DesktopSession, controller: subprocess.Popen) -> None:
    subprocess.run([session.binary], check=True, timeout=8)
    assert controller.poll() is None

    def single_window() -> bool:
        return sum(title == 'Ubrus' for title, *_ in session.window_layout()['windows']) == 1

    wait_for('a single Ubrus settings window', single_window)
    session.log('reopened one settings window from the background service')


def check_wayland_import(session: DesktopSession) -> None:
    files = [MEDIA_DIR / 'landscape.png', MEDIA_DIR / 'single.gif']
    selection = session.selection.with_suffix('.json')
    selection.write_text(json.dumps([str(path) for path in files]), encoding='utf-8')
    expected = {hashlib.sha256(path.read_bytes()).hexdigest() for path in files}
    session.probe('Import')
    wait_for(
        'both selected files to be imported',
        lambda: expected.issubset(session.profile()['items']),
    )
    assert (
        session.selection.with_suffix('.parent').read_text(encoding='utf-8').startswith('wayland:')
    )
    selection.unlink()
    session.log('imported two files through the file chooser parented to the Iced window')


def check_extension_restart(session: DesktopSession) -> None:
    session.extension('DisableExtension')
    wait_for('the extension to disconnect', lambda: not session.status()['connected'])
    wait_for(
        'the disabled extension state',
        lambda: session.extension('GetExtensionInfo')[0].get('state') == 2,
    )
    session.extension('EnableExtension')
    wait_for(
        'renderer windows after reconnecting',
        lambda: (session.status().get('engine') or {}).get('windows', 0) > 0,
        timeout=45,
    )
    session.log('extension restart passed')


def run(session: DesktopSession) -> None:
    if os.getenv('UBRUS_TEST_COMPONENT') == 'library':
        from scenarios.benchmarks import library
        start_shell(session)
        library(session)
        return
    if os.getenv('UBRUS_TEST_COMPONENT') in ('gif', 'webp'):
        from scenarios.benchmarks import animation
        start_shell(session)
        animation(session, os.environ['UBRUS_TEST_COMPONENT'], start_controller)
        return
    if os.getenv('UBRUS_TEST_COMPONENT') == 'update':
        from scenarios.update import check_update
        check_update(session, start_shell)
        return
    if os.getenv('UBRUS_TEST_COMPONENT') == 'renderer':
        from scenarios.renderer import verify_renderer
        verify_renderer(session)
        return
    if session.mode == 'x11' or not session.installed:
        session.spawn(sys.executable, str(SESSION_DIR / 'fixtures/filechooser.py'))
        wait_for(
            'the file chooser fixture',
            lambda: session.owns_name('org.freedesktop.impl.portal.desktop.ubrustest'),
        )

    if os.getenv('UBRUS_TEST_COMPONENT') == 'controller':
        from scenarios.x11_settings import check_first_run
        os.environ['UBRUS_RENDERER'] = str(SESSION_DIR / 'fixtures/renderer.py')
        seed_library(session)
        start_shell(session)
        app = start_controller(session)
        check_first_run(session, app)
        return

    seed_library(session)
    start_shell(session)
    app = start_controller(session)
    count = check_decoder(session)
    check_desktop_pixels(session)
    check_workspace_previews(session)
    if session.mode == 'wayland' and count == 2:
        rearrange_outputs(session)
    check_coverage(session, count)
    reopen_settings(session, app)
    if session.mode == 'x11':
        # Pillow and xdotool are only installed in the X11 test environment.
        from scenarios.x11_settings import check_first_run

        app = check_first_run(session, app)
    elif not session.installed:
        check_wayland_import(session)

    check_extension_restart(session)
    check_controller_loss(session, app)
    session.extension('DisableExtension')
    session.extension('DisableExtension', PROBE_EXTENSION)


def report_failure(session: DesktopSession) -> None:
    if not session.installed:
        try:
            session.probe('Capture')
            logs = Path(
                os.getenv('UBRUS_TEST_LOG_DIR', PROJECT_ROOT / 'tests/artifacts/session-tests')
            )
            shutil.copyfile(
                session.runtime_dir / 'ubrus-screen.png', logs / f'{session.mode}-failure.png'
            )
        except (GLib.Error, OSError) as error:
            print(f'Could not save a failure screenshot: {error}', file=sys.stderr)
    app_log = session.library / 'ubrus.log'
    if app_log.is_file():
        print(
            'Application log:',
            app_log.read_text(encoding='utf-8', errors='replace')[-1800:],
            flush=True,
        )
    for name, get_diagnostics in [('Status', session.status), ('Windows', session.window_layout)]:
        try:
            print('Failure diagnostics', name, get_diagnostics(), flush=True)
        except (GLib.Error, ValueError) as error:
            print(f'Could not read {name}: {error}', file=sys.stderr)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('wayland', 'x11'))
    session = DesktopSession(parser.parse_args().mode)
    try:
        run(session)
    except Exception:
        report_failure(session)
        raise
    finally:
        session.close()


if __name__ == '__main__':
    main()
