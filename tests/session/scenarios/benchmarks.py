"""Repeatable measurements of the settings library and full-size animated images."""

import hashlib
import json
import os
import time

from PIL import Image, ImageChops, ImageDraw
from desktop_session import APP_BUS, DesktopSession, wait_for
from scenarios.playback import process_usage


def save_profile(session, items, screens):
    profile = dict(items=items, screens=screens, autostart=True, power=False, paused=False, storage_version=2)
    session.profile_path.write_text(json.dumps(profile))


def library(session: DesktopSession) -> None:
    media = session.library / 'media'
    thumbs = session.library / 'thumbs'
    media.mkdir(parents=True, exist_ok=True)
    thumbs.mkdir(exist_ok=True)
    base = Image.effect_noise((384, 256), 30).convert('RGB')
    items = {}
    for i in range(500):
        image = ImageChops.offset(base, i % 384, i // 384)
        ImageDraw.Draw(image).text((16, 16), str(i), fill='yellow')
        source = media / 'incoming.png'
        image.save(source, compress_level=1)
        digest = hashlib.sha256(source.read_bytes()).hexdigest()
        original = media / f'{digest}.png'
        source.replace(original)
        os.link(original, thumbs / f'{digest}.png')
        items[digest] = {'name': f'Photo {i:03d}.png', 'ext': 'png'}
    for n in (10, 500, 10, 500):
        save_profile(session, dict(list(items.items())[:n]), {})
        start = time.monotonic()
        app = session.spawn(session.binary)
        wait_for('mapped settings window', lambda: any(row[0] == 'Ubrus' and row[3] for row in session.window_layout()['windows']))
        elapsed = time.monotonic() - start
        time.sleep(.6)
        _, _, rss = process_usage(app.pid)
        session.log(f'{n} PNGs: mapped in {elapsed:.2f}s, RSS {rss:.1f} MiB')
        app.terminate()
        app.wait(timeout=8)
        wait_for('controller to exit', lambda: not session.owns_name(APP_BUS))


def animation(session: DesktopSession, ext: str, start_controller) -> None:
    media = session.library / 'media'
    thumbs = session.library / 'thumbs'
    media.mkdir(parents=True, exist_ok=True)
    thumbs.mkdir(exist_ok=True)
    frames = []
    for i in range(16):
        frame = Image.new('RGB', (1920, 1080), '#183b52')
        ImageDraw.Draw(frame).rectangle((i * 65, 90, i * 65 + 640, 900), fill='#f2b36e')
        frames.append(frame)
    source = media / f'incoming.{ext}'
    frames[0].save(source, save_all=True, append_images=frames[1:], duration=40, loop=0, lossless=True)
    digest = hashlib.sha256(source.read_bytes()).hexdigest()
    source.replace(media / f'{digest}.{ext}')
    frames[0].resize((384, 216)).save(thumbs / f'{digest}.png')
    save_profile(session, {digest: dict(name=ext, ext=ext, motion=True)}, {'*': dict(id=digest, fit=True)})
    launch = time.monotonic()
    start_controller(session)
    def first_frame():
        engine = session.snapshot()['engine']
        return engine if engine['displayed'] and engine['streams'] and engine['streams'][0]['rendered'] > 0 else None

    first = wait_for('a published animation frame', first_frame)
    session.log(f'1080p {ext}: first published frame observed {time.monotonic() - launch:.2f}s after controller launch')
    start, cpu, _ = process_usage(first['pid'])
    time.sleep(int(os.getenv('UBRUS_BENCH_ACTIVE_SECONDS', '10')))
    last = session.snapshot()['engine']
    end, final_cpu, rss = process_usage(last['pid'])
    rendered = last['streams'][0]['rendered'] - first['streams'][0]['rendered']
    assert rendered > 0 and last['streams'][0]['dropped'] is None
    session.log(f'1080p {ext}: {rendered} published frames in {end - start:.1f}s, CPU {100*(final_cpu-cpu)/(end-start):.1f}% of one core, RSS {rss:.1f} MiB')
