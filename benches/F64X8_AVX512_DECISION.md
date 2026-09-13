# wide::f64x8 / AVX-512 DSP Path Evaluation — Decision Record

**Capture date:** 2026-09-13
**Code under evaluation:** `src/audio/dsp.rs` — the SIMD DSP path, which uses
`wide::f64x4` for the biquad/EQ bands and the compressor RMS/peak accumulator, with a
scalar tail; it is the unconditional default on all hardware
**Candidate:** a `wide::f64x8` / AVX-512 variant of the same biquad and compressor paths
**Commit:** `2484b08` (record captured against the current tree; no code change)
**Reference host:** `tix-slx` (see Machine / OS)

Outcome: not implemented

## Question

Can a `wide::f64x8` AVX-512 DSP path be added so that it is selected at runtime only on
hardware that supports AVX-512, while the existing `f64x4`/scalar path remains the portable
fallback — and does that path deliver a measurable benefit here? Implement it only if the
ISA can be selected at runtime and the return on investment is demonstrably positive;
otherwise leave the portable default unchanged and record why.

## Pre-registered decision criteria

Recorded before the evaluation was concluded:

- **Runtime selectability:** the AVX-512 path must be selectable in a single portable
  binary at runtime, falling back to the existing path on CPUs without AVX-512. A path that
  can only be reached by making the whole binary AVX-512-only does not qualify.
- **Measurable benefit:** a real AVX-512 gain must be demonstrable on the reference host.
  A no-gain result measured without AVX-512 hardware does not qualify, and extrapolated
  performance is not accepted as evidence.
- **Portability preserved:** the default build must keep running on CPUs without AVX-512;
  unconditionally emitting AVX-512 instructions (which can raise `SIGILL` on other CPUs)
  is rejected.
- **All must hold to implement.** If any fails, the path is intentionally not implemented
  and the portable `f64x4`/scalar default stays unchanged.

## Machine / OS

| Field | Value |
|-------|-------|
| Host | `tix-slx` |
| OS | Soplos Linux Tyron (kernel `7.2.5-soplos-v3`, x86_64) |
| CPU | AMD Ryzen 7 5800X 8-Core Processor (16 hardware threads, up to ~4.85 GHz); AVX2 present, **no AVX-512** |
| Memory | 31 GiB installed |

## Toolchain

```
rustc 1.98.1 (48a229cea 2026-09-01)
cargo 1.98.1 (797e8a9bc 2026-08-05)
```

Build configuration: `.cargo/config.toml` sets no `RUSTFLAGS`, `target-feature`, or
`target-cpu`, so the binary targets the x86-64 baseline.

## Evidence

### `wide` selects its instruction set at compile time

`wide` resolves the representation of `f64x8` when the `wide` crate itself is compiled, via
a `cfg(target_feature = "avx512f")` gate: with the feature the type wraps an AVX-512
register, without it the type falls back to two `f64x4` values. The same pattern is used for
`f64x4` with the `avx` feature. This is a **compile-time** decision inside `wide`, and it is
not re-evaluated when a downstream crate is compiled.

Consequences verified from the vendored source:

- `is_x86_feature_detected!("avx512f")` placed around a `wide::f64x8` call does **not**
  switch `wide`'s implementation — the runtime check is inert with respect to instruction
  selection.
- The `multiversion` crate, the usual workaround for runtime ISA selection, explicitly does
  not work with `wide`.
- A `#[target_feature(enable = "avx512f")]` attribute on a caller does not re-run `wide`'s
  internal `cfg` either, because `wide` is compiled once and inlining does not change which
  branch its code was built from.

In other words, the candidate cannot be runtime-selected through `wide`, so a single
portable binary cannot contain both `wide`'s AVX-512 `f64x8` branch and a non-AVX-512
fallback chosen at runtime.

### The reference host has no AVX-512

