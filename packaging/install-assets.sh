#!/bin/bash
set -euo pipefail
dir=$1
install -Dm644 packaging/desktop/org.ubrus.Ubrus.desktop "$dir/usr/share/applications/org.ubrus.Ubrus.desktop"
install -Dm644 packaging/desktop/org.ubrus.Ubrus.metainfo.xml "$dir/usr/share/metainfo/org.ubrus.Ubrus.metainfo.xml"
install -Dm644 packaging/desktop/org.ubrus.Ubrus.svg "$dir/usr/share/icons/hicolor/scalable/apps/org.ubrus.Ubrus.svg"
install -Dm644 LICENSE-MIT "$dir/usr/share/doc/ubrus/LICENSE-MIT"
install -Dm644 LICENSE-APACHE "$dir/usr/share/doc/ubrus/LICENSE-APACHE"
python3 packaging/notices.py "$dir/usr/share/doc/ubrus/THIRD-PARTY-NOTICES"
