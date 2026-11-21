#!/bin/bash
set -euo pipefail
mkdir -p dist
stage=$(mktemp -d dist/.extension.XXXXXX)
trap 'rm -rf "$stage"' EXIT
(
    cd extension
    python3 -m zipfile -c "../$stage/ubrus-extension.zip" \
        *.js desktop metadata.json ../LICENSE-MIT ../LICENSE-APACHE
)
mv "$stage/ubrus-extension.zip" dist/
