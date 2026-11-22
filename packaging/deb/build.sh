#!/bin/bash
set -euo pipefail
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml)
arch=$(dpkg --print-architecture)
dir=$(mktemp -d)
chmod 755 "$dir"
umask 022
mkdir -p dist
stage=$(mktemp -d dist/.deb.XXXXXX)
trap 'rm -rf "$dir" "$stage"' EXIT
install -Dm755 target/release/ubrus "$dir/usr/bin/ubrus"
prefix="$dir/usr/share"
bash packaging/install-assets.sh "$dir"
mkdir -p "$prefix/gnome-shell/extensions/wallpaper@ubrus.org" "$prefix/doc/ubrus" "$dir/DEBIAN" dist
unzip -qo dist/ubrus-extension.zip -d "$prefix/gnome-shell/extensions/wallpaper@ubrus.org"
cp README.md "$prefix/doc/ubrus/"
cp -r docs "$prefix/doc/ubrus/"
install -Dm644 tests/README.md "$prefix/doc/ubrus/tests/README.md"
cat LICENSE-MIT LICENSE-APACHE > "$prefix/doc/ubrus/copyright"
sed -e "s/@VERSION@/$version/" -e "s/@ARCH@/$arch/" packaging/deb/control.in > "$dir/DEBIAN/control"
package="ubrus_${version}_${arch}.deb"
dpkg-deb --threads-max=1 --root-owner-group --build "$dir" "$stage/$package"
mv "$stage/$package" dist/
