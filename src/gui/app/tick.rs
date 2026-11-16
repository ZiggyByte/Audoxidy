//! The authorized tick taxonomy and the pure scheduling policy.
//!
//! This module is the single source of truth for the application's periodic
//! cadences. It declares the [`TickKind`] enumeration, the named period
//! constants, the pure [`tick_policy`] decision, and the [`tick`] subscription
//! builder. Keeping the decision separate from the subscription wiring means the
//! schedule can be reasoned about and tested without an iced runtime, and every
//! timer gets an explicit recipe identity instead of relying on its duration or
//! its mapper closure.
//!
//! | Kind | Period | Gate |
//! |------|--------|------|
//! | [`TickKind::Frame`] | 33 ms | playing, meter fading out, dragging, or a stepper held |
//! | [`TickKind::Ui`] | 500 ms | player chrome active, or playing with a marquee |
//! | [`TickKind::Transport`] | 500 ms | playing |
//! | [`TickKind::Scan`] | 500 ms | scanning while not playing |
//! | [`TickKind::Idle`] | 5000 ms | neither playing nor scanning |

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use super::Message;

// ── Fire instrumentation ──────────────────────────────────────────
//
// One process-global counter per tick kind. A zero counter after a scenario
// that should have fired a kind is the signature of a tick whose subscription
// recipe iced dropped, so the counters are read after a known scenario to
// verify the cadence. The increments are relaxed: the counters order nothing,
// they only make each cadence observable.
static FRAME_TICK_FIRES: AtomicU64 = AtomicU64::new(0);
static UI_TICK_FIRES: AtomicU64 = AtomicU64::new(0);
static TRANSPORT_TICK_FIRES: AtomicU64 = AtomicU64::new(0);
static SCAN_TICK_FIRES: AtomicU64 = AtomicU64::new(0);
static IDLE_TICK_FIRES: AtomicU64 = AtomicU64::new(0);

/// Snapshot of every tick kind's fire counter.
///
/// Read after a known scenario to show which cadence actually ran; the
/// counters are process-global, so callers compare two snapshots rather than
/// trusting an absolute value.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::gui::app) struct TickFires {
    /// Fires of the frame cadence.
    pub(in crate::gui::app) frame: u64,
    /// Fires of the UI cadence.
    pub(in crate::gui::app) ui: u64,
    /// Fires of the transport cadence.
    pub(in crate::gui::app) transport: u64,
    /// Fires of the scan cadence.
    pub(in crate::gui::app) scan: u64,
    /// Fires of the idle cadence.
    pub(in crate::gui::app) idle: u64,
}

/// Reads every tick kind's fire counter.
pub(in crate::gui::app) fn tick_fires() -> TickFires {
    let [frame, ui, transport, scan, idle] = TickKind::all_five().map(TickKind::fires);
    TickFires {
        frame,
        ui,
        transport,
        scan,
        idle,
    }
}

/// Identifies a periodic schedule and, together with its period, its recipe.
///
/// The `Hash`, `PartialEq` and `Eq` derives are load-bearing: the kind is hashed
/// into the subscription recipe, so two ticks that share a period still carry
/// distinct identities and iced cannot collapse them into one stream.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub(in crate::gui::app) enum TickKind {
    /// Animation and redraw cadence: meter ballistics, drag auto-scroll and
    /// stepper hold-to-repeat.
    Frame,
    /// Player chrome activity: marquee scrolling and the activity window that
    /// keeps the controls visible.
    Ui,
    /// Playback transport: preload trigger, end-of-track advance, art sync and
    /// progress persistence.
    Transport,
    /// Library scan: the dirty poll and the library snapshot refresh.
    Scan,
    /// Idle maintenance: garbage-collection policy, memory probes and metrics.
    Idle,
}

/// Period of the frame cadence: the meter, drag auto-scroll and stepper redraw.
///
/// This single value replaces the previous 30 ms meter, 32 ms drag and 30 ms
/// stepper-hold cadences. The per-tick meter ballistics and the drag auto-scroll
/// coefficients are deliberately **not** time-normalized to the new period: the
/// small visual delta they produce is accepted in exchange for one uniform
/// cadence.
pub(in crate::gui::app) const FRAME_TICK_PERIOD: Duration = Duration::from_millis(33);

/// Period of the UI cadence: marquee scrolling and the chrome activity window.
pub(in crate::gui::app) const UI_TICK_PERIOD: Duration = Duration::from_millis(500);

/// Period of the transport cadence while playing. This value is audio-adjacent
/// and must not change: the preload trigger and the end-of-track advance are
/// polled on it.
pub(in crate::gui::app) const TRANSPORT_TICK_PERIOD: Duration = Duration::from_millis(500);

/// Period of the scan cadence while a library scan runs and playback is stopped.
pub(in crate::gui::app) const SCAN_TICK_PERIOD: Duration = Duration::from_millis(500);

/// Period of the idle cadence: garbage-collection policy, memory probes and
/// metrics while neither playback nor a scan is active.
pub(in crate::gui::app) const IDLE_TICK_PERIOD: Duration = Duration::from_millis(5000);

/// The set of tick kinds that are active for a given application state.
///
/// Produced by [`tick_policy`] and consumed by the subscription builder that
/// declares the live timers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::gui::app) struct TickSet {
    /// Whether the frame cadence runs.
    pub(in crate::gui::app) frame: bool,
    /// Whether the UI cadence runs.
    pub(in crate::gui::app) ui: bool,
    /// Whether the transport cadence runs.
    pub(in crate::gui::app) transport: bool,
    /// Whether the scan cadence runs.
    pub(in crate::gui::app) scan: bool,
    /// Whether the idle cadence runs.
    pub(in crate::gui::app) idle: bool,
}

