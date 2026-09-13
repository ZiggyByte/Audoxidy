# Benchmark Baseline `post-audio-core`

**Capture date:** 2026-09-13
**Commit:** `aec143955ae4708563c027f9798c44f7ac26053e`
**Purpose:** Authoritative post-audio-core reference for the DSP-chain, engine-mix, and
utility benchmarks. It reflects the code after the symphonia/rubato/cpal/ringbuf/audio-core
migration, replacing the earlier `pre-audio-core` numbers.

Comparison against this baseline is **advisory**. Investigate a sustained **>10–15%**
regression for the affected benchmark; do **not** treat the threshold as a blocking gate.
Absolute timings depend on host CPU, kernel, load, thermals, and frequency scaling, so only
same-machine comparisons are meaningful.

## Machine / OS

| Field | Value |
|-------|-------|
| Host | `tix-slx` |
| OS | Soplos Linux Tyron (kernel `7.2.5-soplos-v3`, x86_64) |
| CPU | AMD Ryzen 7 5800X 8-Core Processor (8 cores / 16 threads, up to ~4.85 GHz) |
| Memory | 31 GiB installed |

## Toolchain

```
rustc 1.98.1 (48a229cea 2026-09-01)
cargo 1.98.1 (797e8a9bc 2026-08-05)
```

Bench profile: `[profile.bench]` inherits `release` with `lto = "fat"`,
`codegen-units = 1`, `debug = true`, `strip = "none"`. No `target-cpu`/`RUSTFLAGS`
pinning is configured (`.cargo/config.toml`), so results depend on the host CPU.

## Capture Command

The nominal command is `cargo bench -- --save-baseline post-audio-core`, but with this
package's `[lib]` target enabled for benchmarks, plain `cargo bench` also runs the lib
test harness, which rejects criterion's `--save-baseline` argument
(`error: Unrecognized option: 'save-baseline'`). The three `[[bench]]` targets must be
selected explicitly:

```bash
cargo bench --bench dsp_bench --bench engine_bench --bench utils_bench -- --save-baseline post-audio-core
```

This runs all three benchmark targets with no per-benchmark filter and no skipped target.
Criterion stores the named baseline under
`target/criterion/<group>/<bench>/post-audio-core/` (ungrouped benches like
`audio_state_read` store under `target/criterion/<bench>/post-audio-core/`).

Future comparison command:

```bash
cargo bench --bench dsp_bench --bench engine_bench --bench utils_bench -- --baseline post-audio-core
```

## Per-Benchmark Results

Mean is criterion's point estimate; the bracketed values are the confidence interval.

### DSP chain (`benches/dsp_bench.rs`)

| Benchmark id | Mean | Throughput |
|--------------|------|------------|
| `dsp_chain_51_full/full` | 607.27 ns | 9.8802 Melem/s |
| `dsp_chain_disabled_bypass/bypass` | 1.9448 ns | 1.0284 Gelem/s |
| `eq_31band_stereo/31band` | 26.503 ns | 75.463 Melem/s |
| `biquad_peak/stereo` | 5.9859 ns | 334.12 Melem/s |
| `biquad_peak/4ch_simd_candidate` | 6.9947 ns | 571.86 Melem/s |
| `reverb_stereo_frame/stereo` | 74.229 ns | 26.944 Melem/s |
| `compressor_above_threshold/above_threshold` | 330.98 ns | 6.0427 Melem/s |
| `stereo_expander_width_150/width_150` | 58.777 ns | 34.027 Melem/s |

### Engine mix (`benches/engine_bench.rs`)

| Benchmark id | Mean | Throughput |
|--------------|------|------------|
| `mix_planar/stereo_to_stereo_512frames` | 1.8224 µs | 561.91 Melem/s |
| `mix_planar/51_to_stereo_512frames` | 2.0724 µs | 494.11 Melem/s |
| `channel_map_construction/4configs` | 15.175 ns | 263.60 Melem/s |
| `audio_state_read` | 4.2601 ns | — (no throughput element set) |

### Utilities (`benches/utils_bench.rs`)

| Benchmark id | Mean | Throughput |
|--------------|------|------------|
| `truncate_text/long` | 140.58 ns | 7.1135 Melem/s |
| `truncate_text/short` | 9.7903 ns | 102.14 Melem/s |
| `format_duration/various` | 279.11 ns | 14.331 Melem/s |
| `format_size/various` | 377.82 ns | 10.587 Melem/s |
| `compare_track_numbers/various` | 63.895 ns | 62.602 Melem/s |
| `intelligent_path/deep` | 319.07 ns | 3.1341 Melem/s |

## Throughput Unit Legend

Throughput is reported by criterion as elements per second (`Melem/s` = million elements/s,
`Gelem/s` = billion elements/s). The element counted per benchmark iteration:

- Per-frame DSP benches (`dsp_chain_*`, `eq_31band_stereo`, `biquad_peak`, `reverb_stereo_frame`,
  `compressor_above_threshold`, `stereo_expander_width_150`): one element per input sample
  in the frame (6 for the 5.1 frame, 2 for stereo, 4 for the 4-channel biquad).
- `mix_planar/*`: output samples per iteration (`frames * out_channels` = 512 × 2 = 1024).
- `channel_map_construction/4configs`: 4 channel configurations per iteration.
- Utility benches: number of calls per iteration (`truncate_text` = 1, `format_duration` /
  `format_size` / `compare_track_numbers` = 4, `intelligent_path` = 1).
- `audio_state_read`: no throughput element (a state-read microbench).

## Caveats

- Criterion 0.8 randomizes memory layout per run and these numbers are machine- and
  session-dependent (host CPU, kernel, load, thermals, and frequency scaling all matter).
  **Only same-machine comparisons against this named `post-audio-core` baseline are
  meaningful.** Do not compare these numbers across machines.
- The comparison is advisory: a >10–15% delta is a prompt to investigate, not a build
  failure. There is no CI or automated gate consuming these numbers.
- `post-audio-core` is the current authoritative baseline; it supersedes the earlier
  `pre-audio-core` record, whose engine-mix numbers no longer describe the shipped code.
- Baseline data lives in `target/criterion/**/post-audio-core/` (gitignored, not committed);
  this document is the version-controlled record.
