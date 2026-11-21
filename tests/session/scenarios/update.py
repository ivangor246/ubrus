"""Reinstall the same DEB into an isolated prefix and activate it through Settings."""

import os
import json
import hashlib
import shutil
import subprocess
from pathlib import Path
from desktop_session import MEDIA_DIR, PROJECT_ROOT, wait_for
from .x11_settings import SettingsWindow


def check_update(session, start_shell):
    package = max(
        (PROJECT_ROOT / 'dist').glob('ubrus_*_amd64.deb'), key=lambda p: p.stat().st_mtime
    )
    old = session.runtime_dir / 'installed'
    new = session.runtime_dir / 'replacement'
    for dest in [old, new]:
        subprocess.run(['dpkg-deb', '-x', package, dest], check=True)
    exe = old / 'usr/bin/ubrus'
    installed_metadata = old / 'usr/share/gnome-shell/extensions/wallpaper@ubrus.org/metadata.json'
    # The actual system package is root-owned even when the test prefix is user-owned.
    for path in installed_metadata.parent.rglob('*'):
        assert path.stat().st_mode & 0o004, f'Shell cannot read packaged asset {path}'
        if path.is_dir():
            assert path.stat().st_mode & 0o001, f'Shell cannot traverse packaged directory {path}'
    source = MEDIA_DIR / 'landscape.png'
    digest = hashlib.sha256(source.read_bytes()).hexdigest()
    for folder in ('media', 'thumbs'):
        dest = session.library / folder
        dest.mkdir(parents=True)
        shutil.copyfile(source, dest / f'{digest}.png')
    stored = dict(
        storage_version=2,
        items={digest: dict(name='Landscape', ext='png')},
        screens={'*': dict(id=digest, fit=True)},
        autostart=False,
        paused=False,
        power=False,
        theme='dark',
        language='english',
    )
    session.profile_path.write_text(json.dumps(stored), encoding='utf-8')
    profile = session.profile_path.read_bytes()
    start_shell(session, installed_metadata.parent)
    app = session.spawn(str(exe))
    window = SettingsWindow()
    wait_for('saved theme before update', window.dark)
    window.key('ctrl+comma')
    wait_for('controller', lambda: session.owns_name('org.ubrus.Ubrus'))
    os.replace(new / 'usr/bin/ubrus', exe)
    os.replace(
        new / 'usr/share/gnome-shell/extensions/wallpaper@ubrus.org/metadata.json',
        installed_metadata,
    )
    proc = Path(f'/proc/{app.pid}/exe')
    assert not proc.samefile(exe)

    def restart():
        if proc.samefile(exe):
            return True
        window.click(103, 102)
        return False

    wait_for('the running process to exec the installed image', restart, timeout=15)
    restored = SettingsWindow()
    wait_for('restored dark theme', restored.dark)
    assert session.profile_path.read_bytes() == profile
    session.log('same-version DEB reinstalled, process image replaced and settings reopened')
