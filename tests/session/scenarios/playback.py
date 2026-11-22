"""Check frame delivery, monitor placement and pausing in the private GNOME session."""

import json
import os
import sys
import time
from pathlib import Path

import gi
from desktop_session import SESSION_DIR, DesktopSession, wait_for

gi.require_version('GdkPixbuf', '2.0')
from gi.repository import GdkPixbuf, GLib  # noqa: E402


def process_usage(pid: int) -> tuple[float, float, float]:
    process = Path('/proc') / str(pid)
    # After the parenthesized command, fields 14 and 15 are user and kernel CPU ticks.
    fields = (process / 'stat').read_text().rsplit(')', 1)[1].split()
    cpu_seconds = (int(fields[11]) + int(fields[12])) / os.sysconf('SC_CLK_TCK')
    resident_pages = int((process / 'statm').read_text().split()[1])
    rss_mib = resident_pages * os.sysconf('SC_PAGESIZE') / 1024**2
    return time.monotonic(), cpu_seconds, rss_mib


def battery_energy() -> dict[str, int]:
    """Read whole-system energy in µWh when running on discharging batteries."""
    batteries = {}
    try:
        for device in Path('/sys/class/power_supply').glob('*'):
            if (device / 'type').read_text().strip() != 'Battery':
                continue
            if (device / 'status').read_text().strip() != 'Discharging':
                return {}
            batteries[device.name] = int((device / 'energy_now').read_text())
    except (OSError, ValueError):
        return {}
    return batteries


def report_battery(session, before: dict[str, int], elapsed: float, state: str) -> None:
    after = battery_energy()
    if before and before.keys() == after.keys() and elapsed >= 60:
        energy = sum(before.values()) - sum(after.values())
        if energy > 0:
            session.log(f'{state}: whole-system battery discharge {energy * 0.0036 / elapsed:.2f} W')


def check_decoder(session: DesktopSession) -> int:
    initial = session.snapshot()
    pid = initial['engine']['pid']
    if not session.installed:
        assert os.path.samefile(f'/proc/{pid}/exe', session.binary), (
            'Unexpected renderer executable'
        )
    process_status = Path(f'/proc/{pid}/status').read_text()
    parent = next(
        line.split()[1] for line in process_status.splitlines() if line.startswith('PPid:')
    )
    session.log(f'renderer process {pid}, parent {parent}')

    (first_stream,) = initial['engine']['streams']
    battery = battery_energy()
    start, initial_cpu, _ = process_usage(pid)
    seconds = int(os.getenv("UBRUS_BENCH_ACTIVE_SECONDS", "3"))
    time.sleep(seconds)
    later = session.snapshot()
    end, final_cpu, rss = process_usage(pid)
    percent_cpu = 100 * (final_cpu - initial_cpu) / (end - start)
    report_battery(session, battery, end - start, 'playing')
    session.log(f'1080p/30 over {seconds}s, renderer CPU {percent_cpu:.1f}% of one core, RSS {rss:.1f} MiB')
    (stream,) = later['engine']['streams']
    decoded = stream['decoded'] - first_stream['decoded']
    assert decoded > 0, stream
    if os.getenv('UBRUS_BENCH_ACTIVE_SECONDS'):
        rendered = stream['rendered'] - first_stream['rendered']
        dropped = stream['dropped'] - first_stream['dropped']
        assert rendered > 0 and dropped >= 0, (first_stream, stream)
        session.log(f'active interval: {decoded} decoded, {rendered} rendered, {dropped} dropped')
    count = len(later['outputs'])
    session.log(
        f'{count} display(s), one {stream["decoder"]} decoder, '
        f'{stream["decoded"]} decoded frames, {stream["dropped"]} sink drops in current loop'
    )
    return count


def check_desktop_pixels(session: DesktopSession) -> None:
    session.probe('Desktop')
    time.sleep(1)

    def capture() -> bytes:
        session.probe('Capture')
        image = GdkPixbuf.Pixbuf.new_from_file(str(session.runtime_dir / 'ubrus-screen.png'))
        crop = image.new_subpixbuf(50, 90, 550, 340)
        thumbnail = crop.scale_simple(160, 100, GdkPixbuf.InterpType.BILINEAR)
        return bytes(thumbnail.get_pixels())

    first = capture()

    def pixels_changed() -> bool:
        return sum(abs(a - b) > 32 for a, b in zip(first, capture(), strict=True)) >= 3

    wait_for('changing desktop pixels', pixels_changed, timeout=8)
    session.log('captured changing desktop pixels')


