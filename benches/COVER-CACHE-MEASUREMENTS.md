# Cover Cache — Measurement Report

**Date:** 2026-11-13
**Status:** final run passed the scroll-back criterion

This report records the real-library measurements of the cover cache: the method,
every run (including the failing ones), the defects the failures exposed, and the
final numbers. It is the behavioural counterpart to `covers_bench.rs`.

---

## Environment

| Item | Value |
|---|---|
| Albums | 676 |
| Cover files | 698 (676 album covers + 22 per-song overrides) |
| Songs | 8,067 |
| Window | 1920×1080, display scale 1.0 |
| Grid | 7 columns × 4 rows, 220 px decode floor |
| Build | dev profile inside `dev-audoxidy`, in mode `--debug` |
| OS | Linux |
| Track changes during P0–P3 | none |

### Cache policy under test

- Decode bucket: `max(220, rendered_size × scale)`, capped at the native 400 px.
- Player: 400 px path handle (unchanged); thumbnails: 32 px.
- Budget: `clamp(3 × viewport_capacity × bucket² × 4, 8 MiB, 48 MiB)`.
- At 1080p / 220 px: **16,262,400 B = 15.5 MiB = 84 entries**
  (35 visible window + 21 prefetch band + 28 scroll-back cushion).

---

## Method

1. **Reset:** stop the app, delete `library.db` (+ WAL/SHM) and `cache/covers/`,
   start the app, let the scan finish. There is no migration; the library and the
   cache are regenerated.
2. **Points:** P0 = settled with no scroll; P1 = scrolled to the bottom; P2 = half a
   viewport up and back (inside the cushion); P3 = full return to the top.
3. **Counters:** the GUI tick logs
   `audoxidy::covers: cover cache counters ...` at most every 10 s with
   `cover_file_opens`, `cover_decodes`, `cover_cache_hits`, `cover_cache_misses`,
   `cover_key_lookups`, `cover_evictions`, `cover_cache_bytes`,
   `cover_work_in_flight`, `cover_scale`, `cover_bucket`.
4. **Memory:** `Rss`, `Pss`, `Pss_Anon` and `Private_Dirty` from
   `/proc/$PID/smaps_rollup`.
5. **Drain rule:** before a point, wait until the counters are identical in two
   readings 10 s apart. `cover_work_in_flight` only counts active workers, not
   queued jobs, so stability — not the field — is the drain signal.
6. **In-cushion gesture:** one wheel notch (≈3 rows) inside the four-row cushion. A
   multi-notch or inertial scroll leaves the cushion and is not the P2 criterion.
7. `strace` was optional; the final run used the app counters only.

### Pass criteria

- P2 adds **0 file opens and 0 decodes**.
- `cover_cache_bytes` stays at or below the budget and does not grow.
- `Pss_Anon` after the full pass stays inside the budget delta over P0.
- The full pass is bounded (the library exceeds the retained set, so re-decoding
  during a full traversal is expected).

---

## Results

Counters are `opens / decodes / hits / misses / lookups / evictions / cache_bytes`.
Memory is `Pss_Anon` in kB.

### Run 1 — 467 albums, 489 covers, 6,088 songs: FAILED

| Point | Counters | Pss_Anon |
|---|---|---|
| P0 | 3,928 / 141 / 3,287 / 1,573 / 4,860 / 46 / 4,300,800 | 202,588 |
| P1 | 5,255 / 1,468 / 53,103 / 6,111 / 59,214 / 988 / 3,993,600 | 206,960 |
| P2 | 9,339 / 5,550 / 271,325 / 6,989 / 278,314 / 4,753 / 8,294,400 | 227,872 |
| P3 | 13,763 / 9,976 / 509,013 / 10,872 / 519,885 / 9,029 / 8,294,400 | 230,608 |

9,976 decodes for 489 covers ≈ **20 decodes per cover**, and P2 added 4,084 opens
where zero was required. Three defects were found:

- completed decodes were discarded when the scroll epoch advanced;
- the collection pass removed everything outside the current window, deleting the
  cushion;
- every insertion invalidated the grid row memo, amplifying the lookup count.

Fixed in `851b616`, `e52ec85`, `841680f`.

### Run 2 — 676 albums, 698 covers, 8,067 songs: FAILED (protocol + scan churn)

| Point | Counters | Pss_Anon |
|---|---|---|
| P0 | 12,405 / 87 / 2,330 / 1,406 / 3,736 / 2 / 8,372,224 | 232,416 |
| P1 | 14,147 / 1,829 / 26,492 / 1,618 / 28,110 / 1,748 / 8,294,400 | 254,596 |
| P2 | 15,872 / 3,554 / 26,828 / 1,618 / 28,446 / 3,473 / 8,294,400 | 254,992 |
| P3 | 17,122 / 4,804 / 40,506 / 18,778 / 59,284 / 4,723 / 8,294,400 | 256,520 |

P2 was contaminated by the tail of P1's queue (0 new render misses, only +336
lookups), and P0 showed **12,405 opens for 698 covers** during the scan. The retry
loop: the decode-completed message re-requested the window after every outcome,
including failures, so a cover that did not exist yet was retried immediately until
the scanner wrote it. Fixed in `89a776f`; the scan and post-scan negative-cache
handling was consolidated in `bc60a64`, `4fed566`, `0cc4bd5`.

### Loop incident — 220 px build, 2× budget

The 220 px floor raised each entry to 193,600 B and lowered the 2× budget from 82 to
56 entries. The window set was declared **before** the prefetch band was appended, so
the band was evictable: decode → insert → evict → re-request. Measured at 05:04:46:

