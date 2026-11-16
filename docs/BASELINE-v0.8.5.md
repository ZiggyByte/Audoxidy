# Audoxidy Behavior Baseline `v0.8.5`

**Recorded behavior baseline for milestone v0.8.5 (Refactor & Optimization).**

## Purpose & authority

This document is the recorded behavior baseline for the **v0.8.5** refactor and
optimization milestone. It is published with the repository at `docs/BASELINE-v0.8.5.md`,
together with its machine-diffable test-name list at `docs/baseline-tests-v0.8.5.txt`.

Both files travel with the repository, so a fresh clone carries the same measurement and
the same gates into the same paths. The comparison procedure in the test floor section
therefore applies unchanged in every checkout, and this baseline is the snapshot later work
is verified against.

It records:

- the **green-suite floor** — the total test count plus the sorted, fully-qualified
  test-name list (`docs/baseline-tests-v0.8.5.txt`);
- the **idle RAM/CPU footprint** with its capture procedure, as the pre-reduction
  reference;
- the **pre-consolidation tick/subscription set** and the **metrics snapshot** as the
  reference for the later tick and memory work;
- the **manual regression checklist** — the human-verifiable companion to the automated
  suite;
- the **existing versioned criterion baselines**, referenced rather than re-measured.

The tick set, the lock-order rules and the verification procedure are defined by the
**Audoxidy Standard** (`.planning/AUDOXIDY-STANDARD.md`, sections 5, 8 and 11). This
baseline records what the tree *is*; the Standard records what it *must be*. Where this
document cites the Standard, the Standard is authoritative and this document is the
snapshot.

**See also — `AUDOXIDY-STANDARD.md` § "Verification Against the Baseline".** That section
is the authority for how this baseline is applied: it defines the test-floor and
test-name comparison, the manual checklist gate, the advisory metrics tolerance, and the
convention guards. The gate policy below and that procedure must be read together; where
they differ, the Standard wins.

This milestone's dominant risk is **regression introduced by moving code or changing
resource usage** — not missing functionality. The automated suite catches compile and
logic breakage; this baseline exists to catch the silent regressions the suite cannot see:
a dropped tick, a broken crossfade tail, a purge that runs during playback, a deadlock
freeze, an unbounded growth in idle memory.

## Capture conditions

| Field | Value |
|-------|-------|
| Capture date | 2026-09-18 |
| Commit | `a53b142d99656f63facc97247f78d930fbf94457` |
| Host | `tix-slx` |
| OS | Soplos Linux Tyron (kernel `7.2.6-soplos-v3`, `x86_64`) — `uname -a`: `Linux tix-slx 7.2.6-soplos-v3 #1 SMP PREEMPT_DYNAMIC Tue Sep 15 03:59:18 CEST 2026 x86_64 GNU/Linux` |
| CPU | AMD Ryzen 7 5800X 8-Core Processor (8 cores / 16 threads) |
| Memory | 31 GiB installed |
| Toolchain | `rustc 1.98.1 (48a229cea 2026-09-01)` / `cargo 1.98.1 (797e8a9bc 2026-08-05)` |
| Build profile | development (`cargo test`, `cargo build`); **never `--release`** |
| Cargo `TMPDIR` | `$HOME/.cache/audoxidy-cargo-tmp` (disk-backed, never the RAM-backed `/tmp`) |
| Repository version reference | `v0.8.5` (milestone name; `Cargo.toml` still reads `0.8.0`) |

**Execution environment — current.** The Rust toolchain is not installed on the host.
Every command in this document runs inside the distrobox container `dev-audoxidy`; the
`cargo …` lines below are the inner commands, wrapped as:

```bash
distrobox enter dev-audoxidy -- bash -c 'cd /home/tix/Proyectos/Audoxidy/Audoxidy && <cargo command>'
```

The capture conditions above record the toolchain that produced the baseline
(`rustc 1.98.1` / `cargo 1.98.1` on the host) and are not rewritten. The container
currently ships `rustc 1.99.1` / `cargo 1.99.1`; if a comparison shows deltas the source
cannot explain, re-capture the floor rather than lowering it.

**Working-tree state at capture time.** The tree was **dirty**. The recorded commit
therefore does *not* fully describe the measured build; the measurement describes the
commit plus the working-tree changes listed below. `git status --porcelain` at capture:

```
M  .gitignore
 M src/audio/engine.rs
 M src/audio/tests.rs
 M src/db/database.rs
 M src/gui/app.rs
 M src/gui/audio_center.rs
 M src/gui/library.rs
 M src/gui/library_filters.rs
 M src/gui/player.rs
 M src/gui/widgets.rs
 M src/gui/widgets_tests.rs
 M src/lib.rs
?? docs/
?? src/core/
```

