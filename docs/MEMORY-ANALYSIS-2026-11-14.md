# Memory analysis — 280 MB peak during playback at 384 kHz

Last updated: 2026-11-18

Scope: Audoxidy playback and browsing at 384,000 Hz stereo, with a library of 27,743 songs
and a full-library playlist. Measured peak: ~280 MB. Floor after reclaim: ~210 MB. Earlier
measured ceiling: ~190 MB.

**Amendment 2026-11-18.** The cover-store trim now follows inactivity rather than focus, and
the resulting peak is **~219 MiB** with a post-browsing floor of **~214 MiB** — see § 8, which
also records the measured playback profile and the sampling method. The library this section
was written against has since grown to **35,961 songs and 3,495 albums**, so the per-song
holders below scale about 30 % above the counts stated here.

This document records the evidence and the budget. It is a static, code-based attribution;
the numbers come from the live application.

---

## 1. Conclusion

**There is no leak.** The peak is accumulated retention that the GC returns when it runs, and
the 210 MB floor is the capacity the engine intentionally keeps. Every large allocation has a
fixed ceiling; none grows with playback time or with the number of track changes.

The cover cache adds **one** large holder: the decoded-cover store, bounded at 32 MiB (reduced
from 48 MiB; the measured 1080p / 220 px budget is 16.26 MB, so the ceiling only binds at high
DPI) and deliberately untouched by the GC. That, plus the playlist and filter-index copies for a
27,743-song library, explains the ~70–90 MB delta over the earlier 190 MB measurement. The audio
engine buffers are unchanged by the metadata and cover-cache work.

---

## 2. Budget at the measured configuration

Assumes stereo. The last column shows the 8-channel case for the buffers whose size scales
with channels.

| Holder | Stereo @ 384 kHz | 8 ch @ 384 kHz | Lifetime |
|---|---:|---:|---|
| Promoted preload head (one buffer; roles exchanged, cap = crossfade max + 2 s, clamped to 8 s) | 18.4 MB | 64 MB (8 M-sample ceiling) | Reused across tracks; no copy, no second allocation |
| Audio ring buffer (recreated as `rate × channels × 2 s`) | 6.1 MB | 24.6 MB | Constant after the rate change |
| Ring buffer if the device rate never triggers a recreate | 32 MB | 32 MB | Constant |
| Mix tail buffer (200 ms) | 1.2 MB | 4.9 MB | Constant |
| DSP chain (reverb FDN, EQ, compressor, limiter) | ~1.8 MB | ~1.8 MB | Constant |
| **Decoded-cover store** | **≤ 32 MB** (~16 MB at 1080p / 220 px) | ≤ 32 MB | Budget-bounded |
| iced cover textures (viewport + 50 % margin) | 25–50 MB | 25–50 MB | Viewport-bounded |
| Player cover handle (400 px cache) | ~0.6 MB | ~0.6 MB | One per track |
| Playlist (`PlaylistSongRef`, shared `Arc`) + filtered view | ~7 MB | ~7 MB | While the playlist is loaded |
| Filter index (arena, 19,881 nodes) | ~1 MB | ~1 MB | Never dropped |
| SQLite page cache (2 conns) + mmap window | 8–24 MB | 8–24 MB | Constant |
| Interner (purges at 10 MB) | ≤ 10 MB | ≤ 10 MB | Bounded |
| Base private heap (fonts, GPU buffers, runtime, MPRIS) | ~30 MB | ~30 MB | Constant |
| Shared (release binary, GPU drivers, fonts) | ~60–90 MB | ~60–90 MB | Constant |

Sum: **~230–290 MB peak**, which matches the measured 280 MB. The 210 MB floor is the same
budget with the cover store and the viewport textures evicted and the interner/playlist caches
cleared.

### Why the preload caps are what they are

`predecode_cap_frames_for` (`src/audio/decoder.rs`) computes

```
cap_ms = clamp(xfade_max_ms + 2000, 0, 8000)
samples = min((cap_ms / 1000) * out_rate * channels, 8_000_000)
```

With the persisted settings (`crossfade_auto_ms = 250`, `crossfade_manual_ms = 1000`; the manual
value counts even while manual crossfade is disabled), `xfade_max_ms = 1000`, so `cap_ms = 3000`
→ 2,304,000 samples at 384 kHz stereo → **18.4 MB** in f64. The 8 M-sample ceiling is 64 MB. The
cap dates from the crossfade redesign (commit `3eacdcf`).