```
opens 2727 · decodes 2725 · hits 89 · misses 163 · lookups 252
evictions 2669 · cache_bytes 10841600 · in_flight 2 · scale 1.0 · bucket 220
~600 redraws/s · CPU ~50%
```

Fixed in `65c9620` with four regression tests (`26ea0c6`): the band is capped by the
real budget and every requested key is protected.

### Run 3 — 2× budget, after the loop fix: PARTIAL

| Point | Counters | Pss_Anon |
|---|---|---|
| P0 | 63 / 63 / 86 / 124 / 210 / 0 / 12,196,800 | 102,912 |
| P1 | 757 / 757 / 60,887 / 124 / 61,011 / 701 / 10,841,600 | 112,248 |
| P2 | 785 / 785 / 62,497 / 124 / 62,621 / 729 / 10,841,600 | 103,508 |
| P3 | 2,601 / 2,601 / 162,150 / 9,365 / 171,515 / 2,545 / 10,841,600 | 107,572 |

P2 added 28 opens/decodes — exactly one viewport. The 2× budget was fully consumed by
the 35-key window plus the 21-key band, leaving no entries behind. The 3× cushion
(`315471b`) reserved one viewport behind the window.

### Run 4 — 3× budget, app not restarted: PROTOCOL ISSUE

| Point | Counters | Pss_Anon |
|---|---|---|
| P0 | 141 / 76 / 3,785 / 1,137 / 4,922 / 0 / 14,713,600 | 278,464 |
| P1 | 755 / 690 / 59,809 / 1,137 / 60,946 / 606 / 16,262,400 | 322,568 |
| P2 | 1,111 / 1,046 / 69,356 / 5,325 / 74,681 / 962 / 16,262,400 | 284,352 |
| P3 | 2,965 / 2,900 / 171,271 / 17,026 / 188,297 / 2,816 / 16,262,400 | 327,256 |

P2 added 356 decodes because the wheel gesture covered roughly ten screens, well
beyond the four-row cushion; P0 also included post-scan memory (278 MB vs 100 MB after
a restart). Both are protocol issues, not code regressions.

### Run 5 — 3× budget, clean restart: PASSED

| Point | Counters | Pss_Anon |
|---|---|---|
| P0 | 63 / 63 / 86 / 124 / 210 / 0 / 12,196,800 | 100,184 |
| P1 | 686 / 686 / 58,754 / 124 / 58,878 / 602 / 16,262,400 | 114,780 |
| **P2** | **686 / 686 / 59,006 / 124 / 59,130 / 602 / 16,262,400** | **114,780** |
| P3 | 2,739 / 2,739 / 204,197 / 271 / 204,468 / 2,655 / 16,262,400 | 110,916 |

| Criterion | Result |
|---|---|
| P2 file opens | **+0** |
| P2 decodes | **+0** |
| P2 render misses | **+0** (124 → 124) |
| P2 `Pss_Anon` | **+0** |
| `cover_cache_bytes` | flat at 16,262,400 B across P1–P3 |
| Peak `Pss_Anon` delta | **+14.3 MiB** (P1 vs P0) |
| Full-pass `Pss_Anon` delta | **+10.5 MiB** (P3 vs P0) |
| Full-pass decodes | 2,053 for 676 albums = **3.04×**, bounded |
| Grid sharpness | album covers are sharp at the 220 px floor |

The full pass re-decodes because 676 covers do not fit in 84 retained entries; the
scroll-back criterion is proven by P2, which added **zero** opens and decodes.

---

## Defects found and fixed

| Defect | Fix commits |
|---|---|
| Completed decodes discarded on a stale scroll epoch | `851b616`, `e52ec85`, `841680f` |
| Grid row memo invalidated by every off-window decode | `841680f` |
| Failed decode immediately re-requested (scan churn) | `89a776f` |
| Negative cache cleared per batch; no post-scan cleanup | `bc60a64`, `4fed566`, `0cc4bd5` |
| Prefetch band unprotected (decode/evict loop) | `65c9620`, `26ea0c6` |
| No scroll-back cushion in the 2× budget | `315471b` |
| Library covers below 220 px | `6282122` |
| Counters invisible in the log file | `8e0c0db` |

## Reproduce

```bash
# 1. Reset (app stopped). No migration exists.
rm -f library.db library.db-wal library.db-shm
rm -rf cache/covers/

# 2. Build inside the container (never on the host, never --release).
distrobox enter dev-audoxidy -- bash -c 'cd /home/tix/Proyectos/Audoxidy/Audoxidy && mkdir -p "$HOME/.cache/audoxidy-cargo-tmp" && TMPDIR="$HOME/.cache/audoxidy-cargo-tmp" cargo build'

# 3. Run from an interactive container shell, let the scan settle (40 s cleanup).
./target/debug/audoxidy

# 4. Capture at each point (after the counters are stable for 10 s):
PID=$(pgrep -n audoxidy)
grep -E '^(Rss|Pss|Pss_Anon|Private_Dirty):' /proc/$PID/smaps_rollup
grep "cover cache counters" logs/audoxidy.log | tail -1
```

For P2 use a single wheel notch inside the cushion; the expected delta is zero.

## Caveats

- The 220 px floor and the 3× budget are the current policy; the earlier 2× and
  160 px figures are superseded.
- At 4K the 48 MiB ceiling caps the retained set below 3× viewport; the cushion is
  smaller and the full pass re-decodes, bounded by the ceiling.
- The player keeps one 400 px path handle per track change (two `openat` per rendered
  load); it is outside the library decode funnel by design.
