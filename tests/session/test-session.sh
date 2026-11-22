#!/bin/bash
set -euo pipefail
mode=${1:-wayland}
root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root"
temp=$(mktemp -d)
cleanup() {
    if [[ -n ${xvfb_pid:-} ]]; then
        kill "$xvfb_pid" 2>/dev/null || true
        wait "$xvfb_pid" 2>/dev/null || true
    fi
    for m in "$temp/run/doc" "$temp/run/gvfs"; do
        if mountpoint -q "$m"; then fusermount3 -uz "$m" 2>/dev/null || true; fi
    done
    for delay in 1 2 3; do
        if rm -rf --one-file-system "$temp" 2>/dev/null; then return; fi
        sleep .3
    done
    rm -rf --one-file-system "$temp"
}
trap cleanup EXIT
export XDG_RUNTIME_DIR="$temp/run" XDG_CONFIG_HOME="$temp/config" XDG_DATA_HOME="$temp/data" XDG_CACHE_HOME="$temp/cache"
export XDG_STATE_HOME="$temp/state" DCONF_PROFILE=user GSETTINGS_BACKEND=keyfile
export GDK_BACKEND="$mode" XDG_CURRENT_DESKTOP=GNOME XDG_SESSION_TYPE="$mode" UBRUS_TEST_BINARY="${UBRUS_TEST_BINARY:-$root/target/debug/ubrus}"
unset UBRUS_RENDERER
export LP_NUM_THREADS=1 NO_AT_BRIDGE=1 GTK_A11Y=none
if [[ ${UBRUS_TEST_SOFTWARE:-0} == 1 ]]; then export LIBGL_ALWAYS_SOFTWARE=1 GSK_RENDERER=cairo; fi
mkdir -p -m 700 "$XDG_RUNTIME_DIR" "$XDG_CONFIG_HOME" "$XDG_DATA_HOME"
gsettings set org.gnome.desktop.interface enable-animations false
mkdir -p "$XDG_DATA_HOME/applications" "$XDG_RUNTIME_DIR/downloads"
printf 'XDG_DOWNLOAD_DIR="%s"\n' "$XDG_RUNTIME_DIR/downloads" > "$XDG_CONFIG_HOME/user-dirs.dirs"
cp packaging/desktop/org.ubrus.Ubrus.desktop "$XDG_DATA_HOME/applications/"
export GIO_USE_VFS=local GVFS_DISABLE_FUSE=1
if test -f "$HOME/.cache/gstreamer-1.0/registry.x86_64.bin"; then
    mkdir -p "$XDG_CACHE_HOME/gstreamer-1.0"
    cp "$HOME/.cache/gstreamer-1.0/registry.x86_64.bin" "$XDG_CACHE_HOME/gstreamer-1.0/"
fi
if [[ "$mode" == x11 || -z ${UBRUS_TEST_INSTALLED:-} ]]; then
    export XDG_DESKTOP_PORTAL_DIR="$temp/portals"
    mkdir -p "$temp/portals"
    ln -s /usr/share/xdg-desktop-portal/portals/*.portal "$temp/portals/"
    cat > "$temp/portals/ubrustest.portal" <<'PORTAL'
[portal]
DBusName=org.freedesktop.impl.portal.desktop.ubrustest
Interfaces=org.freedesktop.impl.portal.FileChooser;
PORTAL
    cat > "$temp/portals/portals.conf" <<'PORTALS'
[preferred]
default=gnome;gtk;
org.freedesktop.impl.portal.FileChooser=ubrustest
PORTALS
fi
unset DBUS_SESSION_BUS_ADDRESS GNOME_SHELL_SESSION_MODE SESSION_MANAGER DISPLAY WAYLAND_DISPLAY
log_dir=${UBRUS_TEST_LOG_DIR:-tests/artifacts/session-tests}
mkdir -p "$log_dir"
# D-Bus activated dialogs and the test application must use the same isolated display.
if [[ "$mode" == x11 ]]; then
    # Start with a known system language before exercising the application overrides.
    export LC_ALL=C.UTF-8 LANGUAGE=en
    export XAUTHORITY="$temp/Xauthority"
    touch "$XAUTHORITY"
    cookie=$(mcookie)
    xauth add :0 MIT-MAGIC-COOKIE-1 "$cookie"
    geometry=1280x720x24
    if [[ ${UBRUS_TEST_COMPONENT:-} == controller ]]; then
        geometry=2000x1250x24
        export UBRUS_TEST_WIDGETS="$temp/widgets"
        mkdir -m 700 "$UBRUS_TEST_WIDGETS"
    fi
    "${XVFB:-Xvfb}" -displayfd 3 -screen 0 "$geometry" -nolisten tcp -noreset -auth "$XAUTHORITY" \
        3>"$temp/display" 2>"$log_dir/xvfb.log" &
    xvfb_pid=$!
    for ((i=0; i<100; i++)); do
        [[ -s "$temp/display" ]] && break
        kill -0 "$xvfb_pid" || { tail -20 "$log_dir/xvfb.log" >&2; exit 1; }
        sleep 0.1
    done
    read -r number <"$temp/display"
    [[ "$number" =~ ^[0-9]+$ ]]
    export DISPLAY=":$number"
    xauth add "$DISPLAY" MIT-MAGIC-COOKIE-1 "$cookie"
else
    export WAYLAND_DISPLAY=ubrus-integration
fi
log="$log_dir/$mode.log"
if dbus-run-session -- /usr/bin/python3 "$root/tests/session/test-session.py" "$mode" >"$log" 2>&1; then
    grep "^$mode:" "$log"
else
    grep -E "^(RuntimeError|AssertionError|Traceback|$mode:)|JS ERROR|ubrus@|Ubrus:|^Failure diagnostics|^Error:" "$log" | tail -15 || true
    tail -15 "$log"
    exit 1
fi
