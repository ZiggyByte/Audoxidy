//! Resident-set diagnostic for attributing memory growth.
//!
//! The budget this measures against lives in `docs/MEMORY-ANALYSIS-2026-11-14.md`:
//! the audio ring and preload buffers, the decoded-cover store, iced's viewport
//! textures, SQLite's page cache and the library snapshot are the only holders
//! large enough to move the number by tens of megabytes. A sample is logged at
//! the moments that separate those holders, so a delta between two samples names
//! its owner instead of only its size.
//!
//! On Linux each sample reads `/proc/self/smaps_rollup`, which the kernel keeps
//! up to date without walking the page tables, so the cost is a file read. On
//! other platforms every entry point is a no-op.

use crate::utils::observability::MEMORY_TARGET;

/// Minimum seconds between two scroll samples. Scrolling is the highest-frequency
/// caller and a burst of events would otherwise flood the log.
const SCROLL_INTERVAL_SECS: u64 = 3;

/// Minimum seconds between two window samples: dragging and resizing emit a
/// bounded number of events, but a slow drag can still produce several.
const WINDOW_INTERVAL_SECS: u64 = 2;

static LAST_SCROLL_SECS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static LAST_WINDOW_SECS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Logs the current resident set with a label.
///
/// The label is the attribution key: `startup`, `gc`, `window` and `scroll`
/// bracket the four states the budget distinguishes, so two consecutive samples
/// isolate what happened in between.
#[cfg(target_os = "linux")]
pub fn log_rss(label: &str) {
    let Ok(contents) = std::fs::read_to_string("/proc/self/smaps_rollup") else {
        return;
    };
    let rss = field_kb(&contents, "Rss:");
    let pss = field_kb(&contents, "Pss:");
    let pss_anon = field_kb(&contents, "Pss_Anon:");
    let private_dirty = field_kb(&contents, "Private_Dirty:");
    tracing::info!(
        target: MEMORY_TARGET,
        label,
        rss_kb = rss,
        pss_kb = pss,
        pss_anon_kb = pss_anon,
        private_dirty_kb = private_dirty,
        "RSS sample"
    );
}

/// Logs the current resident set with a label.
///
/// The platform has no `/proc` equivalent wired here, so the call is a no-op and
/// the log stays free of platform branches.
#[cfg(not(target_os = "linux"))]
pub fn log_rss(_label: &str) {}

/// Logs a sample after a library scroll, at most once every few seconds.
///
/// A scroll is where the cover store and iced's textures grow together, so this
/// is the sample that separates them from the audio buffers.
pub fn log_rss_on_scroll() {
    if throttle(&LAST_SCROLL_SECS, SCROLL_INTERVAL_SECS) {
        log_rss("scroll");
    }
}

/// Logs a sample after the window geometry is persisted, at most every couple of
/// seconds.
///
/// Returning from maximized or changing the window size reflows the viewport and
/// therefore re-decodes covers, which is the other path that grows those two
/// holders.
pub fn log_rss_on_window() {
    if throttle(&LAST_WINDOW_SECS, WINDOW_INTERVAL_SECS) {
        log_rss("window");
    }
}

/// Returns `true` when the caller is allowed to sample, and records the instant.
///
/// Uses whole seconds so the slot fits an `AtomicU64` without a lock; two
/// samples inside the same second collapse, which is the intent.
fn throttle(slot: &std::sync::atomic::AtomicU64, interval: u64) -> bool {
    let now = now_secs();
    let last = slot.load(std::sync::atomic::Ordering::Relaxed);
    if now.saturating_sub(last) < interval {
        return false;
    }
    slot.store(now, std::sync::atomic::Ordering::Relaxed);
    true
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// Reads a `Key:     1234 kB` line from `smaps_rollup`. Missing fields read as 0.
#[cfg(target_os = "linux")]
fn field_kb(contents: &str, key: &str) -> u64 {
    contents
        .lines()
        .find_map(|line| {
            line.strip_prefix(key)
                .and_then(|rest| rest.split_whitespace().next())
                .and_then(|value| value.parse::<u64>().ok())
        })
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "rss_tests.rs"]
mod tests;
