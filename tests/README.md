# Testing Ubrus

Run one target at a time from the repository root. Start with `make check` for formatting, Clippy, Rust, Node.js and GJS tests. [Build dependencies](../docs/development.md)

## Desktop sessions

| Command | What it tests |
| --- | --- |
| `make test-controller` | X11 editor layout, preferences, keyboard navigation, portal dialogs and Shell panel actions with a test renderer |
| `make test-renderer` | X11 renderer framing and playback rates with a test controller |
| `make test-integration` | GNOME Wayland with two virtual monitors, pausing, monitor rearrangement and recovery |
| `make test-integration SESSION=x11` | Full X11 session with real playback, multi-file import, extension restart and controller recovery |
| `make test-update` | Same-version DEB replacement and settings recovery |

Sessions use private XDG directories and D-Bus. Logs are in `tests/artifacts/session-tests/`. Set `XVFB` for a custom Xvfb binary or `UBRUS_TEST_BINARY` to test a specific executable. The controller target builds with the `ui-test` feature. Its private session reads the visible widget bounds and keyboard focus from Iced and sends real X11 input. A custom controller binary needs that feature too. CI exercises the editor at 125% scale. The CI workflow lists the Ubuntu desktop packages needed to run these tests. [CI workflow](https://github.com/ivangor246/ubrus/blob/main/.github/workflows/ci.yml)

## Installed packages

**DEB on GNOME 50.** Build the package on Ubuntu 24.04, then test it in the Ubuntu 26.04 container.

```sh
make deb
docker compose -f tests/docker/compose.test.yaml up --build --abort-on-container-exit --exit-code-from gnome50 gnome50
```

**Strict Snap.** The KVM guest uses the host's core24, mesa-2404 and gnome-46-2404 snaps. Put the Ubuntu 24.04 minimal amd64 cloud image and its matching `SHA256SUMS` in `tests/artifacts/vm/`. [Ubuntu cloud images](https://cloud-images.ubuntu.com/minimal/releases/noble/release/)

```sh
make snap
docker compose -f tests/docker/compose.test.yaml up --build --abort-on-container-exit --exit-code-from snap-vm snap-vm
docker compose -f tests/docker/compose.test.yaml --profile snap-test down
```

## Performance

Build with `make release`. The first command below measures library startup time and RSS. The second measures a minute of 1080p/30 VP8 playback and another minute with both monitors covered.

```sh
make test-integration UBRUS_TEST_BINARY="$PWD/dist/ubrus" UBRUS_TEST_COMPONENT=library
make test-integration UBRUS_TEST_BINARY="$PWD/dist/ubrus" UBRUS_BENCH_ACTIVE_SECONDS=60 UBRUS_BENCH_SECONDS=60
```

Use `UBRUS_TEST_COMPONENT=gif` or `webp` for animated images. On NVIDIA systems, `GST_PLUGIN_FEATURE_RANK=nvvp8dec:NONE` disables the NVIDIA VP8 decoder for comparison. The log identifies the decoder used. On a discharging laptop with an `energy_now` counter, intervals of at least 60 seconds also report whole-system power.

### Version 0.2.0 results

On Ubuntu 24.04 Wayland the editor opened a library of 500 PNGs in 0.12–0.13 s and used 60–61 MiB RSS, measured 600 ms after the window appeared. The previous implementation took 0.85–0.86 s and used 258–261 MiB on the same host.

Playback used two virtual outputs (800×600 and 640×480). CPU is a percentage of one core. GdkPixbuf reports published frames and has no dropped-frame counter.

| Source and decoder | Interval | Frames | CPU | RSS |
| --- | ---: | --- | ---: | ---: |
| VP8, nvvp8dec | 60 s | 1,800 rendered, 0 dropped | 69.2% | 503 MiB |
| GIF, GdkPixbuf | 10.1 s | 250 published | 103.0% | 256 MiB |
| WebP, GdkPixbuf | 10.1 s | 250 published | 67.2% | 391 MiB |

With both outputs covered, the hardware decoder's frame counters stayed steady for 60 seconds and renderer CPU fell to 2.4%. The one-vCPU GNOME 50 container could not sustain 1080p/30 with software VP8 decoding. Its sink rendered 87 frames and dropped 641 in 60 seconds.