`docs/` was untracked at capture time (this baseline is its first content). It is published
with the repository today, so a fresh clone carries the same document and name list and the
comparison procedure below applies unchanged.

**Capture commands.**

```bash
mkdir -p "$HOME/.cache/audoxidy-cargo-tmp"

# Green suite (must pass before the inventory is recorded)
TMPDIR="$HOME/.cache/audoxidy-cargo-tmp" cargo test --quiet

# Fully-qualified test-name inventory (one name per line, sorted)
TMPDIR="$HOME/.cache/audoxidy-cargo-tmp" cargo test -- --list 2>/dev/null \
  | grep ': test$' | sed 's/: test$//' | LC_ALL=C sort \
  > docs/baseline-tests-v0.8.5.txt
wc -l docs/baseline-tests-v0.8.5.txt
```

## Gate policy

The baseline is enforced with two **HARD gates** and one **ADVISORY** class. The
separation is deliberate: a later phase must not be able to claim a pass by re-defining
the gate.

| Class | Item | Rule |
|-------|------|------|
| **HARD gate** | Test count and test-name list | The suite must never drop below the recorded floor, and no recorded test name may disappear. A missing name is a failure even if the total count still meets the floor. Renames and additions are allowed (the floor is a minimum, not a ceiling), but a rename that removes a recorded name must be justified in the phase summary. |
| **HARD gate** | Manual regression checklist | Every scenario in `## Manual regression checklist` must be executed and marked pass, or recorded as a deviation with severity and the phase expected to fix it. A failing scenario does not silently pass. |
| **ADVISORY** | Timing and RSS metrics | Investigate a **sustained 10–15 %** delta against the recorded figures. This is never a build failure. Absolute numbers depend on host CPU, kernel, load, thermals and frequency scaling, so only same-machine comparisons are meaningful. |

The advisory tolerance is the same policy `benches/BASELINES.md` already records for the
criterion baselines, so the criterion record and this behavior record agree rather than
compete.

**See also.** The procedure that applies these gates — the reproducible test-floor and
name-list comparison, the manual checklist, and the advisory metrics tolerance — is
defined by `AUDOXIDY-STANDARD.md` § **Verification Against the Baseline**. This table is
the policy; that section is the procedure.

## Test floor

**Original measurement: 322 tests, all green** (`test result: ok. 322 passed; 0 failed;
0 ignored`; the suite also runs two empty targets with 0 tests each).

**Recorded floor — HARD: 648.** The floor is the measured count of the green suite, and it
is the only authoritative threshold in this document. The first measurement recorded 322
tests, taken when the convention guards landed and expected to be approximately 311; the
measured value was 322, above the expectation, and was recorded as measured. **A future
measurement below the recorded floor is a finding to report, never a threshold to lower.**

**Amendment (2026-11-13):** the count was raised to **441** names once the name list had
grown past the original measurement. The file — not the historical count above — is the
source of truth for the floor.

**Amendment (2026-11-16):** the count was raised twice as the consolidating work landed:
to **632** names when the name list was re-based against the measured suite, and to **648**
names at its close, once the new coverage had landed and the suite was green again. The
count gate and the reproducible comparison command below both use **648**, the current
measurement. Every earlier measurement is preserved unchanged as history; the list only
ever grows.

The authoritative name list is `docs/baseline-tests-v0.8.5.txt`: currently 648 sorted
fully-qualified names, one per line, suitable for `diff`. The list is produced from a green
run of the current tree, so it records every test the suite runs today.

**Reproducible comparison command** (a later phase runs this and must see an empty
*removed* list):

```bash
# 1. List the current tests into a temporary file
mkdir -p "$HOME/.cache/audoxidy-cargo-tmp"
TMPDIR="$HOME/.cache/audoxidy-cargo-tmp" cargo test -- --list 2>/dev/null \
  | grep ': test$' | sed 's/: test$//' | LC_ALL=C sort \
  > "$HOME/.cache/audoxidy-cargo-tmp/current-tests.txt"

# 2. Count check — hard floor (raise, never lower)
test "$(wc -l < "$HOME/.cache/audoxidy-cargo-tmp/current-tests.txt")" -ge 648 \
  && echo "floor OK"

# 3. Removed names (must be empty — this is the failure signal)
LC_ALL=C comm -23 docs/baseline-tests-v0.8.5.txt \
  "$HOME/.cache/audoxidy-cargo-tmp/current-tests.txt"

# 4. Added names (expected as the codebase grows; informational)
LC_ALL=C comm -13 docs/baseline-tests-v0.8.5.txt \
  "$HOME/.cache/audoxidy-cargo-tmp/current-tests.txt"
```