---

### The `[AUDIO_HEALTH]` diagnostic

`[AUDIO_HEALTH] open_preload_track tomó N ms` measured opening and probing the next file on the
decoder thread; it fired above 10 ms. On a slow mount, 18 ms is normal disk latency. The audible
cut correlated with that block: while the decoder opened the next file, it stopped refilling the
ring. The promotion also copied the whole head on that thread. The head now uses one buffer, and
the preload open/probe runs on a helper thread (`fd51cd5`), so the decoder never blocks on it; the
warning was removed because it no longer measures an audio-thread operation. If a cut ever
reappears, look at the preload decode batch or the disk, not the log.

The same 15 s window prefetches the next cover from the 400×400 AVIF cache (a few KB); the
library decodes it down to 220 px for its textures and the player uses the 400 px file. A
short-lived experiment decoded it synchronously through the bounded funnel (`8f103e1`) to cap a
future larger source, but that decode competed with the preload open at the same moment and was
reverted (`34491e4`); `Handle::from_path` decodes the 400 px file off the GUI thread. It was not
the cause of the memory spike.

## 3. Why the peak persists while changing tracks

`Core::evaluate_gc` (`src/core/maintenance.rs`) runs the purge on a 60 s timer and on RAM
pressure. It returns early when either the scanner is running or `gc_blocked_by_preload` is true
(`src/gui/app/update/memory.rs`), which covers the preload's last 15 s plus 20 s after the
transition. Consecutive track changes therefore leave few windows for the timer, and the GC's own
500 MB app cap never fires at 280 MB.

When it does run, `handle_memory` (`src/gui/app/update/memory.rs`) unloads the inactive playlist,
clears the virtualization caches, drops the decoded covers outside the visible window when the
library is unfocused, clears the interner and purges the audio preload, then calls
`malloc_trim(0)`. The visible cover window is kept, so returning to the same scroll position
stays instant.

The library's own row data is deliberately **not** released. It used to be truncated to 100
albums and 500 songs with its artist groups dropped, which forced a database reload and left the
list views empty until the grid rebuilt them. Cover RAM is now bounded by the store's byte budget
and the guarded off-window trim above, so that truncation only traded a bounded win for a
visible rebuild.

---

## 4. No-leak evidence

- No per-song audio cache exists: no waveform, spectrum, FFT or play-history structure in
  `src/audio`; the decoder state is dropped at end of track.
- `ShuffleSession.history` grows ~8 bytes per played track — negligible, and the only monotonic
  per-play growth found.
- Player metadata fields are overwritten per track, not accumulated.
- The interner self-purges at 10 MB.
- Every large buffer has a ceiling (table above); none is proportional to elapsed playback.

These conclusions match an earlier attribution study recorded with the benchmark notes, which
predicted the same shape before the live measures existed.

---

## 5. Retention levers and their visible impact

Ordered by value/risk. **None of them affects text rendering, cover images once decoded, or
window navigation.** The only visible effect is a possible re-decode or re-query when returning
to a view.

| Lever | Expected saving | Visible impact | Status |
|---|---:|---|---|
| Trim the decoded-cover store when the library is unfocused (keep the current window) | up to the store size (≤32 MB; ~16 MB at 1080p) | Returning to the library keeps the window; scrolling into the trimmed band re-decodes asynchronously | **Applied (`2a6d0fb`)** |
| Drop `filter_index` and `tree_data` on purge when the library is unfocused | 5–15 MB | The filter panel would be empty until a reload; rejected | **Not used; index compacted instead (`5807a7f`)** |
| `shrink_to_fit` on the preload buffers from an idle path | up to 37 MB | None, but freeing the capacity forces the next preload to grow it again | **Not used; one reused buffer instead (`482cee9`)** |
| Reduce the `+2000 ms` preload margin | ~12 MB at 384 kHz stereo | The mix would cover less of the song; requires a listening test | **Retained at +2000 ms** |
| Background-purge allocator (mimalloc/jemalloc) | Allocator-dependent | None | **Not used; new dependency** |

The decoded-cover store cannot be removed without losing the scroll-back guarantee; the budget
lever is its ceiling, not its existence.

### What was actually recovered

The 340 → 250 MB figure is partly the reversal of a regression introduced by an early idle
release: freeing the preload buffers made the next preload reallocate them, producing the 340 MB
spike. The honest comparison is against the **280 MB baseline** measured before those changes:

