# Native cpal PipeWire Host Evaluation — Decision Record

**Capture date:** 2026-09-13
**Code under evaluation:** `src/integrations/system_audio.rs` — the custom PipeWire/PulseAudio
integration (server detection, live clock configuration, config persistence, status, and
service restart) plus the current ALSA-compatibility host and device enumeration in
`src/audio/device_manager.rs`
**Candidate:** `cpal 0.18.2`'s default-off `pipewire` feature, which pulls the `pipewire 0.10`
crate and exposes `cpal::platform::PipeWireHost`
**Commit:** `2dcf083c2960e66c89baf320d5e4440192f7e03f`
**Reference host:** `tix-slx` (see Machine / OS)

Outcome: keep the custom path; do not enable the pipewire feature

Confirmed by human: 2026-09-13 — the reviewer confirmed the recorded outcome (keep the custom path, feature disabled) and that the custom `force-rate`/`force-quantum` clock configuration and the audio-service restart/recovery action are preserved.

## Question

Does the native `cpal` PipeWire host beat the current ALSA-compatibility path for stream
open and device enumeration by enough to justify enabling the `pipewire` cargo feature —
given that doing so adds a `libpipewire-0.3` build dependency and that the custom
system-audio integration, which the native host does not replace, must be preserved? Keep
the custom path and leave the feature disabled unless the native host is a documented net
positive.

## Pre-registered decision criteria

Recorded before the evaluation was concluded:

- **Stream open / enumeration win:** the native host must measurably beat the current
  ALSA-compatibility path (`cpal::default_host()` resolving to the ALSA host, which on a
  PipeWire system routes through `pipewire-alsa`) for opening a stream and listing devices.
  A tie or a marginal difference does not qualify.
- **Feature preservation:** the custom `force-rate`/`force-quantum` clock configuration and
  the audio-service restart/recovery action must remain available. The native host must not
  be adopted in a way that removes either capability.
- **Build cost acceptable:** enabling the feature must not require a development dependency
  that the reference host lacks, and must not risk breaking builds for users on a
  PipeWire runtime without the development package installed.
- **No dead code:** an optional improvement is allowed only if it is cheap and does not
  enable the feature; a detection path that can never fire with the feature off is not
  worth adding.

## Machine / OS

| Field | Value |
|-------|-------|
| Host | `tix-slx` |
| OS | Soplos Linux Tyron (kernel `7.2.5-soplos-v3`, x86_64) |
| CPU | AMD Ryzen 7 5800X 8-Core Processor (16 hardware threads, up to ~4.85 GHz) |
| Memory | 31 GiB installed |
| Runtime | PipeWire active as a user service; `pw-metadata` present; `systemctl --user is-active pipewire` → `active` |

## Toolchain

```
rustc 1.98.1 (48a229cea 2026-09-01)
cargo 1.98.1 (797e8a9bc 2026-08-05)
pkg-config 2.5.1
```

## Evidence

### Build-time cost of enabling the feature

`cpal 0.18.2` declares the `pipewire` feature as opt-in and default-off; on Linux it maps to
the `pipewire 0.10` crate, whose build script requires the `libpipewire-0.3` development
files through pkg-config. On the reference host that development package is **absent**:

```bash
$ pkg-config --exists libpipewire-0.3; echo "exit=$?"
exit=1
$ pkg-config --modversion alsa
1.2.16.1
```

PipeWire itself is running (`systemctl --user is-active pipewire` → `active`, and
`pw-metadata` is on `PATH`), so the *runtime* server is present while the *development*
headers are not. Enabling the feature would therefore break `cargo build` on this host —
and on any user machine with a PipeWire runtime but no `libpipewire-0.3-dev` package —
until the distro package is installed. The current manifest keeps the feature off
(`cpal = "0.18.2"` with no feature selection and no `[features]` entry that enables it).

### What the native host provides — and what it does not

The native host changes only the stream backend: when the feature is on,
`cpal::default_host()` prefers PipeWire → PulseAudio → JACK → ALSA, and
`cpal::available_hosts()` would list `"PipeWire"`. It is a stream-open and
device-enumeration backend. It does **not** provide:

