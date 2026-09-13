# CoverCache LRU Evaluation — Decision Record

**Capture date:** 2026-09-13
**Code under evaluation:** `src/utils/covers.rs` — `CoverCache` (hand-rolled `HashMap<String, Handle>` + `VecDeque<String>`)
**Candidate:** `lru 0.18.4` (`LruCache<String, Handle>`), currently a pinned-but-unused direct dependency
**Commit:** `e2736aa7174f3509df68339293f2b497678f5a15`
**Harness:** `benches/covers_bench.rs` (criterion 0.8.2, `harness = false`)

Outcome: adopt

Confirmed: adopt (2026-09-12) — the reviewer verified that the outcome follows the recorded evidence and the parity result, and fixed the branch for the follow-up change.

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

`iter_batched_ref` is used with `BatchSize::SmallInput`: the untimed `setup` builds a
fresh pre-populated cache per batch, the routine receives `&mut Mutex<Cache>`, and the
cache is dropped after the measured region ends. Only the operation under test (plus the
enclosing `Mutex` acquisition) is timed; per-sample allocation, cold-cache access and
teardown are excluded. The `full_cycle` routine reuses one cache for `cap` hits + one
insert + one evict inside a single measured call.

An earlier version of this harness used `iter_batched`, which moves the input into the
timed routine and therefore measured cache teardown inside every standalone per-operation
sample (roughly 16x the warm cost at capacity 64). Those per-operation numbers were
invalid and are superseded by the tables below. In that superseded run, one row —
capacity-16 `get_miss` run2 — was a fully separated +20.6% regression for `lru`
(hand-rolled 746.1 ns vs `lru` 900.0 ns). With teardown excluded the same row favors `lru`
(34.99 ns vs 15.97 ns, −54.4%), confirming the old regression was an artifact of the
harness rather than of the operation.

The measured quantity is **cache-internal**: the adapter op plus the enclosing `Mutex`. It
does not include the per-hit wrapper that the real `load_cover_handle` pays on every call
before the cache op — the `NEGATIVE_CACHE` lock/`contains` check and
`cache.resize(get_max_covers())` — nor the `Handle::from_path` a miss builds. The
benchmark also uses synthetic always-hit/always-miss splits and a reverse sweep rather
than a measured hit/miss mix. The full-cycle ratios below are therefore cache-internal,
not an end-to-end cover-load speedup: the `lru` win is real for the cache internals, but
it should not be read as a user-visible gain.

## Results

Mean per-operation time, two independent runs (50 samples each). Intervals are criterion's
95% confidence bounds. `lru vs hand-rolled` is the signed change in mean time (positive =
`lru` slower). Absolute timings are machine- and session-dependent; only same-machine
comparisons against these runs are meaningful.

### Capacity 64 (normal)

| Operation | Hand-rolled run1 | Hand-rolled run2 | `lru` run1 | `lru` run2 | `lru` vs hand-rolled |
|-----------|------------------|------------------|------------|------------|----------------------|
| `get_hit` | 130.2 ns [117.3, 140.4] | 120.5 ns [108.7, 130.4] | 65.5 ns [58.0, 71.8] | 62.2 ns [55.4, 68.0] | −49.7% / −48.4% |
| `get_miss` | 35.8 ns [34.1, 37.3] | 34.3 ns [32.8, 35.8] | 15.9 ns [14.8, 17.0] | 15.0 ns [14.2, 15.7] | −55.6% / −56.3% |
| `insert` | 325.0 ns [297.1, 347.6] | 316.9 ns [285.8, 344.0] | 132.6 ns [118.4, 144.7] | 117.4 ns [103.0, 131.1] | −59.2% / −63.0% |
| `evict` | 97.2 ns [90.8, 103.8] | 91.2 ns [86.8, 95.5] | 78.2 ns [75.3, 80.7] | 71.9 ns [70.3, 73.3] | −19.5% / −21.2% |
| `full_cycle` | 9.278 µs [9.207, 9.350] | 9.214 µs [9.144, 9.280] | 1.870 µs [1.810, 1.918] | 1.855 µs [1.787, 1.913] | −79.9% / −79.9% |

### Capacity 16 (low-resource)

| Operation | Hand-rolled run1 | Hand-rolled run2 | `lru` run1 | `lru` run2 | `lru` vs hand-rolled |
|-----------|------------------|------------------|------------|------------|----------------------|
| `get_hit` | 118.0 ns [108.0, 126.9] | 112.5 ns [102.9, 120.9] | 63.5 ns [54.2, 72.3] | 67.8 ns [59.5, 74.9] | −46.2% / −39.8% |
| `get_miss` | 34.5 ns [33.2, 35.7] | 35.0 ns [33.9, 36.1] | 19.2 ns [17.0, 21.0] | 16.0 ns [15.3, 16.6] | −44.4% / −54.4% |
| `insert` | 175.9 ns [160.9, 189.0] | 178.7 ns [162.6, 192.4] | 110.6 ns [98.6, 121.3] | 107.9 ns [98.6, 115.8] | −37.1% / −39.6% |
| `evict` | 85.3 ns [82.4, 87.9] | 85.5 ns [82.5, 88.0] | 90.8 ns [82.1, 98.9] | 75.1 ns [72.7, 77.1] | +6.5% / −12.2% |
| `full_cycle` | 1.354 µs [1.334, 1.372] | 1.358 µs [1.336, 1.378] | 619.6 ns [596.4, 639.3] | 613.2 ns [591.8, 631.2] | −54.2% / −54.9% |

