"""Check release versions and the contents of a built Snap before distribution."""

import argparse
import io
import json
import re
import subprocess
import tomllib
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

import yaml


def check_versions(tag: str | None) -> str:
    version = tomllib.loads(Path('Cargo.toml').read_text())['package']['version']
    lock = tomllib.loads(Path('Cargo.lock').read_text())
    snap = yaml.safe_load(Path('snap/snapcraft.yaml').read_text())
    rust_version = tomllib.loads(Path('rust-toolchain.toml').read_text())['toolchain']['channel']
    archive = Path(snap['parts']['rust-deps']['source']).name
    if archive != f'rust-{rust_version}-x86_64-unknown-linux-gnu.tar.xz':
        raise SystemExit(f'Snap toolchain {archive} does not match Rust {rust_version} for amd64')
    component = ET.parse('packaging/desktop/org.ubrus.Ubrus.metainfo.xml')
    releases = component.findall('releases/release')
    sources = {
        'Cargo.lock': next(p['version'] for p in lock['package'] if p['name'] == 'ubrus'),
        'snap/snapcraft.yaml': snap['version'],
        'latest AppStream release': releases[0].get('version') if releases else None,
    }
    for name, value in sources.items():
        if value != version:
            raise SystemExit(f'{name}: expected {version}, found {value}')
    if tag is not None and tag != f'v{version}':
        raise SystemExit(f'Expected tag v{version}, found {tag}')
    return version


def check_snap(path: Path, version: str) -> None:
    def member(name: str) -> bytes:
        return subprocess.check_output(['unsquashfs', '-processors', '1', '-cat', path, name])

    manifest = yaml.safe_load(member('meta/snap.yaml'))
    expected = {'name': 'ubrus', 'version': version, 'confinement': 'strict', 'base': 'core24'}
    for key, value in expected.items():
        if manifest.get(key) != value:
            raise SystemExit(f'{path}: expected {key}={value}, found {manifest.get(key)}')
    if manifest.get('architectures') != ['amd64']:
        raise SystemExit(f'{path}: expected the amd64 build')
    slot = manifest['slots']['wallpaper']
    if (slot['interface'], slot['bus'], slot['name']) != ('dbus', 'session', 'org.ubrus.Ubrus'):
        raise SystemExit(f'{path}: unexpected wallpaper D-Bus slot')
    if 'wallpaper' not in manifest['apps']['ubrus'].get('slots', []):
        raise SystemExit(f'{path}: wallpaper slot is missing from the app')
    metainfo = Path('packaging/desktop/org.ubrus.Ubrus.metainfo.xml')
    if member(f'usr/share/metainfo/{metainfo.name}') != metainfo.read_bytes():
        raise SystemExit(f'{path}: AppStream metadata differs from the release source')
    archive = member('share/ubrus/ubrus-extension.zip')
    with zipfile.ZipFile(io.BytesIO(archive)) as extension, zipfile.ZipFile('dist/ubrus-extension.zip') as expected_zip:
        if (set(extension.namelist()) != set(expected_zip.namelist())
                or any(extension.read(name) != expected_zip.read(name) for name in expected_zip.namelist())):
            raise SystemExit(f'{path}: bundled extension differs from the tagged sources')
        build_id = json.loads(extension.read('metadata.json'))['build-id']
    if (not re.fullmatch(r'[0-9a-f]{64}', build_id)
            or member('share/ubrus/extension-build-id').decode().strip() != build_id):
        raise SystemExit(f'{path}: extension build ID mismatch')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tag')
    parser.add_argument('--snap', type=Path)
    args = parser.parse_args()
    version = check_versions(args.tag)
    if args.snap:
        check_snap(args.snap, version)
    print(version)