def check_workspace_previews(session: DesktopSession) -> None:
    try:
        def scaled_cards() -> list[dict] | None:
            cards = json.loads(session.probe('Overview')[0])
            return cards if any(0 < c['size'][0] < c['source'][0] - 2 for c in cards) else None

        cards = wait_for('scaled workspace previews', scaled_cards)
        for card in cards:
            sw, sh = card['source']
            width, height = card['size']
            assert abs(width / sw - height / sh) <= 2 / min(sw, sh), (sw, sh, width, height)
            assert card['live'], 'The workspace must paint the live renderer surface'

        def sample_card() -> bytes:
            session.probe('Capture')
            image = GdkPixbuf.Pixbuf.new_from_file(str(session.runtime_dir / 'ubrus-screen.png'))
            for card in cards:
                x, y, w, h = card['viewport']
                left, top = int(x + w / 4), int(y + h / 4)
                width, height = int(w / 2), int(h / 2)
                if (
                    width > 0 and height > 0 and left >= 0 and top >= 0
                    and left + width <= image.get_width() and top + height <= image.get_height()
                ):
                    crop = image.new_subpixbuf(left, top, width, height)
                    return bytes(crop.scale_simple(128, 72, GdkPixbuf.InterpType.BILINEAR).get_pixels())
            raise AssertionError('No workspace card inside the screenshot')

        seen = set()

        def four_distinct_frames() -> bool:
            seen.add(sample_card())
            return len(seen) >= 4

        wait_for('new wallpaper pixels in the overview', four_distinct_frames, timeout=5)
    finally:
        session.probe('Desktop')
    session.log('workspace previews retained the monitor aspect ratio')


def rearrange_outputs(session: DesktopSession) -> None:
    interface = 'org.gnome.Mutter.DisplayConfig'
    path = '/org/gnome/Mutter/DisplayConfig'
    serial, monitors, logical, _ = session.call(interface, path, interface, 'GetCurrentState')
    current_modes = {
        spec[0]: next(mode for mode in modes if mode[-1].get('is-current'))
        for spec, modes, _ in monitors
    }
    layout = []
    y = 0
    for _, _, scale, transform, primary, specs, _ in sorted(
        logical, key=lambda output: output[0], reverse=True
    ):
        members = [(spec[0], current_modes[spec[0]][0], {}) for spec in specs]
        layout.append((0, y, scale, transform, primary, members))
        y += round(current_modes[specs[0][0]][2] / scale)
    # DisplayConfig method 1 applies this layout temporarily.
    args = GLib.Variant('(uua(iiduba(ssa{sv}))a{sv})', (serial, 1, layout, {}))
    session.call(interface, path, interface, 'ApplyMonitorsConfig', args)

    def all_repositioned() -> bool:
        observed = session.window_layout()
        frames = {
            int(title.rsplit(' ', 1)[1]): frame
            for title, _, _, _, frame, *_ in observed['windows']
            if title and title.startswith('Ubrus surface ')
        }
        monitors = observed['monitors']
        return (
            any(rect[1] > 0 for rect in monitors)
            and len(frames) == len(layout)
            and all(frames.get(index) == expected for index, expected in enumerate(monitors))
        )

    wait_for('renderer windows to follow the rearranged monitors', all_repositioned)
    session.log('renderer windows follow the rearranged monitors')


def check_coverage(session: DesktopSession, count: int) -> None:
    def reasons() -> list[str]:
        return [output['reason'] for output in session.status()['outputs']]

    def fixtures_mapped() -> bool:
        windows = session.window_layout()['windows']
        return (
            sum(
                1
                for title, _, _, shown, *_ in windows
                if title and title.startswith('Ubrus coverage fixture ') and shown
            )
            == count
        )

    def paused_stream() -> dict | None:
        first = session.snapshot()['engine']['streams'][0]
        if not first['paused']:
            return None
        # Allow frames already queued in the decoder to drain before comparing counters.
        time.sleep(0.2)
        if session.snapshot()['engine']['streams'][0] == first:
            return first
        return None

    command = (sys.executable, str(SESSION_DIR / 'fixtures/cover.py'))
    covers = [session.spawn(*command, '0')]
    wait_for('the first monitor to be covered', lambda: 'covered' in reasons())
    if count > 1:
        assert 'visible' in reasons(), 'Covering one monitor paused all monitors'
        covers.append(session.spawn(*command, *map(str, range(1, count))))
        wait_for('all cover windows to be mapped', fixtures_mapped)
        session.probe('Arrange')
    wait_for('all monitors to be covered', lambda: set(reasons()) == {'covered'})
    paused = wait_for('the paused decoder to drain', paused_stream)
    seconds = int(os.getenv('UBRUS_BENCH_SECONDS', '2'))
    pid = session.status()['engine']['pid']
    battery = battery_energy()
    start, initial_cpu, _ = process_usage(pid)
    time.sleep(seconds)
    end, final_cpu, rss = process_usage(pid)
    report_battery(session, battery, end - start, 'covered')
    session.log(f'covered renderer CPU {100 * (final_cpu - initial_cpu) / (end - start):.1f}% of one core, RSS {rss:.1f} MiB')
    current = session.snapshot()['engine']['streams'][0]
    assert current == paused, (paused, current)
    session.log(f'hidden frames stopped for {seconds}s')
    for process in covers:
        process.terminate()
    wait_for('all monitors to become visible', lambda: set(reasons()) == {'visible'})