- The live system-clock configuration the custom integration exposes through
  `pw-metadata … clock.force-rate` / `clock.force-quantum`, which must run **before** the
  output stream is built so PipeWire does not re-configure the graph and disconnect the
  existing stream. The native host has no API for either `force-rate` or `force-quantum`.
- The user-facing persistence of the clock choice to
  `~/.config/pipewire/pipewire.conf.d/audoxidy.conf` (and the PulseAudio `daemon.conf`
  fallback), so the setting survives a service restart without third-party tooling. The
  native host does not write either file.
- The audio-service restart/recovery action (`systemctl --user restart pipewire.service
  pipewire-pulse.service wireplumber.service`) used when playback goes silent. The native
  host exposes no equivalent control.

Both of these are deliberate features of the custom integration, not debt, and neither is
replaced by the native backend. Adopting the native host would leave them in place
alongside it, so the only thing the feature could improve is the stream backend itself.

### The detection hint is dead code with the feature off

A cheap optional improvement was considered: use `cpal::available_hosts()` as a PipeWire
detection hint while keeping `systemctl` as a fallback. With the feature disabled,
`cpal::available_hosts()` is compiled without the PipeWire host and therefore can never
report `"PipeWire"`, so the hint would never fire. Implementing it would add a branch that
is always false — dead code — and provide no value unless the feature is enabled, which the
build-cost criterion above already rejects.

## Criterion disposition

- **Stream open / enumeration win — not established.** The native host is a backend swap
  only; the ALSA-compatibility path already opens streams and enumerates devices through
  `pipewire-alsa`. No measured or documented stream-open/enumeration advantage was found to
  outweigh the build dependency, so this criterion does not qualify.
- **Feature preservation — satisfied only by keeping the custom path.** The native host
  provides neither `force-rate`/`force-quantum` nor restart/recovery, so the custom
  integration must be preserved in all cases. This is a reason to keep it, not a reason to
  adopt the native host alongside it.
- **Build cost acceptable — failed.** The `libpipewire-0.3` development files are absent on
  the reference host while PipeWire runs, and enabling the feature would break the build
  until users install the development package.
- **No dead code — honored by not implementing the hint.** With the feature off the hint can
  never fire, so it is not added.

No criterion supports enabling the feature. The recorded outcome is to keep the custom path
and leave the `pipewire` feature disabled, with no code or manifest change.

## Analysis

The native host and the custom integration solve different problems. cpal's PipeWire host
decides which backend opens the stream; the custom integration decides the system clock and
recovers the audio service. The evaluation question was deliberately narrow — does the
native backend beat the ALSA-compatibility path for stream open/enumeration — and the
answer is not favorable: any backend advantage would have to be paid for with a hard
build-time dependency that the reference host cannot satisfy and that would degrade the
build experience for similarly provisioned users, while leaving the clock and restart
features exactly where they are. The ALSA-compatibility path keeps working through
`pipewire-alsa`, so there is no functional gap to close.

The existing custom path is therefore preserved as-is: `is_pipewire_active`,
`apply_pipewire_clock` (force-rate / force-quantum), `persist_pipewire_conf`,
`persist_pulse_conf`, `restart_audio_services`, and `system_sound_status` are unchanged,
and `cpal::default_host()` / `cpal::available_hosts()` in the device manager continue to
resolve to the ALSA host on a PipeWire system.

## Caveats

- This record is documentation-only: no source file, `Cargo.toml`, or `Cargo.lock` change is
  made. The `pipewire` feature stays default-off.
- The comparison is qualitative. A direct stream-open/enumeration micro-benchmark was not
  run because the candidate cannot be built on the reference host (missing
  `libpipewire-0.3`), and installing that package solely to benchmark a backend that does
  not replace the preserved features would be out of scope.
- If a future host provides `libpipewire-0.3` and a documented stream-open/enumeration win
  appears, this decision should be revisited — but the clock/restart features must be
  preserved regardless of the backend.

## References

- `src/integrations/system_audio.rs` — the custom PipeWire/PulseAudio integration preserved
  by this decision.
- `src/audio/device_manager.rs` — `cpal::default_host()` / `cpal::available_hosts()`
  enumeration on the ALSA-compatibility path.
- `Cargo.toml` — `cpal = "0.18.2"` with the `pipewire` feature left disabled.
- `benches/COVERCACHE_LRU_DECISION.md` — the record format this follows.
