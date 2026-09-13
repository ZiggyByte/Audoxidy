# Resampler Evaluation (sinc vs rubato FFT) — Decision Record

**Capture date:** 2026-09-13
**Code under evaluation:** `src/audio/decoder.rs` — the three `Async::<f64>::new_sinc`
construction sites (primary resampler, crossfade preload resampler, crossfade tail
resampler)
**Candidate:** `rubato 5.0.0` `Fft<f64>` (`Fft::<f64>::new(.., FixedSync::Input)`), no
dependency change — the `fft_resampler` feature and `realfft`/`num-complex` are already
resolved in `Cargo.lock`
**Commit:** `5668a39adb5f53751de5eff3d5fef989dc7786e9`
**Harnesses:** `src/audio/dsp_tests.rs` (frequency response) and `benches/resampler_bench.rs`
(criterion 0.8.2, `harness = false`)

Outcome: keep sinc

## Question

Does the `rubato` FFT resampler beat the current sinc interpolator across the
representative rate pairs (48 k→44.1 k, 44.1 k→48 k, 96 k→48 k) on both frequency response
and CPU cost, such that it can be adopted across all three decoder resampler sites? Adopt
only when all pre-registered criteria hold; otherwise record the measured numbers and keep
sinc as the default.

## Pre-registered decision criteria

Recorded before the numbers were read:

- **Quality non-regression:** the adopted configuration's frequency response must not
  regress and the applicable anti-aliasing assertions must stay green. The existing sinc
  gate (`passband > 0.4`, `rolloff ∈ 0.38..=0.49`, `stopband < 0.05` at 48 k→44.1 k) is not
  weakened or deleted for the reference path.
- **f64 pipeline preserved:** no new f32 narrowing; the single `as f32` narrowing stays at
  the ring-buffer push.
- **CPU not worse:** the FFT must not be slower than sinc on any measured rate pair.
- **Crossfade timing preserved:** the preload/tail resampler continuation must still hold,
  including the FFT's added half-block delay.
- **All four must hold to adopt.** Any quality regression or capability loss keeps sinc as
  the default and leaves `src/audio/decoder.rs` unchanged.

## Machine / OS

| Field | Value |
|-------|-------|
| Host | `tix-slx` |
| OS | Soplos Linux Tyron (kernel `7.2.5-soplos-v3`, x86_64) |
| CPU | AMD Ryzen 7 5800X 8-Core Processor (16 hardware threads, up to ~4.85 GHz) |
| Memory | 31 GiB installed |

## Toolchain

```
rustc 1.98.1 (48a229cea 2026-09-01)
cargo 1.98.1 (797e8a9bc 2026-08-05)
```

Bench profile: `[profile.bench]` inherits `release` with `lto = "fat"`,
`codegen-units = 1`, `debug = true`, `strip = "none"`. No `target-cpu`/`RUSTFLAGS` pinning
(`.cargo/config.toml`), so results depend on the host CPU.

## Capture commands

Frequency response:

```bash
cargo test --lib -j 2 -- test_resampler_antialiasing --nocapture
```

CPU:

```bash
cargo check --benches -j 2
cargo bench --bench resampler_bench -j 2 -- --warm-up-time 1 --measurement-time 3
```

Both ran to completion on an otherwise idle machine. The test prints a steady-state peak per
band for both resamplers; the bench constructs each resampler once and processes 1024-frame
stereo sine chunks, reporting throughput in input frames/s. `-j 2` bounds compile memory and
does not affect the measured workload.

## Methodology and its limits

Both resamplers are driven with the same chunk size (1024), stereo channel count, 1 kHz
sine input, 20-chunk warmup and 20-chunk measurement horizon in the frequency-response test,
and the same criterion warmup/measurement/sample counts in the bench. The sinc reference is
the audiophile profile the decoder uses (`sinc_len: 256`, `f_cutoff: Some(0.99)`,
`Cubic`, `oversampling_factor: 256`, `BlackmanHarris2`). The FFT candidate is the plain
`Fft::<f64>::new(.., FixedSync::Input)`, which targets an internal sub-chunk of roughly 256
frames and selects the `BlackmanHarris2` window.

The frequency-response numbers are RMS-free steady-state peaks of a single tone per band; the
CPU numbers are per-chunk wall times that include the adapter construction and the per-chunk
output buffer reuse. Absolute timings are machine- and session-dependent; only same-machine
comparisons are meaningful. The decoder's real workload also includes Symphonia decode, DSP
and ring-buffer pushes, so the bench ratios are resampler-internal, not an end-to-end
playback speedup.

## Results

### Frequency response (steady-state peak per band)

Tones are given in Hz. The rolloff and stopband probes are the same ones the sinc gate uses
at 48 k→44.1 k (21.5 kHz just below the 22.05 kHz output Nyquist, 23 kHz above it). The
upsample and hi-res rows use probes that are meaningful for their pair.

| Rate pair | Band | sinc | FFT |
|-----------|------|------|-----|
| 48 k→44.1 k | passband (1 kHz) | 0.499998 | 0.499997 |
| 48 k→44.1 k | rolloff (21.5 kHz) | 0.436861 | 0.009777 |
| 48 k→44.1 k | stopband (23 kHz) | 0.000001 | 0.000000 |
| 44.1 k→48 k | passband (1 kHz) | 0.499574 | 0.500000 |
| 44.1 k→48 k | rolloff (15 kHz) | 0.493130 | 0.500000 |
| 44.1 k→48 k | stopband (20 kHz) | 0.487808 | 0.500000 |
| 96 k→48 k | passband (1 kHz) | 0.500000 | 0.500000 |
| 96 k→48 k | rolloff (23 kHz) | 0.453500 | 0.001530 |
| 96 k→48 k | stopband (30 kHz) | 0.000000 | 0.000000 |

