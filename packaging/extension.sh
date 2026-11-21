#!/bin/bash
set -euo pipefail
mkdir -p dist
stage=$(mktemp -d dist/.extension.XXXXXX)
trap 'rm -rf "$stage"' EXIT
python3 - "$stage/ubrus-extension.zip" <<'PY'
import hashlib
import json
import sys
import zipfile
from pathlib import Path

root = Path('extension')
files = sorted([*root.glob('*.js'), *root.glob('desktop/*.js'), *root.glob('icons/*.svg'), root/'locale.json'])
digest = hashlib.sha256()
with zipfile.ZipFile(sys.argv[1], 'w', zipfile.ZIP_DEFLATED) as archive:
    def add(name, contents):
        entry = zipfile.ZipInfo(name)
        entry.create_system = 3
        entry.external_attr = 0o100644 << 16
        archive.writestr(entry, contents, compress_type=zipfile.ZIP_DEFLATED)

    for path in files:
        name = path.relative_to(root).as_posix()
        data = path.read_bytes()
        digest.update(name.encode() + b'\0' + hashlib.sha256(data).digest())
        add(name, data)
    metadata = json.loads((root/'metadata.json').read_bytes())
    digest.update(json.dumps(metadata, sort_keys=True).encode())
    metadata['build-id'] = digest.hexdigest()
    add('metadata.json', json.dumps(metadata))
    for license in ['LICENSE-MIT', 'LICENSE-APACHE']:
        add(license, Path(license).read_bytes())
PY
mv "$stage/ubrus-extension.zip" dist/