## Idle footprint (pre-reduction reference)

This is the **pre-reduction reference** for the idle memory and CPU footprint. The
attribution of the footprint (heap vs mmap vs GPU) and every reduction decision belong to
the later memory phase; this section only records the starting point reproducibly.

**Procedure.** Build and launch the application in development mode (never `--release`),
from an isolated working directory with an isolated `HOME` so the library is empty and
nothing is playing:

```bash
BIN=target/debug/audoxidy
RUN="$HOME/.cache/audoxidy-19-04/run"
FHOME="$HOME/.cache/audoxidy-19-04/home"
mkdir -p "$RUN" "$FHOME"
cd "$RUN" && HOME="$FHOME" DISPLAY=:0 "$BIN" &   # empty library.db is created in $RUN
sleep 60                                          # let startup transients settle
PID=$(pgrep -n -f 'target/debug/audoxidy')

# Process RSS
ps -o rss= -p "$PID"
# Mapping-level breakdown (pidstat and smem are not installed on this host)
grep -E '^(Rss|Pss|Private_|Anonymous|Shared)' /proc/"$PID"/smaps_rollup
grep -E '^Vm(RSS|HWM|Size|Swap)' /proc/"$PID"/status
# CPU over a 60 s window
top -b -n 2 -d 60 -p "$PID"
```

Disk-backed `TMPDIR` is used for the build; the launched process itself needs none.

**Measured values.**

| Metric | Value | Command |
|--------|-------|---------|
| Process RSS, settled (60 s) | `165352 kB` (≈ **161.5 MiB**) | `ps -o rss= -p <pid>` |
| `smaps_rollup` `Rss` | `165352 kB` | `grep ^Rss /proc/<pid>/smaps_rollup` |
| `smaps_rollup` `Pss` | `91912 kB` | same |
| `smaps_rollup` `Pss_Anon` | `30180 kB` | same |
| `smaps_rollup` `Private_Dirty` | `30192 kB` | same |
| `smaps_rollup` `Shared_Clean` | `92696 kB` | same |
| `VmHWM` (peak RSS) | `165352 kB` | `/proc/<pid>/status` |
| `VmSize` (virtual) | `3459816 kB` (≈ 3.3 GiB) | `/proc/<pid>/status` |
| `VmSwap` | `0 kB` | `/proc/<pid>/status` |
| Process RSS, after the 60 s CPU window (≈ 120 s uptime) | `165564 kB` (≈ **161.6 MiB**) | `ps -o rss= -p <pid>` |
| CPU, 60 s window | **0.564 %** (34 jiffies at 100 Hz over 60.27 s) | `/proc/<pid>/stat` `utime+stime` delta |
| CPU, `top` second snapshot | **0.6 %** | `top -b -n 2 -d 60 -p <pid>` |

**Reading the numbers.**

- The footprint is **stable**: RSS moved from 161.5 MiB to 161.6 MiB across the second
  minute, and `VmHWM` equals the settled RSS (no peak above it).
- Idle CPU is **below 1 %** of one core. The cost is almost entirely the GUI/animation
  cadence, not the idle tick.
- Roughly **two thirds of the RSS is shared/file-backed** (`Shared_Clean` 92696 kB,
  `Pss` 91912 kB against `Rss` 165352 kB). The private anonymous working set is only
  ≈ 30 MB (`Pss_Anon` 30180 kB). Any reduction phase must reason on `Pss`/`Private`, not
  on the headline RSS alone.
- The capture spans exactly one **60-second reclaim cycle**: `FIXED_PURGE_INTERVAL_SECS`
  is 60 s, and the log shows the audio ring buffer being recreated and the output stream
  rebuilt (`Ringbuf recreado: 384000 muestras (48000 Hz x 2 ch x 2s)` / `Stream de salida
  construido y en PLAY`) at ≈ 60 s uptime. RSS did not grow across that cycle.

**Reference figure and discrepancy.** The milestone context records an observed idle
footprint of **~270 MB RAM with an empty library and nothing playing** (`PROJECT.md`).
The measurement above is **≈ 161 MiB** under the same nominal conditions — roughly 40 %
lower. The two figures are not reconciled here; the difference may come from a different
build/session, from RSS-vs-PSS accounting, or from GPU/driver mappings, and it is left for
the memory attribution phase to resolve with evidence. Both figures are recorded so that
neither is lost, and the **measured** value is the operative reference because its
procedure and host are recorded above.

### Reproducible four-scenario measurement (Pss_Anon)

**Metric hierarchy.** `Pss_Anon` (private anonymous heap) is the primary metric and the
reduction target. `Pss` is secondary context. `Rss` and `VmSize` are reported but are not
targets, because roughly two thirds of `Rss` is file-backed/shared (debug image, Mesa/LLVM/
nouveau, two system-font enumerations) and no application refactor can reduce it.

