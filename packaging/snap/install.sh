#!/bin/bash
set -euo pipefail
dir=$1
bash packaging/install-assets.sh "$dir"
install -Dm644 dist/ubrus-extension.zip "$dir/share/ubrus/ubrus-extension.zip"
python3 - "$dir/share/ubrus" <<'PY'
import json
import sys
from pathlib import Path
from zipfile import ZipFile

assets = Path(sys.argv[1])
with ZipFile(assets / "ubrus-extension.zip") as extension:
    build_id = json.loads(extension.read("metadata.json"))["build-id"]
(assets / "extension-build-id").write_text(build_id + "\n", encoding="ascii")
PY
