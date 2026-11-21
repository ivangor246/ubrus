# Tests

Run these commands from the project root.

| Command | Coverage |
| --- | --- |
| `make check` | Rust formatting, Clippy and all unit tests |
| `make test` | Rust tests plus Node.js and GJS extension tests |
| `make test-integration` | Private GNOME Wayland session with two virtual monitors |
| `make test-integration SESSION=x11` | X11 session with Xvfb, portal dialogs and the Iced window |

Unit tests need Node.js and GJS. For session tests install GNOME Shell, python3-gi, gir1.2-gtk-4.0, dbus-daemon, gstreamer1.0-tools, python3-gi-cairo, xauth, xdotool, x11-utils, python3-pil and xvfb.

## Desktop sessions

The runner uses temporary XDG directories and a private D-Bus. It checks frame delivery, pausing, extension restarts and recovery when a controller exits mid-assignment. Wayland also checks monitor rearrangement. X11 drives the settings window through import, theme and language changes, report export and login restoration.

Logs are in `tests/artifacts/session-tests/`. Set `XVFB` to use a specific Xvfb executable. To test a release binary, run `make release` followed by `make test-integration UBRUS_TEST_BINARY="$PWD/dist/ubrus"`.

## Installed packages

- **DEB on GNOME 50.** Build with `make deb` on Ubuntu 24.04, then run `docker compose -f tests/docker/compose.test.yaml up --build --abort-on-container-exit --exit-code-from gnome50 gnome50`.
- **Strict Snap.** Build with `make snap`. The single-vCPU KVM guest uses the host’s core24, mesa-2404 and gnome-46-2404 Snap files. Put the amd64 minimal cloud image and its matching SHA256SUMS in `tests/artifacts/vm/`. [Ubuntu 24.04 images](https://cloud-images.ubuntu.com/minimal/releases/noble/release/)

```sh
docker compose -f tests/docker/compose.test.yaml up --build --abort-on-container-exit --exit-code-from snap-vm snap-vm
docker compose -f tests/docker/compose.test.yaml --profile snap-test down
```

## Playback measurements

The session test samples the renderer for three seconds while playing a local 1080p/30 VP8 clip. CPU is a percentage of one logical core. RSS includes GTK, GStreamer and driver allocations. Set `UBRUS_BENCH_SECONDS=60` to check that frame counters remain steady during a minute of full coverage.

| Session tested with 0.1.0 | Virtual outputs | Decoder | CPU | RSS |
| --- | --- | --- | ---: | ---: |
| Ubuntu 24.04 Wayland | 800×600 and 640×480 | nvvp8dec | 66% | 503 MiB |
| Ubuntu 24.04 Xvfb, Cairo rendering | 1280×720 | nvvp8dec | 132% | 624 MiB |
| Ubuntu 26.04 Wayland, single-vCPU container | 800×600 and 640×480 | vp8dec | 89% | 395 MiB |
| Ubuntu 24.04 Wayland, installed strict Snap in a single-vCPU KVM guest | 800×600 and 640×480 | vp8dec | 78% | 334 MiB |

The Xvfb and single-vCPU tests recorded dropped frames. The Noble Wayland test held its decoded and rendered
frame counters steady for 60 seconds with both outputs covered. Repeat the measurement in the target
graphical session when evaluating a particular GPU, video format or resolution.