**Budget.** `Pss_Anon` < 100 MB with 20k songs while playing. The "measure a release build"
lever is dropped: the owner measured debug and release as equal on this host — the dominant
shared pages are identical in both builds, and only touched binary pages are resident.

**Isolation.** Every scenario uses a private `RUN`/`FHOME` under
`$HOME/.cache/audoxidy-mem/` with `HOME="$FHOME"` and cwd `$RUN`, so the capture cannot
read or write the developer's `library.db`/`covers/`. Use the disk-backed
`TMPDIR="$HOME/.cache/audoxidy-cargo-tmp"` for any build.

**The four scenarios:**

| Scenario | Description | Precondition |
|----------|-------------|--------------|
| (a) Fresh idle | Empty library, nothing playing | Fresh `RUN`/`FHOME`, no prior scan |
| (b) One album added, idle | Single album (~8 songs) scanned | Add one album to the isolated library, let scan settle |
| (c) 20k songs, idle | Full library, nothing playing | Populate with the 20k-song library, idle |
| (d) 20k songs, playing | Full library, playback active | Start playback, capture after 60 s of playback |

If the full 20k set is unavailable on the host, capture the largest available set and record
the actual song count as a deviation (do not silently substitute).

**The exact capture recipe** (reused verbatim for every scenario):

```bash
BIN=target/debug/audoxidy
RUN="$HOME/.cache/audoxidy-mem/run"
FHOME="$HOME/.cache/audoxidy-mem/home"
mkdir -p "$RUN" "$FHOME"
cd "$RUN" && HOME="$FHOME" DISPLAY=:0 "$BIN" &
sleep 60   # let startup transients settle
PID=$(pgrep -n -f 'target/debug/audoxidy')

# Process RSS
ps -o rss= -p "$PID"
# Mapping-level breakdown (pidstat and smem are not installed on this host)
grep -E '^(Rss|Pss|Private_|Anonymous|Shared)' /proc/"$PID"/smaps_rollup
grep -E '^Vm(RSS|HWM|Size|Swap)' /proc/"$PID"/status
```

**Advisory tolerance.** The 10–15 % advisory tolerance already recorded in the gate policy
applies to same-host before/after comparison; cross-host or cross-scenario absolute values
are context only.

**Reporting fields per scenario:** scenario label, UTC timestamp, build (debug), the five
`smaps_rollup` numbers (`Rss`, `Pss`, `Pss_Anon`, `Private_Dirty`, `Shared_Clean`),
`VmSize`, `VmHWM`, and a one-line attribution note (what changed between this scenario and
the previous one).

### Recorded baseline — 2026-09-21 (pre-fix)

Build: debug (`target/debug/audoxidy`). Host: Linux, Mesa/llvmpipe + nouveau.
20,744 songs, 2,767 covers (109 MB AVIF on disk).

| Scenario | Rss (kB) | Pss (kB) | Pss_Anon (kB) | Private_Dirty (kB) | Shared_Clean (kB) | VmSize (kB) | VmHWM (kB) | Δ Pss_Anon vs prev |
|----------|----------|----------|---------------|--------------------|--------------------|-------------|------------|---------------------|
| (a) Fresh idle | 169,648 | 83,301 | 30,036 | 30,040 | 113,432 | 3,525,888 | 169,648 | — |
| (b) 1 album, idle | 194,148 | 107,629 | 49,828 | 49,852 | 113,612 | 4,049,068 | 194,148 | +19,792 |
| (c) 20k songs, idle | 253,956 | 167,735 | 94,232 | 94,236 | 113,284 | 3,737,216 | 253,956 | +44,404 |
| (d) 20k songs, playing | 298,068 | 211,686 | 130,132 | 130,144 | 113,512 | 3,760,652 | 298,068 | +35,900 |

**Veredicto:** `Pss_Anon` < 100 MB pasa en (a), (b), (c). **Falla en (d): 127.1 MB** (27.1 MB
por encima del objetivo de 100 MB). El incremento (c)→(d) de +35.9 MB corresponde al
decoder buffer (~64 MB virtual, ~35 MB residente lazy), resampler, ring buffer 8M f32
(32 MB virtual) y DSP chain — costo legítimo de la reproducción activa.

**Attribution (d) breakdown:**
- Decoder predecode buffer: ~64 MB virtual (8M f64), ~35 MB residente
- Ring buffer: 384,000 samples (48 kHz × 2 ch × 2s) = ~3 MB residente
- Resampler: ~0.5 MB (64-bit audiophile mode, 44100→48000)
- DSP chain: ~1 MB (biquads, compressor, limiter, reverb)
- GPU driver + Mesa/LLVM shared: ~113 MB `Shared_Clean` (immutable, not reducible)