| Change | Steady saving | Notes |
|---|---:|---|
| One preload head instead of two (`482cee9`) | ~18.4 MB | 64 MB at 8 channels; reused across tracks |
| Filter index arena (`5807a7f`) | ~3 MB | 19,881 nodes measured on the real library |
| Playlist filtered view sharing `Arc` (`48a6d0c`) | ~5.5 MB while a query matches everything | transient |
| Cover trim on GC (`2a6d0fb`) | ~5–10 MB when the library is unfocused | re-decodes on return |
| Cover ceiling 32 MiB (`98a769e`) | 0 at 1×; up to 16 MB at high DPI | validated at 4K |
| **Total steady** | **~21–25 MB** | measured peak 280 → 250 (~30 MB) |

The 250 MB peak is real but modest; the 340 MB figure was the idle-release regression, not a
recovered baseline. The remaining large holders are the 18.4 MB inherent preload head, iced's
viewport textures, SQLite and the interner; the last two are kept by policy.

### Implementation status

- **Cover-store trim — applied (`2a6d0fb`).** With the library out of focus the GC drops the
  decoded covers outside the visible window and keeps the window. Returning to the same scroll
  position is unchanged; scrolling into the trimmed band re-decodes those covers asynchronously
  (1–3 ms per 220 px cover). The earlier source-text guard was replaced by the behavioural test
  that pins window retention.
- **Filter index — compacted, not dropped (`5807a7f`).** The panel must stay visible, so the
  module is never freed.
- **Preload head — one reused buffer (`482cee9`).** The two buffers exchange roles with
  `mem::swap`, so only one head capacity exists and the drained buffer is reused. The promotion
  no longer copies. The manual `Load` path no longer frees the capacity either (`25a0294`).
- **Preload margin — retained at +2000 ms.**
- **Playlist filtered view — shared `Arc<PlaylistSongRef>` (`48a6d0c`).** The filtered view clones
  pointers instead of ~200-byte structs (~5.5 MB → ~220 KB for a full match). Mutations use
  `Arc::make_mut` only on the changed song.

### Filter index — compacted instead of dropped

Measured on the real library: 19,881 nodes across the five trees (genre 2,995; artist 2,630;
album 4,502; year 3,805; folder 5,949). The old tree stored a `String` label and a full-path
`String` id per node, and `apply_view` cloned the active tree while the flattened view copied the
strings again.

The arena (`5807a7f`) stores `Arc<str>` labels shared with the database's interned strings, a
`u32` identity and parent index per node, and `u32` indices in `tree_data` and the flattened
view. The panel is never emptied and never reloaded: `apply_view` only computes a visibility
mask. Estimated cost drops from ~4 MB to ~1 MB; the path is rebuilt only when a node is clicked.

---

## 6. How to confirm live

1. Run the release build manually.
2. Reproduce: 384 kHz output, one playlist of the full library, change tracks for several
   minutes without navigating.
3. Capture `grep -E 'Rss|Pss|Pss_Anon|Private_Dirty' /proc/<pid>/smaps_rollup` at peak, after
   the 60 s GC, and after a manual purge.
4. Attribute the delta against the budget table above: the preload head, the cover store and the
   playlist/filter structures are the only holders large enough to move the number by tens of MB.

---

## 7. References

| Topic | Location |
|---|---|
| Predecode cap | `src/audio/decoder.rs` (`predecode_cap_frames_for`) |
| Preload promotion (head roles exchanged) | `src/audio/decoder.rs` (`promote_preload_head`) |
| Preload open thread | `src/audio/decoder.rs` (`open_preload_track_blocking`) |
| Ring buffer size and recreate | `src/audio/engine.rs` |
| GC guard | `src/gui/app/update/memory.rs` |
| GC policy | `src/core/maintenance.rs` |
| Cover budget | `src/utils/covers.rs` (`COVER_BUDGET_MAX_BYTES`) |
| Cover store ledger | `src/gui/library/cover_cache.rs` |
| Interner cap | `src/utils/interner.rs` |

---

## 8. Navigation footprint and the cover-store trim (2026-11-18)

The budget above measured the engine and the cover store separately. This section records
what the whole process does while a reader browses and then settles, which is the state a
desktop player spends its life in, and the change that brought it down.

### The retention that showed up after browsing

