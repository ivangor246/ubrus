# Tests

Run these commands from the project root.

| Command | Coverage |
| --- | --- |
| `make check` | Rust formatting, Clippy and all unit tests |
| `make test` | Rust tests plus Node.js and GJS extension tests |
| `make test-controller` | Private X11 window at three sizes, portal and panel with a Python renderer |
| `make test-renderer` | Private X11 renderer, per-monitor framing and playback rates with a Python protocol peer |
| `make test-update` | Same-version DEB replacement in a temporary install tree |
| `make test-integration` | Private GNOME Wayland session with two virtual monitors |
| `make test-integration SESSION=x11` | X11 session with Xvfb, portal dialogs and the Iced window |

Unit tests need Node.js and GJS. For session tests install GNOME Shell, python3-gi, gir1.2-gtk-4.0, dbus-daemon, gstreamer1.0-tools, python3-gi-cairo, xauth, xdotool, x11-utils, python3-pil and xvfb.

Use the component targets one at a time on machines restricted to one running Rust application. The full integration targets run both real components.

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

The session test samples the renderer for three seconds while playing a local 1080p/30 VP8 clip. CPU is a percentage of one logical core. RSS includes GTK, GStreamer and driver allocations. Use `UBRUS_BENCH_ACTIVE_SECONDS=60 UBRUS_BENCH_SECONDS=60` to measure a minute of continuous playback and a minute with all outputs covered. Set `GST_PLUGIN_FEATURE_RANK=nvvp8dec:NONE` to repeat with the software VP8 decoder on a system using the NVIDIA decoder. The log records the decoder actually selected.

Pass `UBRUS_TEST_COMPONENT=gif` or `webp` to measure a 1920×1080 animated image on both virtual outputs. Their GdkPixbuf clock reports published frames. It does not provide a dropped-frame counter. For a library benchmark, pass `UBRUS_TEST_COMPONENT=library`. The fixture measures time until the settings window maps and samples RSS 600 ms later. Run the library benchmark against the release build with these commands.

```sh
make release
make test-integration UBRUS_TEST_BINARY="$PWD/dist/ubrus" UBRUS_TEST_COMPONENT=library
```

On Ubuntu 24.04 Wayland, two release runs with ten PNGs opened in 0.12 s and used 49–50 MiB. With 500 PNGs the window opened in 0.12–0.13 s and used 60–61 MiB. The previous implementation measured 0.85–0.86 s and 258–261 MiB with 500 PNGs on this host.

On a laptop with discharging batteries that expose `energy_now` under Linux power_supply, intervals of at least 60 seconds also report whole-system watts. Keep display brightness and background activity steady and repeat both intervals. The runner omits the reading when the energy counter is unavailable or unchanged.

### Ubuntu 24.04 measurements for 0.2.0

The Wayland session used two virtual outputs (800×600 and 640×480) and one shared decoder per source. VP8 displayed all 1,800 frames over 60 seconds. With both outputs covered for another minute, its counters stayed unchanged at 2.4% of one CPU core.

| 1080p source | Decoder | Interval | Delivered frames | CPU, one core | RSS |
| --- | --- | ---: | ---: | ---: | ---: |
| VP8, 30 fps | nvvp8dec | 60 s | 1,800 rendered, 0 sink drops | 69.2% | 503 MiB |
| GIF, 25 fps | GdkPixbuf | 10.1 s | 250 published | 103.0% | 256 MiB |
| WebP, 25 fps | GdkPixbuf | 10.1 s | 250 published | 67.2% | 391 MiB |

The first published frame was observed 0.73 s after controller launch for GIF and 1.09 s for WebP. A separate one-vCPU KVM test installed the strict Snap and exercised two outputs with software VP8 decoding. It recorded 89.1% of one core and 344 MiB RSS over its three-second active sample, with six sink drops in the current loop.

On Ubuntu 26.04 with GNOME 50, the installed DEB passed monitor rearrangement, coverage, extension restart and recovery in a one-vCPU container. Software VP8 decoding could not sustain 1080p/30 in that container. Over 60 seconds it decoded 794 frames, the sink rendered 87 and dropped 641, and the renderer used 92.4% of one core and 425 MiB RSS. With both outputs covered, the counters stayed unchanged for 60 seconds and renderer CPU measured 0.0%.

### Earlier measurements

| Session tested with 0.1.0 | Virtual outputs | Decoder | CPU | RSS |
| --- | --- | --- | ---: | ---: |
| Ubuntu 24.04 Wayland | 800×600 and 640×480 | nvvp8dec | 66% | 503 MiB |
| Ubuntu 24.04 Xvfb, Cairo rendering | 1280×720 | nvvp8dec | 132% | 624 MiB |
| Ubuntu 26.04 Wayland, single-vCPU container | 800×600 and 640×480 | vp8dec | 89% | 395 MiB |
| Ubuntu 24.04 Wayland, installed strict Snap in a single-vCPU KVM guest | 800×600 and 640×480 | vp8dec | 78% | 334 MiB |

The Xvfb and single-vCPU tests recorded dropped frames. The Noble Wayland test held its decoded and rendered
frame counters steady for 60 seconds with both outputs covered. Repeat the measurement in the target
graphical session when evaluating a particular GPU, video format or resolution.
