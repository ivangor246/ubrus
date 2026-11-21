"""Collect third-party license notices for the registry crates in Cargo.lock."""

import argparse
import os
import tomllib
from collections.abc import Iterator
from pathlib import Path

LICENSE_PREFIXES = ('LICENSE', 'COPYING', 'NOTICE', 'COPYRIGHT')


def crate_notices(registry: Path, name: str, version: str) -> str:
    crate = f'{name}-{version}'
    parts = [f'{name} {version}', f'https://crates.io/api/v1/crates/{name}/{version}/download']
    source = next(registry.glob(f'*/{crate}'), None)
    if source is not None:
        with (source / 'Cargo.toml').open('rb') as manifest:
            package = tomllib.load(manifest)['package']
        parts.append(package.get('license', package.get('license-file', '')))
        for path in sorted(source.iterdir()):
            if path.is_file() and path.name.upper().startswith(LICENSE_PREFIXES):
                parts.append(path.read_text(encoding='utf-8', errors='replace'))
    return '\n'.join(parts)


def locked_notices(lockfile: Path, registry: Path) -> Iterator[str]:
    with lockfile.open('rb') as file:
        packages = tomllib.load(file)['package']
    for package in packages:
        if package.get('source', '').startswith('registry+'):
            yield crate_notices(registry, package['name'], package['version'])


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    cargo_home = Path(os.environ.get('CARGO_HOME', Path.home() / '.cargo'))
    notices = locked_notices(Path('Cargo.lock'), cargo_home / 'registry' / 'src')
    args.output.write_text('\n\n'.join(notices), encoding='utf-8')


if __name__ == '__main__':
    main()