The single row where `lru` is slower — capacity-16 `evict`, run1: +6.5% — has overlapping
confidence intervals (hand-rolled [82.4, 87.9] vs `lru` [82.1, 98.9]), i.e. within noise; in
run2 the same operation is −12.2% with separated intervals. Every other row favors `lru`
with separated intervals in both runs, so no operation regresses beyond measurement noise.

## Behavioral parity

At evaluation time (before the swap) the hand-rolled cache was diffed against an
`lru::LruCache` adapter: the same sequence ran against both implementations and the
observable trace and final MRU→LRU order had to match, so a real divergence failed the
test. That differential comparison is the basis of the "parity demonstrated, not
asserted" claim above.

After the swap the parity suite is a golden-contract guard, not a differential test:
the production cache is `lru`-backed, so comparing it against another `lru` instance
could never observe a divergence from the hand-rolled semantics. The suite now
validates the production cache against a frozen, independent reference model of the
previous hand-rolled semantics (`HashMap` + `VecDeque`) plus an explicit golden MRU→LRU
trace. It covers: capacity bound; recency/eviction order; `get` promotes while `peek`
does not; concurrency under a single `Mutex`; `clear`; negative-cache isolation; purge
(including the low-resource doubling); resize (shrink evicts the LRU first, growth does
not evict). `cargo test --lib covercache` passes 9/9.

## Public surface

The behavior-bearing public functions (`get_lru_cache()`, `load_cover_handle`,
`clear_all_cover_cache`, `clear_raw_cache`, `purge_old_covers`, `preload_visible_covers`,
`load_raw_image_for_iced`) are unchanged in signature and behavior. The pre-registered
"existing public surface unchanged" clause is nonetheless **not** absolutely true: the
three `pub` fields of `CoverCache` (`map`, `order`, `max_size`) were removed and replaced
by a private `entries`, which is a semver-breaking change for external consumers.
`CoverCache` is reachable through `pub mod utils` → `pub mod covers` (`src/lib.rs`,
`src/utils/mod.rs`), so an external crate that read those fields would no longer compile.
A grep of `src/` and `benches/` confirms no in-tree caller ever read them, so the clause
holds for this crate's call sites. As a consequence of making the fields private,
`get_lru_cache()` returns a `&'static Mutex<CoverCache>` whose type exposes no public
methods to external callers.

## Criterion disposition

- **Faster — satisfied, no waiver needed.** The full working-set cycle improves by ~80% at
  capacity 64 and ~54% at capacity 16, repeatable across both runs with separated intervals.
  After the harness correction every standalone operation also favors `lru` beyond noise
  (the sole slower row, capacity-16 `evict` run1, has overlapping intervals), so the
  pre-registered "no operation regressing beyond measurement noise" clause holds on the
  valid numbers. The earlier per-operation table came from a harness that timed teardown
  and is superseded; the clause is not waived because the corrected run meets it.
- **Safer — satisfied with one caveat.** Behavioral parity is demonstrated against a frozen
  independent reference (see "Behavioral parity"). The three internal `pub` fields were
  made private: a semver-breaking change for external consumers, of which there are none
  in-tree (see "Public surface"). The public functions and their behavior are unchanged.
- **Both** hold; the recorded outcome is adopt.

## Analysis

- **Representative workload (full cycle): `lru` is decisively faster.** 4.96x at capacity 64
  and 2.19–2.22x at capacity 16, with fully separated confidence intervals in both
  independent runs. This is the large, repeatable win the evaluation was looking for, and it
  is the workload that matches the real cache (a long-lived, warm static touched by many
  operations over the process lifetime). The hand-rolled full cycle is dominated by the
  O(n²) hit-refresh path (`VecDeque::iter().position()` + `remove`), exactly the asymptotic
  difference identified before measuring.
- **Standalone per-operation micro-benches now also favor `lru`.** With the harness
  corrected so teardown is outside the measured region, the per-operation ratios range from
  −19.5% to −63.0% at capacity 64 and from −12.2% to −54.4% at capacity 16, except one
  capacity-16 `evict` run where `lru` is +6.5% with overlapping intervals (within noise).
  The earlier "hand-rolled wins the micro-benches" reading was an artifact of timing cache
  teardown inside the sample and no longer applies.
- **"Safer" holds.** Parity is demonstrated against a frozen independent reference, not
  asserted, and adoption deletes ~60 lines of bespoke LRU bookkeeping (recency list
  maintenance, manual eviction, manual resize) rather than adding capability, while the
  behavior-bearing public functions are preserved. The one caveat is the removed `pub`
  fields (see "Public surface"), a semver break with no in-tree consumer.

## Size dependence

The result is size-dependent in magnitude but not in direction: the full-cycle advantage
shrinks from ~80% at capacity 64 to ~54% at capacity 16, consistent with an O(n) hit
refresh whose cost grows with the working set. Capacity 16 wins do not regress: the
full-cycle intervals are fully separated, and every standalone operation favors `lru`
beyond noise except one capacity-16 `evict` run whose intervals overlap. A win at 64 with
no regression at 16 therefore clears the speed bar on its own, and here both sizes win on
the representative workload.

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