**Next step:** Implement Tier 0 fixes targeting the (c)→(d) overhead and the 92 MB idle baseline.

### Recorded baseline — 2026-09-21 (post-fix Tier 0)

Build: debug (`target/debug/audoxidy`). Host: Linux, Mesa/llvmpipe + nouveau.
20,744 songs, 2,767 covers (109 MB AVIF on disk).
Fixes applied: shrink_to_fit idle-safe (library Vecs, decoder preloaded_pending), artist_groups release in unload, clear_all_cover_cache renamed.

| Scenario | Rss (kB) | Pss (kB) | Pss_Anon (kB) | Private_Dirty (kB) | Shared_Clean (kB) | VmSize (kB) | VmHWM (kB) | Δ Pss_Anon vs pre-fix |
|----------|----------|----------|---------------|--------------------|--------------------|-------------|------------|------------------------|
| (a) Fresh idle | 169,560 | 83,370 | 30,124 | 30,128 | 113,268 | 3,525,388 | 169,560 | +88 (≈0) |
| (b) 1 album, idle | 178,848 | 92,298 | 34,068 | 34,072 | 113,704 | 4,012,912 | 193,592 | −15,760 (−31.6%) |
| (c) 20k songs, idle | 256,712 | 170,374 | 96,920 | 96,924 | 113,416 | 3,736,944 | 256,712 | +2,688 (+2.8%) |
| (d) 20k songs, playing | 299,928 | 213,597 | 131,744 | 131,756 | 113,428 | 3,760,840 | 299,928 | +1,612 (+1.2%) |

**Veredicto post-fix (budget aceptado):** El objetivo original de `Pss_Anon < 100 MB` en
reproducción (escenario d) no es alcanzable sin cambiar el decoder buffer (~35 MB legítimo).
**Budget aceptado: ~130 MB `Pss_Anon` con 20k canciones reproduciendo.** El costo del decoder
es ineludible (8M f64 para predecode de calidad audiophile). Los fixes Tier 0 demostraron
eficacia en el path de unload idle (escenario b: −31.6%).

**Mejora principal:** escenario (b) bajó de 48.7 MB a 33.3 MB (−31.6%), demostrando que
`shrink_to_fit` + `artist_groups` release funcionan en el path de unload idle.

## Tick & subscription snapshot (pre-consolidation reference)

Recorded source: the subscription builder `subscription()` in `src/gui/app.rs`. One row
per subscription, with its cadence, activation gate and consumers.

| Subscription | Message | Cadence | Activation gate | Consumers |
|--------------|---------|---------|-----------------|-----------|
| progress tick | `Message::Tick` | 500 ms while playing or scanning; 5000 ms idle | always (the duration itself is the gate) | status cache, library-dirty poll, art sync, preload trigger, end-of-track advance, autosave counter (every 40 ticks), metrics emission, GC/purge decisions, crossfade trigger |
| UI / marquee tick | `Message::UITick` | 500 ms | player UI active, or playing with a title/artist that needs the marquee | player chrome activity, title/artist marquee |
| drag tick | `Message::DragTick` | 32 ms | library drag active (`library_manager.is_dragging`) | library auto-scroll while dragging |
| meter tick | `Message::MeterTick` | 30 ms (~33 Hz) | playing, or the meter is fading out | VU meter ballistics and the pause fade |
| search pulse | `Message::SearchPulse` | 100 ms | a search query is non-empty (library or filters) | none — the handler performs no work |
| stepper-hold tick | `Message::Tick` | 30 ms | a number stepper is held (`GLOBAL_STEPPER_HOLD != 0`) | keeps redraw alive so the stepper auto-repeat advances |
| mouse / keyboard stream | `PlayerMouseMoved`, `LibraryMarqueeStart`, `GlobalMouseRelease`, `GlobalKeyDown`, `ModifiersChanged`, `WindowResized`, `SetWindowId` | event-driven | always | window-id capture, cursor tracking, click/key handling, resize, window events |
| external control stream | `Message::ExternalControl` | event-driven | always | system media controls (via a stable recipe identity so a single bridge stream stays alive) |

**Known defect — recorded, not fixed here.** Two subscriptions share the same 500 ms
duration: the progress tick (500 ms while playing or scanning) and the UI/marquee tick
(500 ms). The runtime deduplicates subscriptions whose recipe identity matches, so two
timers with the same duration can be treated as one stream and **the second never fires —
silently, with no error**. This is a defect of the recorded set. It is **not** fixed by
this baseline; any fix belongs to the tick work item that owns the inventory and holds the
user's authorization (see Audoxidy Standard §8, which records the same defect).