impl TickSet {
    /// Lists the active kinds paired with their period, in the canonical order
    /// frame, ui, transport, scan, idle.
    pub(in crate::gui::app) fn active_kinds(&self) -> Vec<(TickKind, Duration)> {
        let mut active = Vec::new();
        if self.frame {
            active.push((TickKind::Frame, TickKind::Frame.cadence()));
        }
        if self.ui {
            active.push((TickKind::Ui, TickKind::Ui.cadence()));
        }
        if self.transport {
            active.push((TickKind::Transport, TickKind::Transport.cadence()));
        }
        if self.scan {
            active.push((TickKind::Scan, TickKind::Scan.cadence()));
        }
        if self.idle {
            active.push((TickKind::Idle, TickKind::Idle.cadence()));
        }
        active
    }
}

/// Decides which tick kinds are active for a given application state.
///
/// The resulting set reproduces the previous base schedule exactly: a 500 ms
/// cadence while playing or scanning, and a 5000 ms cadence otherwise. The scan
/// cadence is suppressed while playing because the transport cadence already
/// polls the library-dirty flag at 500 ms; running both would repeat the shared
/// tick prelude twice per tick.
pub(in crate::gui::app) fn tick_policy(
    is_playing: bool,
    is_scanning: bool,
    ui_active: bool,
    needs_marquee: bool,
    is_dragging: bool,
    meter_fading: bool,
    stepper_held: bool,
) -> TickSet {
    TickSet {
        frame: is_playing || meter_fading || is_dragging || stepper_held,
        ui: ui_active || (is_playing && needs_marquee),
        transport: is_playing,
        scan: is_scanning && !is_playing,
        idle: !is_playing && !is_scanning,
    }
}

impl TickKind {
    /// The authorized period for this kind.
    pub(in crate::gui::app) fn cadence(self) -> Duration {
        match self {
            TickKind::Frame => FRAME_TICK_PERIOD,
            TickKind::Ui => UI_TICK_PERIOD,
            TickKind::Transport => TRANSPORT_TICK_PERIOD,
            TickKind::Scan => SCAN_TICK_PERIOD,
            TickKind::Idle => IDLE_TICK_PERIOD,
        }
    }

    /// The message this kind's timer emits.
    pub(in crate::gui::app) fn message(self) -> Message {
        match self {
            TickKind::Frame => Message::FrameTick,
            TickKind::Ui => Message::UiTick,
            TickKind::Transport => Message::TransportTick,
            TickKind::Scan => Message::ScanTick,
            TickKind::Idle => Message::IdleTick,
        }
    }

    /// Every kind, in the canonical order frame, ui, transport, scan, idle.
    ///
    /// Used to enumerate the counters without repeating the variant list at
    /// each call site.
    pub(in crate::gui::app) fn all_five() -> [TickKind; 5] {
        [
            TickKind::Frame,
            TickKind::Ui,
            TickKind::Transport,
            TickKind::Scan,
            TickKind::Idle,
        ]
    }

    /// Records one fire of this kind on its own counter.
    ///
    /// A zero counter after a scenario that should have fired this kind is the
    /// signature of a tick whose recipe was dropped by iced, so the counters
    /// must be read after a known scenario to verify the cadence. Recording one
    /// kind never changes another kind's counter.
    pub(in crate::gui::app) fn record_fire(self) {
        self.counter().fetch_add(1, Ordering::Relaxed);
    }

    /// Reads this kind's fire counter.
    pub(in crate::gui::app) fn fires(self) -> u64 {
        self.counter().load(Ordering::Relaxed)
    }

    /// The counter owned by this kind.
    fn counter(self) -> &'static AtomicU64 {
        match self {
            TickKind::Frame => &FRAME_TICK_FIRES,
            TickKind::Ui => &UI_TICK_FIRES,
            TickKind::Transport => &TRANSPORT_TICK_FIRES,
            TickKind::Scan => &SCAN_TICK_FIRES,
            TickKind::Idle => &IDLE_TICK_FIRES,
        }
    }
}

/// Builds the recipe identity of a tick: the explicit period/kind pair.
///
/// This pair is the subscription recipe data, so two ticks that share a period
/// but differ in kind keep distinct identities and iced can never deduplicate
/// them by duration alone. Identity therefore no longer depends on the mapper
/// closure that turns a fire into a message.
pub(in crate::gui::app) fn tick_identity(kind: TickKind, period: Duration) -> (Duration, TickKind) {
    (period, kind)
}

/// Builds the timer subscription for one tick kind.
///
/// The stream mirrors the standard iced interval timer: the first message
/// arrives after `period`, and missed ticks are skipped rather than replayed, so
/// a stalled executor cannot produce a catch-up burst of heavy work. That keeps
/// the transport cadence exact.
pub(in crate::gui::app) fn tick(kind: TickKind, period: Duration) -> iced::Subscription<Message> {
    // The recipe data is the explicit `(period, kind)` identity: two ticks that
    // share a duration but differ in kind stay distinct streams, so identity no
    // longer depends on the mapper closure that turns a fire into a message.
    let (period, kind) = tick_identity(kind, period);

    iced::Subscription::run_with((period, kind), |&(period, kind)| {
        use iced::futures::StreamExt;

        let start = tokio::time::Instant::now() + period;
        let mut interval = tokio::time::interval_at(start, period);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        iced::futures::stream::unfold(interval, move |mut interval| async move {
            let _ = interval.tick().await;
            Some((kind.message(), interval))
        })
        .boxed()
    })
}

#[cfg(test)]
#[path = "tick_tests.rs"]
mod tests;
