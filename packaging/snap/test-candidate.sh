#!/bin/bash
# Run on a disposable Ubuntu amd64 runner with snapd and network access.
set -euo pipefail
export LC_ALL=C
tag=${1:?Expected a release tag}
revision=${2:?Expected the signed candidate revision}
[[ "$revision" =~ ^[1-9][0-9]*$ ]]
if snap list ubrus >/dev/null 2>&1; then
    echo "The candidate test needs a clean runner." >&2
    exit 1
fi
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
sudo apt-get update -qq
sudo apt-get install -y --no-install-recommends python3-yaml squashfs-tools gnome-shell dbus-user-session \
    python3-gi gir1.2-gtk-4.0 gstreamer1.0-tools gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
    libgl1-mesa-dri libegl-mesa0 fonts-dejavu-core unzip python3-cairo python3-gi-cairo
snap download ubrus --channel=latest/candidate --basename=candidate --target-directory="$tmp"
sudo snap ack "$tmp/candidate.assert"
test "$(sed -n 's/^snap-revision: //p' "$tmp/candidate.assert")" = "$revision"
bash packaging/extension.sh
/usr/bin/python3 packaging/check-release.py --tag "$tag" --snap "$tmp/candidate.snap"

check_installed() {
    test "$(snap list ubrus | awk 'NR == 2 {print $3}')" = "$revision"
    test "$(snap run ubrus --version)" = "Ubrus ${tag#v}"
    test "$(readlink /snap/ubrus/current)" = "$revision"
    test -s /snap/ubrus/current/share/ubrus/ubrus-extension.zip
    snap connections ubrus
}

sudo snap install ubrus --channel=latest/candidate
check_installed
snap info ubrus > "$tmp/info"
if awk '$1 == "latest/stable:" && /\([0-9]+\)/ {found=1} END {exit !found}' "$tmp/info"; then
    sudo snap remove --purge ubrus
    sudo snap install ubrus --channel=latest/stable
    previous=$(snap list ubrus | awk 'NR == 2 {print $3}')
    sudo snap refresh ubrus --channel=latest/candidate
    check_installed
    echo "Checked latest/stable ($previous) to latest/candidate ($revision)." >> "${GITHUB_STEP_SUMMARY:-/dev/stdout}"
else
    echo "Fresh candidate $revision installed. First release: no stable revision for upgrade testing." >> "${GITHUB_STEP_SUMMARY:-/dev/stdout}"
fi

# Confinement needs a real user runtime directory and session bus. Start a
# separate login on this disposable runner and exercise the installed revision.
sudo useradd --create-home --shell /bin/bash ubrus-test
sudo loginctl enable-linger ubrus-test
uid=$(id -u ubrus-test)
sudo systemctl start "user@$uid.service"
sudo mkdir /home/ubrus-test/tests
sudo cp -r tests/session tests/media /home/ubrus-test/tests/
sudo systemd-run --wait --pipe --collect --property=User=ubrus-test --property=PAMName=login \
    --setenv=XDG_SESSION_TYPE=wayland --setenv=XDG_SESSION_DESKTOP=gnome \
    env "XDG_RUNTIME_DIR=/run/user/$uid" "DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$uid/bus" \
    HOME=/home/ubrus-test XDG_DATA_HOME=/home/ubrus-test/.local/share XDG_CURRENT_DESKTOP=GNOME \
    GDK_BACKEND=wayland UBRUS_TEST_LIBRARY=/home/ubrus-test/snap/ubrus/common \
    UBRUS_TEST_DISPLAY=wayland-0 UBRUS_TEST_BINARY=/snap/bin/ubrus UBRUS_TEST_INSTALLED=1 \
    UBRUS_TEST_LOG_DIR=/home/ubrus-test/logs LP_NUM_THREADS=1 GSK_RENDERER=cairo LIBGL_ALWAYS_SOFTWARE=1 bash -ec '
        cd "$HOME"
        extension="$XDG_DATA_HOME/gnome-shell/extensions/wallpaper@ubrus.org"
        mkdir -p "$extension"
        unzip -q /snap/ubrus/current/share/ubrus/ubrus-extension.zip -d "$extension"
        /usr/bin/python3 tests/session/test-session.py wayland
    '