**Change rule.** Changing any cadence requires the tick phase's written inventory and the
user's explicit authorization. Nothing in this snapshot authorizes removing, merging or
re-timing a tick; the snapshot exists so that a later phase can prove it changed nothing
by accident.

## Metrics snapshot

The application keeps four process-global performance counters in
`PerformanceMetrics` (`src/utils/observability.rs`). They are fed by structured `tracing`
events whose target is `audoxidy::metrics` and that carry a `metric = "<name>"` field plus
a numeric `u64` field; `MetricsLayer` switches on the metric name and updates the
corresponding atomic.

| Field | Meaning | Source event (`metric =`) | Emitter |
|-------|---------|---------------------------|---------|
| `audio_latency_peak_us` | peak audio latency, microseconds | `audio_latency` (`latency_us`) | GUI tick path, `src/gui/app.rs` |
| `audio_frames_processed` | decoded audio frames | `audio_frames_processed` (cumulative add) | decoder thread, `src/audio/decoder.rs` |
| `underrun_count` | ring-buffer underruns | `underrun` (`count`, cumulative add) | GUI tick path, `src/gui/app.rs` |
| `ram_usage_bytes` | process RSS in bytes | `ram_usage_bytes` (latest store) | GUI tick path, `src/gui/app.rs`; value from `MemoryManager::current_process_ram_bytes()` |

The GUI-sourced counters are emitted from the progress tick, so their sampling cadence is
the tick cadence: **500 ms while playing or scanning, 5000 ms idle**. The frame counter is
emitted by the decoder thread on its own batch boundary.

**Idle values observed during the footprint capture.** During that capture nothing was
loaded and nothing was playing, so `audio_frames_processed` and `underrun_count` stayed at
**0** and `audio_latency_peak_us` stayed at **0** (no decode or output path ran).
`ram_usage_bytes` is the process RSS as reported by `MemoryManager`, i.e. ≈ **161 MiB**
(`165352 kB`) at the settled measurement.

**Observation — the counters are not externally readable.** `MetricsLayer` is filtered to
the metrics target, and the file and stderr layers explicitly exclude that target, so
metric events reach no sink and the atomics are only observable from inside the process.
The capture could therefore not read the counters directly; the idle values above follow
from the capture conditions and from the RSS measurement, and they are recorded as such.
A later phase that needs the counters externally must add a sink or an in-app readout; that
is out of scope here.

**Audio-safety signals.** `underrun_count` and `audio_latency_peak_us` are the audio-safety
signals that a memory reduction must not disturb: a purge that starves the ring buffer, or
a forced free-to-OS call on the audio path, shows up here first. They are recorded as part
of the baseline for exactly that reason.

## Benchmark baselines (referenced, not re-measured)

The tracked criterion baselines are **referenced, not re-measured** in this phase. The
authority is `benches/BASELINES.md`:

- the **`post-audio-core`** record — the authoritative baseline for the DSP-chain,
  engine-mix and utility benchmarks (captured 2026-09-13 at commit
  `aec143955ae4708563c027f9798c44f7ac26053e`, on this same host and toolchain);
- the **CoverCache LRU evaluation** runs (`covercache-lru-eval-run1` /
  `covercache-lru-eval-run2`), a self-contained comparison whose decision record is
  `benches/COVERCACHE_LRU_DECISION.md`.

The comparison policy is the same in both records and is restated here so the two agree:
comparison is **advisory**, a sustained **> 10–15 %** regression for the affected
benchmark is a prompt to investigate, and it is **never a blocking gate**. Baseline data
lives in the gitignored `target/criterion/**`; the version-controlled documents are the
authority. No new benchmark numbers are captured in this phase.

## Manual regression checklist

This is the **human-verifiable companion** to the automated suite, and it is a **HARD
gate**. The scenarios below are the only reliable detector for the silent regressions this
milestone risks: a dropped tick, a broken crossfade tail, a purge during playback, a
deadlock freeze.

Every run must be performed with a **development build** (never `--release`), a populated
library and an audio device available. For each scenario record **precondition, steps,
expected result, observed result, result (pass/fail), and evidence** (a log excerpt or a
screenshot). A failure does not silently pass: it is recorded as a known deviation with
its severity and the phase expected to fix it.

**Execution status: executed — all six scenarios pass.** The checklist was run by the
repository owner against a development build with a populated library and an audio device
(scenario 6 with deadlock detection enabled). Every scenario is marked **pass** below. The
evidence cells record the verification source; no log excerpt or screenshot was supplied to
the recorder, so none is claimed.

### 1. GC / reclaim

- **Precondition:** app launched, empty-to-populated library, reclaim timer
  (`FIXED_PURGE_INTERVAL_SECS` = 60 s) runnable.
