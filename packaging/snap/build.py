"""Build a strict core24 snap using the installed GNOME runtime and Ubuntu codecs."""

import argparse
import platform
import re
import shutil
import subprocess
import tempfile
from pathlib import Path

import yaml

CORE = Path('/snap/core24/current')
GNOME = Path('/snap/gnome-46-2404/current')
LIBDIR = Path('usr/lib/x86_64-linux-gnu')
RECIPE_FIELDS = (
    'name',
    'base',
    'version',
    'summary',
    'description',
    'grade',
    'confinement',
    'license',
    'slots',
)
CONTENT_PLUGS = (
    ('gnome-46-2404', 'gnome-platform', 'gnome-46-2404'),
    ('gpu-2404', 'gpu-2404', 'mesa-2404'),
)
LAUNCHER = """#!/bin/sh
export SNAP_DESKTOP_RUNTIME="$SNAP/gnome-platform"
export GTK_USE_PORTAL=1
export LD_LIBRARY_PATH="$SNAP/usr/lib/x86_64-linux-gnu:${LD_LIBRARY_PATH:-}"
exec "$SNAP/gpu-2404/bin/gpu-2404-provider-wrapper" "$SNAP/gnome-platform/command-chain/desktop-launch" "$SNAP/bin/ubrus" "$@"
"""


def check_build_host() -> None:
    architecture = subprocess.check_output(['dpkg', '--print-architecture'], text=True).strip()
    release = platform.freedesktop_os_release()
    if (
        architecture != 'amd64'
        or release.get('ID') != 'ubuntu'
        or release.get('VERSION_ID') != '24.04'
        or not CORE.is_dir()
        or not GNOME.is_dir()
    ):
        raise SystemExit(
            'Use snapcraft pack, or install the core24 and gnome-46-2404 runtimes on Ubuntu 24.04 amd64.'
        )


def copy_file(source: Path, destination: Path) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)


def debian_package(path: Path) -> str:
    # Debian's package database may record the pre-usrmerge spelling of a library.
    candidates = [str(path)]
    if path.is_relative_to('/usr/lib'):
        candidates.append(str(path).removeprefix('/usr'))
    for candidate in candidates:
        result = subprocess.run(['dpkg-query', '-S', candidate], text=True, capture_output=True)
        if result.returncode == 0:
            return result.stdout.split(':', maxsplit=1)[0]
    raise RuntimeError(f'No Debian package owns {path}')


def stage_codecs(stage: Path) -> None:
    plugin = Path('/') / LIBDIR / 'gstreamer-1.0/libgstlibav.so'
    dependencies = subprocess.check_output(['ldd', plugin], text=True)
    if 'not found' in dependencies:
        raise RuntimeError(f'Unresolved dependencies for {plugin}:\n{dependencies}')

    copy_file(plugin, stage / plugin.relative_to('/'))
    owners = {debian_package(plugin)}
    for name, path in re.findall(r'^\s*(\S+) => (/\S+)', dependencies, re.MULTILINE):
        in_runtime = any((runtime / LIBDIR / name).exists() for runtime in (CORE, GNOME))
        # Ship the FFmpeg ABI that Ubuntu's libav plugin was linked against.
        is_ffmpeg = re.match(
            r'lib(av(codec|filter|format|util)|sw(resample|scale)|postproc)\.so', name
        )
        if in_runtime and not is_ffmpeg:
            continue
        library = Path(path)
        copy_file(library, stage / LIBDIR / name)
        owners.add(debian_package(library.resolve()))

    for owner in sorted(owners):
        copyright_file = Path('usr/share/doc') / owner / 'copyright'
        copy_file(Path('/') / copyright_file, stage / copyright_file)


def stage_metadata(stage: Path, recipe: dict) -> None:
    metadata = {field: recipe[field] for field in RECIPE_FIELDS}
    metadata['architectures'] = ['amd64']
    metadata['plugs'] = {}
    for plug, directory, provider in CONTENT_PLUGS:
        (stage / directory).mkdir()
        metadata['plugs'][plug] = {
            'interface': 'content',
            'target': f'$SNAP/{directory}',
            'default-provider': provider,
        }
    app = recipe['apps']['ubrus'].copy()
    del app['extensions'], app['desktop']
    app['command'] = 'bin/launch'
    metadata['apps'] = {'ubrus': app}

    gui = stage / 'meta/gui'
    gui.mkdir(parents=True)
    (stage / 'meta/snap.yaml').write_text(
        yaml.safe_dump(metadata, sort_keys=False), encoding='utf-8'
    )
    copy_file(Path('packaging/desktop/org.ubrus.Ubrus.svg'), gui / 'icon.svg')
    desktop = Path('packaging/desktop/org.ubrus.Ubrus.desktop').read_text(encoding='utf-8')
    desktop = desktop.replace('Icon=org.ubrus.Ubrus', 'Icon=${SNAP}/meta/gui/icon.svg')
    (gui / 'ubrus.desktop').write_text(desktop, encoding='utf-8')


def build_snap(output: Path, recipe: dict) -> None:
    with tempfile.TemporaryDirectory(prefix='ubrus-snap-') as directory:
        stage = Path(directory)
        stage.chmod(0o755)
        copy_file(Path('target/release/ubrus'), stage / 'bin/ubrus')
        subprocess.run(['bash', 'packaging/snap/install.sh', stage], check=True)
        stage_codecs(stage)
        launcher = stage / 'bin/launch'
        launcher.write_text(LAUNCHER, encoding='utf-8')
        launcher.chmod(0o755)
        stage_metadata(stage, recipe)
        subprocess.run(['snap', 'pack', '--check-skeleton', stage], check=True)
        subprocess.run(
            [
                'mksquashfs',
                stage,
                output,
                '-noappend',
                '-all-root',
                '-no-xattrs',
                '-comp',
                'xz',
                '-processors',
                '1',
                '-no-progress',
            ],
            check=True,
        )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    check_build_host()
    recipe = yaml.safe_load(Path('snap/snapcraft.yaml').read_text(encoding='utf-8'))
    if args.output.name != f'ubrus_{recipe["version"]}_amd64.snap':
        parser.error('Cargo and Snap release versions must match.')
    build_snap(args.output, recipe)
    print(args.output)


if __name__ == '__main__':
    main()