The candidate preserves everything the sinc path preserves at the low and mid bands, but its
anti-aliasing cutoff sits materially lower in the retained band: at 48 k→44.1 k the 21.5 kHz
tone (below the 22.05 kHz output Nyquist) drops from 0.4369 to 0.0098, and at 96 k→48 k the
23 kHz tone (below the 24 kHz output Nyquist) drops from 0.4535 to 0.0015. The 48 k→44.1 k
rolloff value fails the existing sinc gate's lower bound of 0.38 by a wide margin, so the
applicable anti-aliasing assertion is not green for the candidate.

### CPU cost (criterion, mean and 95% CI)

Per 1024-frame stereo chunk, two quantities per benchmark: wall time and throughput in input
frames/s. `FFT vs sinc` is the signed change in mean time (negative = FFT faster).

| Benchmark | sinc | FFT | FFT vs sinc |
|-----------|------|-----|-------------|
| `48k_to_44k1` | 159.06 µs [158.88, 159.31] | 11.695 µs [11.651, 11.749] | −92.6% (13.6×) |
| `44k1_to_48k` | 194.24 µs [193.52, 195.15] | 12.714 µs [12.689, 12.744] | −93.5% (15.3×) |
| `96k_to_48k` | 81.919 µs [81.852, 81.988] | 6.7126 µs [6.6941, 6.7326] | −91.8% (12.2×) |

Throughput mirrors the time: `resampler_sinc/48k_to_44k1` ≈ 6.44 Melem/s vs
`resampler_fft/48k_to_44k1` ≈ 87.6 Melem/s, and likewise for the other pairs. The candidate
is decisively faster on every pair, with fully separated confidence intervals.

## Criterion disposition

- **Quality non-regression — FAILED.** The FFT's default anti-aliasing cutoff attenuates the
  top of the retained band well before the Nyquist of the retained band: the 21.5 kHz tone at
  48 k→44.1 k falls from 0.4369 to 0.0098, below the existing gate's `0.38..=0.49` window,
  and the analogous 96 k→48 k probe falls from 0.4535 to 0.0015. The plan of record forbids
  weakening the sinc gate to accommodate the candidate, so the applicable anti-aliasing
  assertion is not green for the adopted configuration.
- **f64 pipeline preserved — not the deciding factor.** The candidate supports `f64`, so the
  pipeline could be preserved on adoption; this criterion alone would not block.
- **CPU not worse — satisfied.** The FFT is 12–15× faster on every measured pair.
- **Crossfade timing preserved — not reached.** A real crossfade-timing evaluation would
  require migrating all three sites (the preload/tail fields are `Option<Async<f64>>`, which
  would become a trait object) and re-running the continuation tests; because quality already
  fails, the migration was not performed and this criterion is untested rather than passed.
  The candidate also adds a half-FFT-block delay (half of `fft_size_out`, i.e. roughly
  64–160 output frames at these pairs, half of the 128–320-frame FFT output block), which
  the preload/promotion continuation would have to absorb.

One criterion fails and one is untested, so the pre-registered "all four must hold" condition
is not met; the recorded outcome is keep sinc.

## Structural caveat

The FFT resampler requires integer rate pairs (`sample_rate_input`/`sample_rate_output` are
`usize`) and introduces a delay equal to half its internal FFT block, queryable through
`Resampler::output_delay`; the sinc interpolator instead accepts an arbitrary `f64` ratio and
has no such fixed delay. The decoder currently only resamples when the input and output rates
differ and both are already `u32`, so integer pairs are not a blocker there — but the delay
and the three-site coupling would have to be managed for a swap. This trade-off is not the
reason for the outcome (the quality regression is), but it is a reason to keep the decision
explicit rather than a silent swap.

## Analysis

The evaluation was run with a genuine openness to adoption: the candidate is 12–15× faster
across every tested pair, which is a large and repeatable CPU win, and it needs no dependency
change. That alone would usually justify a swap. It does not here, because the win comes with
an anti-aliasing filter whose cutoff is lower than the sinc reference's explicit
`f_cutoff = 0.99`: in the retained band the FFT attenuates a 21.5 kHz tone by roughly 34 dB
relative to the passband where sinc attenuates it by roughly 1 dB. The pre-registered criteria
treat that as a quality regression, and the sinc gate exists precisely to catch a cutoff
change of this kind.

Perhaps the FFT's `new_custom` could trade delay for a higher cutoff and recover the upper
band, but that is outside the pre-registered candidate (`Fft::<f64>::new`), and the decoder's
crossfade continuation would still need a full three-site migration with an added delay.
Given a failing quality criterion, the correct outcome is to keep sinc, record the numbers,
and leave `src/audio/decoder.rs` unchanged.

## Caveats

- Criterion randomizes memory layout per run; these numbers are specific to the machine,
  kernel, load, thermals, and frequency scaling above. Only same-machine comparisons are
  meaningful.
- The bench measures resampler-internal throughput, not end-to-end playback; decode, DSP and
  ring-buffer work are excluded.
- No manifest, dependency, or production-resampler change is made by this record. The
  decoder keeps its three sinc construction sites.

## References

- `benches/resampler_bench.rs` — criterion harness that produced the CPU numbers.
- `src/audio/dsp_tests.rs` — the generalized sinc/FFT frequency-response comparison.
- `benches/BASELINES.md` — benchmark conventions and the sibling decision-record precedent.
- `benches/COVERCACHE_LRU_DECISION.md` — the record format this follows.
