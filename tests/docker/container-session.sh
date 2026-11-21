#!/bin/bash
set -euo pipefail
mkdir -p /run/dbus
dbus-daemon --system --fork
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml)
package="./dist/ubrus_${version}_$(dpkg --print-architecture).deb"
# Reinstall the exact local build, including when its version matches the previous test.
if ! dpkg -i "$package"; then
    apt-get update -qq
    apt-get install --fix-broken --no-remove --no-install-recommends -y
fi
export UBRUS_TEST_BINARY=/usr/bin/ubrus UBRUS_TEST_INSTALLED=1
exec runuser -u ubuntu -- bash tests/session/test-session.sh wayland