- **Steps:** leave the app idle past the reclaim timer and read the reclaim log lines;
  then play audio and watch for buffer reclaim or stream recreation; then start a library
  scan and confirm reclaim is suppressed; then start a preload/crossfade soon after a scan
  and confirm the preload safety window keeps reclaim suppressed.
- **Expected result:** reclaim runs while genuinely idle; **no** reclaim of audio buffers
  and **no** stream recreation while playing; reclaim suppressed during a scan; suppressed
  during the post-scan preload safety window.
- **Observed result:** behaved as expected — no reclaim of audio buffers and no stream
  recreation while playing; reclaim suppressed during the scan and the post-scan preload
  window.
- **Result:** **pass**
- **Evidence:** User-verified pass (GC / reclaim scenario). No artifact attached.

### 2. Track change

- **Precondition:** playlist with at least three tracks, audio device available.
- **Steps:** advance and go back with the transport controls; let a track reach its end
  and let the automatic end-of-track advance fire.
- **Expected result:** playlist order, now-playing metadata, artwork and progress display
  all stay in sync; progress restarts at 00:00; no residual audio plays at the wrong
  position.
- **Observed result:** behaved as expected — order, metadata, artwork and progress stayed in
  sync through both control-driven and automatic advances.
- **Result:** **pass**
- **Evidence:** User-verified pass (track change scenario). No artifact attached.

### 3. Crossfade (automatic)

- **Precondition:** crossfade enabled, a next track available, a track long enough to
  reach the preload window (about 15 s before the end).
- **Steps:** let the track play into the preload window and let the automatic forward mix
  start.
- **Expected result:** the forward mix starts without a jump, without a speed change, and
  without a cut at the previous track's tail end.
- **Observed result:** behaved as expected — the forward mix started without a jump, without
  a speed change and without a cut at the tail end.
- **Result:** **pass**
- **Evidence:** User-verified pass (crossfade automatic scenario). No artifact attached.

### 4. Crossfade (manual)

- **Precondition:** same as above; the manual crossfade available in the transport.
- **Steps:** trigger the manual crossfade at an arbitrary second.
- **Expected result:** it starts immediately, mixes cleanly, and the ring buffer covers
  the opening without an audible gap.
- **Observed result:** behaved as expected — the manual crossfade started immediately, mixed
  cleanly and had no audible gap at the opening.
- **Result:** **pass**
- **Evidence:** User-verified pass (crossfade manual scenario). No artifact attached.

### 5. Database refresh

- **Precondition:** populated library; ability to modify the library on disk.
- **Steps:** modify the library on disk (or run a scan) and wait for the refresh to reach
  the UI; then exercise search.
- **Expected result:** the refresh reaches the UI with updated counts and metadata and is
  not a silent no-op; search results stay consistent afterwards.
- **Observed result:** behaved as expected — the refresh reached the UI with updated
  counts/metadata and search stayed consistent.
- **Result:** **pass**
- **Evidence:** User-verified pass (database refresh scenario). No artifact attached.

### 6. Window drag and resize

- **Precondition:** the run must be performed **with the deadlock-detection feature
  enabled** (`--features deadlock-detection`), because the writer-preference freeze class
  is the risk being watched. Build still in development mode, never `--release`.
- **Steps:** drag the window from the player top bar, from the library-filters top bar
  and from the library top bar; resize; maximize and restore.
- **Expected result:** the layout reflows without freeze and with no deadlock.
- **Observed result:** behaved as expected — drag from all three top bars, resize and
  maximize/restore reflowed without freeze or deadlock, with deadlock detection enabled.
- **Result:** **pass**
- **Evidence:** User-verified pass (window drag and resize scenario, `--features
  deadlock-detection`). No artifact attached.

## Extended manual regression checklist (nine scenarios)

**Execution status: executed — nine scenarios, nine pass, with one recorded observation and
four classified log lines in scenario 8.**

Recorded 2026-11-14 by the repository owner on a real graphical session. Build:
development (`cargo run`, debug profile, never `--release`) inside
`distrobox enter dev-audoxidy`, with an audio device available and the owner's library and
cover cache in place (except scenario 1, see below).

The nine checks extend the six scenarios above with the three checks this milestone's
schema work needs: the versioned rebuild and its startup notice, search coverage after a
rename, and the close-path query-planner call.

| # | Check | Relationship to the checklist above |
|---|-------|--------------------------------------|
| 1 | Rebuild notice and rescan | new — the versioned rebuild, its startup notice and the rescan |
| 2 | Search and rename coverage | new — search coverage after a folder rename |
| 3 | GC / reclaim | baseline scenario 1 |
| 4 | Track change | baseline scenario 2 |
| 5 | Automatic crossfade | baseline scenario 3 |
| 6 | Manual crossfade | baseline scenario 4 |
| 7 | Database refresh | baseline scenario 5 |
| 8 | Window drag / resize with a scan running and a track playing | baseline scenario 6 |
| 9 | Close | new — the close-path query-planner call |

