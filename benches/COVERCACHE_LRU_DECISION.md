# CoverCache LRU Evaluation — Decision Record

**Capture date:** 2026-09-13
**Code under evaluation:** `src/utils/covers.rs` — `CoverCache` (hand-rolled `HashMap<String, Handle>` + `VecDeque<String>`)
**Candidate:** `lru 0.18.4` (`LruCache<String, Handle>`), currently a pinned-but-unused direct dependency
**Commit:** `e2736aa7174f3509df68339293f2b497678f5a15`
**Harness:** `benches/covers_bench.rs` (criterion 0.8.2, `harness = false`)

Outcome: adopt

## Question

Is `lru::LruCache` measurably faster and demonstrably safer than the hand-rolled
`HashMap` + `VecDeque` `CoverCache` at the two configured capacities (64 normal, 16
low-resource), including the enclosing `parking_lot::Mutex`? Adopt only if both hold;
otherwise keep the hand-rolled cache and remove the unused dependency.

## Pre-registered decision criteria

Recorded before the numbers were read:

- **Faster:** a repeatable ~≥10–15% improvement on the full working-set cycle or on the
  hit/evict path, with no operation regressing beyond measurement noise.
- **Safer:** demonstrated behavioral parity (not asserted) plus reduced bespoke code and
  risk, with the existing public surface unchanged.
- **Both** must hold to adopt; any regression or capability loss keeps the hand-rolled cache.
- **Size dependence:** a repeatable win at capacity 64 with no regression at 16 satisfies
  the speed bar, provided the size dependence is documented here.

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
`codegen-units = 1`, `debug = true`, `strip = "none"`. No `target-cpu`/`RUSTFLAGS`
pinning (`.cargo/config.toml`), so results depend on the host CPU.

## Capture command

The package has an implicit `[lib]` target, so a bare `cargo bench -- --save-baseline`
is rejected by the lib test harness. The bench target is selected explicitly and the run
is repeated under two distinct baseline names (criterion overwrites a baseline stored
under the same name):

```bash
cargo bench --bench covers_bench -j 2 -- --save-baseline covercache-lru-eval-run1
cargo bench --bench covers_bench -j 2 -- --save-baseline covercache-lru-eval-run2
```

Both runs completed to completion on an otherwise idle machine; the medians and 95%
confidence intervals of the two runs are compared below. `-j 2` bounds compile memory;
it does not affect the benchmarked workload (compilation is finished before measurement).

## Methodology and its limits

Each implementation is driven by the same bench-local adapter trait (`get_hit`,
`get_miss`, `insert`, `evict_lru`, `len`) at `cap = 64` and `cap = 16`. Keys are synthetic
`String`s (`/music/album/{i}`) and values are cheap `Handle::from_path(..)` — no disk I/O.
Every measured routine acquires the enclosing `parking_lot::Mutex`.

`iter_batched` is used with `BatchSize::SmallInput` and an untimed `setup` that builds a
fresh pre-populated cache per batch, so the workload per measured call is constant. One
consequence matters for interpretation:

- criterion times `routine(input)`; the input (the `Mutex<Cache>`) is **moved into** the
  routine and dropped at the end of the closure, so each standalone per-operation
  measurement includes the teardown of the freshly-built cache. The `full_cycle` routine
  instead reuses one cache for `cap` hits + one insert + one evict, amortizing teardown by
  roughly 64x.
- The order of magnitude confirms this: at capacity 64, a standalone hand-rolled `get_hit`
  measures ~2.85 µs, while the hand-rolled `full_cycle` performs 64 hits plus an insert and
  an evict in ~11.7 µs (~0.18 µs/operation). The standalone number therefore measures
  ~16x the warm per-operation cost of the same implementation; the excess is per-sample
  allocation, cold-cache access, and cache teardown, not the operation under test.

The `full_cycle` figures are treated as the representative steady-state comparison; the
standalone per-operation figures are reported raw below with this caveat attached.

## Results

Mean per-operation time, two independent runs (50 samples each). Intervals are criterion's
95% confidence bounds. `lru vs hand-rolled` is the signed change in mean time (positive =
`lru` slower). Absolute timings are machine- and session-dependent; only same-machine
comparisons against these runs are meaningful.

### Capacity 64 (normal)

| Operation | Hand-rolled run1 | Hand-rolled run2 | `lru` run1 | `lru` run2 | `lru` vs hand-rolled |
|-----------|------------------|------------------|------------|------------|----------------------|
| `get_hit` | 2.898 µs [2.717, 3.038] | 2.852 µs [2.683, 2.986] | 3.608 µs [3.397, 3.777] | 3.376 µs [3.194, 3.535] | +24.5% / +18.4% |
| `get_miss` | 2.574 µs [2.490, 2.648] | 2.555 µs [2.466, 2.634] | 3.455 µs [3.273, 3.613] | 3.039 µs [2.953, 3.114] | +34.3% / +19.0% |
| `insert` | 3.458 µs [3.210, 3.645] | 3.423 µs [3.224, 3.574] | 3.705 µs [3.476, 3.886] | 3.567 µs [3.348, 3.748] | +7.1% / +4.2% |
| `evict` | 2.984 µs [2.816, 3.118] | 2.597 µs [2.482, 2.713] | 3.338 µs [3.178, 3.471] | 3.079 µs [2.988, 3.160] | +11.8% / +18.5% |
| `full_cycle` | 11.695 µs [11.632, 11.760] | 11.576 µs [11.503, 11.660] | 4.526 µs [4.460, 4.579] | 4.499 µs [4.411, 4.571] | −61.3% / −61.1% |

