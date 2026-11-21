#!/bin/bash
set -euo pipefail
dir=$1
bash packaging/install-assets.sh "$dir"
install -Dm644 dist/ubrus-extension.zip "$dir/share/ubrus/ubrus-extension.zip"