The reference CPU is an AMD Ryzen 7 5800X (Zen 3) with AVX2 but **no AVX-512**; this is
confirmed by `/proc/cpuinfo`, which lists `avx2` and no `avx512*` flags. The current build
also has no target features beyond the baseline (`rustc --print cfg` reports only
`fxsr`, `sse`, `sse2`). Therefore:

- Today's `wide::f64x4` is already SSE2-class (two `f64x2`), not AVX.
- A `wide::f64x8` call compiled in this configuration is SSE2-class too (four `f64x2`) —
  bit-for-bit the same instruction class as two `f64x4` operations.
- There is **no way to measure a real AVX-512 gain** on this host; a benchmark here can only
  confirm a no-gain.

### The only route to the AVX-512 branch is non-portable

The only way to obtain `wide`'s real AVX-512 implementation is a whole-build flag such as
`-C target-feature=+avx512f` (or `target-cpu=native` on an AVX-512 host). That makes the
entire binary AVX-512-only: any code path may emit AVX-512 instructions without a runtime
guard, and the program can raise `SIGILL` on CPUs that do not support AVX-512. This is
exactly the unconditional-AVX-512 outcome the project rejects, and such a flag must never be
added to the build configuration.

## Criterion disposition

- **Runtime selectability — failed.** `wide` cannot runtime-select its `f64x8`
  implementation; the ISA is fixed when the crate is compiled, and the runtime-detection
  crates do not work with it. The candidate cannot be enabled conditionally in one portable
  binary.
- **Measurable benefit — not obtainable here.** The reference host has no AVX-512 and the
  default build emits none, so any measurement would compare a fallback against a fallback,
  not a genuine AVX-512 path.
- **Portability preserved — only by not implementing it.** The sole mechanism that reaches
  `wide`'s AVX-512 branch is a whole-build target feature that gives up portability and can
  `SIGILL` on other CPUs.
- **All must hold — none holds**, so the path is intentionally not implemented.

## Return on investment

On the reference host the candidate yields no measurable benefit, and on the broader
deployed hardware base it is likely poor or negative: AVX-512 is absent from most consumer
CPUs of this class, and reaching `wide`'s AVX-512 branch requires sacrificing the portability
of the entire binary. The verified mechanism also makes the originally desired runtime
selection infeasible with `wide`. The effort would add a feature flag and a second SIMD
branch whose benefits cannot be validated here, while increasing the risk of an accidental
non-portable build. That assessment is why the path is recorded as **not implemented**
rather than deferred behind an opt-in feature.

## Outcome and its consequences

- The portable `wide::f64x4` / scalar DSP path in `src/audio/dsp.rs` is unchanged and
  remains the unconditional default on all hardware.
- No `avx512` cargo feature is added and no feature-gated `f64x8` branch is introduced.
- No AVX-512 code and no AVX-512 build flag are introduced; `.cargo/config.toml` keeps no
  `target-feature`/`target-cpu` pinning, so builds stay portable.
- There is nothing to configure and no runtime probe to add: the deterministic default is
  the only code path.

## Caveats

- This record is documentation-only: no source file, `Cargo.toml`, or `.cargo/config.toml`
  change accompanies it.
- The assessment is specific to `wide` as the SIMD abstraction. A genuinely runtime-selected
  AVX-512 path would require platform intrinsics behind a feature-detected guard and is a
  larger, `unsafe`-bearing change that cannot be validated on this host; it was not pursued.
- Criterion baselines remain advisory, so even a mixed SSE2-vs-AVX-512 benchmark would be a
  prompt to investigate rather than a gate — but no such benchmark is meaningful without
  AVX-512 hardware.

## References

- `src/audio/dsp.rs` — the `wide::f64x4` biquad and compressor SIMD path that stays the
  portable default.
- `Cargo.toml` — `wide = "1.7.0"`; no `avx512` feature exists.
- `.cargo/config.toml` — no `target-feature` / `target-cpu` pinning.
- `benches/COVERCACHE_LRU_DECISION.md` — the record format this follows.