Scrolling the library moved the process from 204.2 MiB to a 256.8 MiB peak, and eight
minutes after the scrolling stopped the 60 s purge had only returned it to 247.7 MiB. The
cause was a policy, not an allocation: the purge trimmed the decoded-cover store **only
when the library was out of focus**, so a reader who scrolled and then stayed in the library
kept the prefetch band and the scroll-back cushion resident indefinitely.

### The trim now follows inactivity, not focus

`handle_memory` trims the decoded-cover store when either of these holds:

- the library is out of focus, as before; or
- the library is focused and has not scrolled for `LIBRARY_COVER_TRIM_IDLE_SECS` (5 s).

`LibraryManager::cover_trim_is_safe` authorises that second case and refuses under two
conditions. It refuses while the last scroll is younger than the threshold, because the
window moves on every scroll event and trimming behind it turns a scroll back into a
re-decode burst. And it refuses when the window is empty, which is the state before the
first layout: `is_window` answers `false` for every path there, so a trim would drop the
whole store instead of only the off-screen entries. That guard also protects the
out-of-focus path, which previously had no such check.

The visible window is never trimmed in either case — the budget protects its entries and
`evict_to_budget` stops when only window entries remain — so a cover on screen cannot be
released. An entry dropped outside the window is re-decoded asynchronously in 1–3 ms when
it scrolls back into view, and the cell draws its placeholder for that interval.

The next track's cover is unaffected: the 15 s preload builds an
`iced::widget::image::Handle::from_path` and stores it in
`player_ui_state.prefetched_next_handle`, which is a separate field outside the cover
store. The trim never deletes a file either; the orphan sweep is a different path and only
removes files no row references.

### Measured, same four-step sequence before and after

| Moment | Before the trim | After the trim |
|---|---:|---:|
| `startup` | 94.6 MiB | 94.5 MiB |
| First purge, library loaded | 203.8 MiB | 204.2 MiB |
| Peak while scrolling | 256.8 MiB | 241.5 MiB |
| Purge after the scrolling stops | 247.7 MiB (8 min later) | **213.7 MiB (10 s later)** |

The peak still rises while scrolling, which is the store filling as covers enter the
window; what changed is that the floor comes back. The 60 s purge cadence means the drop
lands within a minute of the reader stopping.

### No leak: the anonymous set is flat under playback

Six minutes of playback across four tracks, sampled once per purge:

| Purge | RSS | `Pss_Anon` |
|---|---:|---:|
| 1 | 205.7 MiB | 103.7 MiB |
| 2 | 211.4 MiB | 109.3 MiB |
| 3 | 211.4 MiB | 109.3 MiB |
| 4 | 211.4 MiB | 109.3 MiB |
| 5 | 212.7 MiB | 109.3 MiB |
| 6 | 215.5 MiB | 109.3 MiB |
| 7 | 219.0 MiB | 109.3 MiB |

`Pss_Anon` rises once (+5.6 MiB) when playback first starts — the ring at the device rate,
the DSP state and the preload head — and then stays flat for five and a half minutes while
RSS continues to climb by 7.6 MiB. Growth with a flat anonymous set is file-backed, and the
connection pragmas bound it: `cache_size = -8000` (7.8 MB per connection, two connections)
and `mmap_size = 16777216` (16 MiB window) over a 34.6 MB database. That is page cache the
system can reclaim, not heap that the process holds.

**Current peak: ~219 MiB** against the ~250 MiB recorded above, and the sustained floor
after browsing is ~214 MiB instead of ~248 MiB.

### How the numbers were taken

The process samples `Rss`, `Pss`, `Pss_Anon` and `Private_Dirty` from
`/proc/self/smaps_rollup` at four attribution points and emits one structured event per
sample under the `audoxidy::memory` target (`src/utils/rss.rs`):

| Label | Emitted from | What it separates |
|---|---|---|
| `startup` | the end of `AudoxidyApp::new` | engine + database + first library load |
| `gc` | after the global purge | what the purge returned against what stayed |
| `window` | window geometry persistence | viewport reflow and cover re-decode |
| `scroll` | `Message::LibraryScroll` | the cover store against iced's textures |

The scroll and window labels are rate-limited to one sample every 3 s and 2 s, so a drag or
a fast scroll cannot flood the log, and reading one sample costs a file read.

To reproduce: run a development build, note `startup`, wait for the first `gc`, scroll the
library to the end and back, stop and let one purge run, then drag and resize the window and
play a few tracks. Attribute each delta between consecutive samples to the holder the label
names.