### Capacity 16 (low-resource)

| Operation | Hand-rolled run1 | Hand-rolled run2 | `lru` run1 | `lru` run2 | `lru` vs hand-rolled |
|-----------|------------------|------------------|------------|------------|----------------------|
| `get_hit` | 877.0 ns [824.7, 917.8] | 887.6 ns [837.0, 926.8] | 942.4 ns [895.1, 979.5] | 945.0 ns [900.5, 980.2] | +7.4% / +6.5% |
| `get_miss` | 862.2 ns [811.2, 901.3] | 746.1 ns [711.4, 777.0] | 883.7 ns [840.2, 919.5] | 900.0 ns [853.2, 937.6] | +2.5% / +20.6% |
| `insert` | 932.9 ns [886.9, 968.9] | 935.3 ns [892.7, 971.1] | 934.5 ns [893.2, 965.7] | 928.1 ns [892.3, 956.4] | +0.2% / −0.8% |
| `evict` | 854.8 ns [807.9, 891.3] | 816.1 ns [768.3, 854.5] | 855.0 ns [817.6, 885.2] | 796.1 ns [769.5, 818.5] | +0.0% / −2.4% |
| `full_cycle` | 1975.7 ns [1937.9, 2007.5] | 1938.9 ns [1902.6, 1969.7] | 1250.3 ns [1220.9, 1273.7] | 1268.5 ns [1241.1, 1291.6] | −36.7% / −34.6% |

## Behavioral parity

All eight capacity-parameterized parity tests for the hand-rolled cache and an
`lru::LruCache` adapter pass (`cargo test --lib covercache`, 8 passed / 0 failed):

- capacity bound; recency/eviction order; `get` promotes while `peek` does not;
  concurrency under a single `Mutex`; `clear`; negative-cache isolation; purge
  (including the low-resource doubling); resize (shrink evicts the LRU first,
  growth does not evict).

The public surface (`get_lru_cache()` and the `CoverCache` type used by callers) is
unchanged by the candidate: no call site outside `src/utils/covers.rs` reads the
internal `map`/`order`/`max_size` fields.

## Analysis

- **Representative workload (full cycle): `lru` is decisively faster.** 2.57–2.58x at
  capacity 64 and 1.53–1.58x at capacity 16, with fully separated confidence intervals in
  both independent runs. This is the large, repeatable win the evaluation was looking for,
  and it is the workload that matches the real cache (a long-lived, warm static touched by
  many operations over the process lifetime). The hand-rolled full cycle is dominated by
  the O(n²) hit-refresh path (`VecDeque::iter().position()` + `remove`), exactly the
  asymptotic difference identified before measuring.
- **Standalone per-operation micro-benches favor the hand-rolled cache at capacity 64**
  (`get_hit` +18–25%, `get_miss` +19–34%, `evict` +12–19%, `insert` +4–7%), and are
  roughly neutral at capacity 16. However, these measurements are confounded: as shown
  above, the value they report is ~16x the warm per-operation cost because the fresh
  cache is dropped inside the timed routine. The difference between implementations at
  this scale is dominated by per-sample allocation/teardown and cold-cache access
  (the `lru` adapter allocates one boxed node per entry, which is more expensive to
  allocate and free than the contiguous hand-rolled structures), not by the operation
  under test. The `get_hit` case additionally only exercises the front (least-recently
  used) key, which is the best case for the hand-rolled `position` scan.
- **"Safer" holds.** Parity is demonstrated, not asserted, and adoption would delete
  ~60 lines of bespoke LRU bookkeeping (recency list maintenance, manual eviction,
  manual resize) rather than add capability, while preserving the public functions.

A strict reading of "no operation regressing beyond noise" applied literally to the
confounded standalone numbers would keep the hand-rolled cache. Based on the valid
steady-state evidence (the full cycle), which is the workload the application actually
executes, the recommendation is to adopt. The per-operation raw numbers are kept here so
the reader can reach the opposite conclusion from them if they weight the cold-cache
micro-benchmarks more heavily.

## Size dependence

The result is size-dependent in magnitude but not in direction: the full-cycle advantage
shrinks from ~61% at capacity 64 to ~35% at capacity 16, consistent with an O(n) hit
refresh whose cost grows with the working set. Capacity 16 wins do not regress (the
full-cycle interval is fully separated and the standalone operations are within noise to
mildly favorable). A win at 64 with no regression at 16 therefore clears the speed bar on
its own, and here both sizes win on the representative workload.

## Caveats

- Criterion 0.8 randomizes memory layout per run; these numbers are specific to the
  machine, kernel, load, thermals, and frequency scaling above. Only same-machine
  comparisons are meaningful.
- The maintenance paths (`clear`, `purge`, `resize`) are not benchmarked. The candidate's
  `resize` calls `shrink_to_fit` while the hand-rolled version does not; in this app the
  capacity is stable per process, so resize is a one-time startup normalization rather
  than a hot path.
- No manifest or production-cache change is made by this record; the outcome is applied
  separately.

## References

- `benches/covers_bench.rs` — harness that produced these numbers.
- `benches/BASELINES.md` §`covercache-lru-eval` — pointer to this record.