### Results

| # | Scenario | Result | Observed behaviour |
|---|----------|--------|--------------------|
| 1 | Rebuild and rescan | **pass** | The notice appeared with an empty library behind it, the folder picker opened, and the scan started. The second launch showed no notice and a populated library. |
| 2 | Search and rename coverage | **pass** | Search found the album by its title. The copied album folder was renamed on disk, deleted and re-scanned; it was then found by the new title and not by the old one. |
| 3 | GC / reclaim | **pass** | The idle GC/reclaim cycle released memory and playback still worked afterwards. |
| 4 | Track change | **pass** | A track change started the next track at 00:00. |
| 5 | Automatic crossfade | **pass** | The transition mixed and the UI position was correct for the new track. |
| 6 | Manual crossfade | **pass** | The change was immediate, with no jump and no fast playback in the tail. |
| 7 | Database refresh | **pass** | Adding files and scanning updated the library, the totals and the filtered grid without a restart. |
| 8 | Window drag / resize during scan and playback | **pass, with an observation** | Detailed below. The window stayed functional through resizing, maximise, minimise and navigation; the owner classified the scenario as pass because it required an extreme 15,000-file scan plus manual resizing, which normal use does not reproduce. |
| 9 | Close | **pass** | The close produced no optimize error before the exit line, and the exit line was the last entry written. |

**Evidence.** Owner-reported results; no screenshot or log file was supplied to the
recorder, so none is claimed beyond the log lines quoted for scenario 8. A successful close
is deliberately silent — the helper logs only when `PRAGMA optimize` fails — so scenario 9's
pass is the absence of that error line, not a positive log entry.

### Recording conditions

- **Scenario 1 was isolated.** It ran in a private working directory with a fake
  `user_version=3` database, so the destructive rebuild path never touched the owner's real
  `library.db`.
- **Scenarios 2-9 ran against the owner's real library** and cover cache.
- **Scenario 8 was deliberately extreme.** A 15,000-song folder was re-scanned while a
  playlist played, and the window was resized manually from ~700 px to ~1800 px, maximised,
  minimised and navigated. Brief millisecond-scale interface pauses were observed while
  resizing manually.
- **Recording limitation for scenario 8:** the owner's report does not state whether the
  build carried the `deadlock-detection` feature, so this record does not claim it. What is
  recorded is the observed responsiveness and the log lines below, none of which is a
  deadlock report, a panic or a data-loss message.

### Scenario 8 observation — non-blocking

The owner classified the scenario as pass. The lines logged during it, and their
classification, are recorded here rather than dropped:

| Log line | Classification |
|----------|----------------|
| `audoxidy::persistence: No se pudo guardar el estado de reproducción ... database is locked` and `No se pudo persistir el ajuste key="shuffle_session" error=database error: database is locked`, while the scan ran and not after it finished | **Expected and non-blocking.** The scanner writes through its own connection while SQLite allows one writer; during the scan the persistence tick cannot acquire the write lock, so the write is logged and retried on the next tick. Playback state and the shuffle session are best-effort by design, nothing is corrupted, and both writes resumed once the scan finished. **Follow-up, outside this phase's scope:** move playback-state persistence off the UI thread or give it a shorter busy timeout, so a heavy scan cannot delay the tick. |
| `iced_winit: More than 3 consecutive RedrawRequested events produced layout invalidation` during the aggressive manual resize | **Framework warning, non-blocking.** An iced diagnostic about layout invalidation under a flood of resize events, not a deadlock and not a leak; the window remained functional. |
| One `No se pudo procesar/guardar la carátula ... Failed to decode image` | **Non-blocking.** A single unreadable embedded image; the scanner skipped that cover and continued with the rest. |
| `lofty::mpeg: Using bitrate to estimate duration` | **Documented fallback, non-blocking.** The scanner estimates an MPEG duration from the bitrate when the container does not expose one. |

The `database is locked` pair is the only one with a design consequence, and it is recorded
as debt for the persistence path rather than as a failure of this phase: it appears only
under a heavy scan on the same database, it costs a delayed best-effort write, and the
phase's data was intact afterwards.

---

*The baseline was recorded when the foundation work and the convention guards landed, and
the Audoxidy Standard remains authoritative for conventions; this baseline is the recorded
snapshot later work is verified against. The nine-scenario manual checklist was recorded
afterwards, against a development build on a real graphical session.*
