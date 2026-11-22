#!/bin/bash
# Validate the DEB against its companion artifacts and assemble the GitHub downloads.
set -euo pipefail
tag=${1:?Expected the release tag}
out=${2:-dist/github}
version=$(/usr/bin/python3 packaging/check-release.py --tag "$tag")
package="dist/ubrus_${version}_amd64.deb"
[[ $(dpkg-deb -f "$package" Package) == ubrus ]]
[[ $(dpkg-deb -f "$package" Version) == "$version" ]]
[[ $(dpkg-deb -f "$package" Architecture) == amd64 ]]

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
dpkg-deb -x "$package" "$tmp/deb"
unzip -q dist/ubrus-extension.zip -d "$tmp/extension"
diff -qr "$tmp/extension" "$tmp/deb/usr/share/gnome-shell/extensions/wallpaper@ubrus.org"
cmp dist/ubrus "$tmp/deb/usr/bin/ubrus"
desktop-file-validate "$tmp/deb/usr/share/applications/org.ubrus.Ubrus.desktop"
appstreamcli validate --no-net "$tmp/deb/usr/share/metainfo/org.ubrus.Ubrus.metainfo.xml"
cmp packaging/desktop/org.ubrus.Ubrus.desktop "$tmp/deb/usr/share/applications/org.ubrus.Ubrus.desktop"
cmp packaging/desktop/org.ubrus.Ubrus.metainfo.xml "$tmp/deb/usr/share/metainfo/org.ubrus.Ubrus.metainfo.xml"
licenses="$tmp/deb/usr/share/doc/ubrus"
cmp LICENSE-MIT "$licenses/LICENSE-MIT"
cmp LICENSE-APACHE "$licenses/LICENSE-APACHE"
test -s "$licenses/THIRD-PARTY-NOTICES"

mkdir -p "$tmp/ubrus" "$out"
cp dist/ubrus dist/ubrus-extension.zip "$licenses"/{LICENSE-MIT,LICENSE-APACHE,THIRD-PARTY-NOTICES} "$tmp/ubrus/"
tar -czf "$out/ubrus-linux-amd64.tar.gz" -C "$tmp" ubrus
cp "$package" "$out/ubrus-amd64.deb"
cp dist/ubrus-extension.zip "$out/"
(
    cd "$out"
    sha256sum ubrus-amd64.deb ubrus-linux-amd64.tar.gz ubrus-extension.zip > SHA256SUMS
    sha256sum --strict --check SHA256SUMS
)
