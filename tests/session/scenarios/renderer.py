"""Exercise the installed renderer protocol using Python as its only controller."""

import json
import subprocess
import time
import os
import queue
import threading

from desktop_session import MEDIA_DIR, wait_for
from PIL import Image, ImageDraw, ImageGrab


def check_pixels(session, serial, send, await_layout):
    path = session.runtime_dir / 'quadrants.gif'
    first = Image.new('RGB', (80, 40), (200, 40, 60))
    draw = ImageDraw.Draw(first)
    draw.rectangle((40, 0, 79, 19), fill=(0, 180, 0))
    draw.rectangle((0, 20, 39, 39), fill=(200, 180, 0))
    draw.rectangle((40, 20, 79, 39), fill=(40, 60, 180))
    second = first.copy()
    second.putpixel((0, 0), (255, 255, 255))
    first.save(path, save_all=True, append_images=[second], duration=100, loop=0)
    screens = []
    for i, (width, height, edge, gain) in enumerate([(320, 180, 0, 50), (180, 320, 100, 120)]):
        screens.append(
            dict(
                index=i,
                path=str(path),
                width=width,
                height=height,
                active=True,
                fit=False,
                frame=dict(x=edge, y=edge, zoom=200, brightness=gain, speed=100),
            )
        )
    send(json.dumps(dict(serial=serial, screens=screens)))
    while await_layout(serial, 2)['displayed'].get('0') != str(path):
        pass
    for i, x in enumerate([0, 400]):
        wid = subprocess.check_output(
            ['xdotool', 'search', '--onlyvisible', '--name', f'^Ubrus surface {i}$'], text=True
        ).splitlines()[-1]
        subprocess.run(['xdotool', 'windowmove', '--sync', wid, str(x), '40'], check=True)

    def painted():
        img = ImageGrab.grab(xdisplay=os.environ['DISPLAY'])
        pixels = [img.getpixel(pt)[:3] for pt in [(160, 130), (490, 200)]]
        targets = [(100, 20, 30), (48, 72, 216)]
        return all(
            max(abs(c - d) for c, d in zip(color, target)) <= 3
            for color, target in zip(pixels, targets)
        )

    wait_for('framed and brightened pixels on landscape and portrait outputs', painted, timeout=5)
    session.log('verified actual crop and brightness pixels on landscape and portrait outputs')


def verify_renderer(session):
    with subprocess.Popen(
        [session.binary, '--renderer'],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        text=True,
        bufsize=1,
    ) as child:
        events = queue.Queue()

        def read():
            for line in child.stdout:
                if line.startswith('UBRUS/1 '):
                    events.put(json.loads(line[8:]))
            events.put(None)

        threading.Thread(target=read, daemon=True).start()

        def send(value):
            child.stdin.write(value + '\n')
            child.stdin.flush()

        def await_layout(serial, count):
            deadline = time.monotonic() + 12
            while time.monotonic() < deadline:
                status = events.get(timeout=max(0.1, deadline - time.monotonic()))
                if status is None or status.get('error'):
                    raise RuntimeError(f'Renderer failed: {status}')
                if status['serial'] == serial and len(status['displayed']) == count:
                    return status
            raise TimeoutError(f'Renderer layout {serial}')

        try:
            serial = 0
            for ext in ['gif', 'webp', 'mp4', 'webm']:
                screens = [
                    dict(
                        index=i,
                        path=str(MEDIA_DIR / f'loop.{ext}'),
                        width=w,
                        height=h,
                        fit=False,
                        active=True,
                        frame=dict(speed=75, x=25, y=75, zoom=150, brightness=65),
                    )
                    for i, (w, h) in enumerate([(320, 180), (180, 320)])
                ]
                for rates, expected in [((75, 75), 1), ((75, 200), 2)]:
                    serial += 1
                    for screen, rate in zip(screens, rates):
                        screen['frame']['speed'] = rate
                    send(json.dumps(dict(serial=serial, screens=screens)))
                    status = await_layout(serial, 2)
                    while status.get('speeds') != {'0': rates[0], '1': rates[1]}:
                        status = await_layout(serial, 2)
                    assert len(status['streams']) == expected
                time.sleep(0.45)
                for screen in screens:
                    screen['active'] = False
                serial += 1
                send(json.dumps(dict(serial=serial, screens=screens)))
                await_layout(serial, 2)
                for _ in range(20):
                    send('status')
                    status = await_layout(serial, 2)
                    if all(s['paused'] for s in status['streams']):
                        break
                    time.sleep(0.1)
                assert all(s['paused'] for s in status['streams'])
            check_pixels(session, serial + 1, send, await_layout)
            session.log('rendered GIF, WebP, MP4 and WebM at two rates and paused both outputs')
        finally:
            child.stdin.close()
            try:
                child.wait(timeout=4)
            except subprocess.TimeoutExpired:
                child.terminate()
