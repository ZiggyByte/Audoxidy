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

use std::time::Duration;

use super::Message;

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
mod tests {
    use super::*;

    #[test]
    fn test_policy_keeps_frame_and_transport_while_playing() {
        let playing = tick_policy(true, false, true, true, false, false, false);
        assert_eq!(
            playing,
            TickSet {
                frame: true,
                ui: true,
                transport: true,
                scan: false,
                idle: false,
            }
        );

        // A scan running alongside playback stays on the transport cadence only.
        let playing_and_scanning = tick_policy(true, true, true, true, false, false, false);
        assert_eq!(playing_and_scanning, playing);
    }

    #[test]
    fn test_policy_activates_scan_only_while_scanning() {
        let scanning = tick_policy(false, true, false, false, false, false, false);
        assert_eq!(
            scanning,
            TickSet {
                frame: false,
                ui: false,
                transport: false,
                scan: true,
                idle: false,
            }
        );
    }

    #[test]
    fn test_policy_suppresses_scan_while_playing() {
        let set = tick_policy(true, true, false, false, false, false, false);
        assert_eq!(
            set,
            TickSet {
                frame: true,
                ui: false,
                transport: true,
                scan: false,
                idle: false,
            }
        );
    }

    #[test]
    fn test_policy_idles_with_ui_activity_when_stopped() {
        let set = tick_policy(false, false, true, false, false, false, false);
        assert_eq!(
            set,
            TickSet {
                frame: false,
                ui: true,
                transport: false,
                scan: false,
                idle: true,
            }
        );
    }

    #[test]
    fn test_policy_marquee_needs_playback() {
        // A long title alone does not wake the UI cadence.
        let stopped = tick_policy(false, false, false, true, false, false, false);
        assert!(!stopped.ui);
        assert!(stopped.idle);

        // Playing with a long title does.
        let playing = tick_policy(true, false, false, true, false, false, false);
        assert!(playing.ui);
    }

    #[test]
    fn test_policy_meter_fade_keeps_frame_alive() {
        let set = tick_policy(false, false, false, false, false, true, false);
        assert!(set.frame);
        assert!(set.idle);
        assert!(!set.transport);
    }

    #[test]
    fn test_policy_drag_keeps_frame_alive() {
        let set = tick_policy(false, false, false, false, true, false, false);
        assert!(set.frame);
        assert!(set.idle);
    }

    #[test]
    fn test_policy_stepper_hold_keeps_frame_alive() {
        let set = tick_policy(false, false, false, false, false, false, true);
        assert!(set.frame);
        assert!(set.idle);
    }

    #[test]
    fn test_cadences_match_authorized_values() {
        assert_eq!(TickKind::Frame.cadence(), Duration::from_millis(33));
        assert_eq!(TickKind::Ui.cadence(), Duration::from_millis(500));
        assert_eq!(TickKind::Transport.cadence(), Duration::from_millis(500));
        assert_eq!(TickKind::Scan.cadence(), Duration::from_millis(500));
        assert_eq!(TickKind::Idle.cadence(), Duration::from_millis(5000));
    }

    #[test]
    fn test_each_kind_maps_to_its_own_message() {
        assert!(matches!(TickKind::Frame.message(), Message::FrameTick));
        assert!(matches!(TickKind::Ui.message(), Message::UiTick));
        assert!(matches!(
            TickKind::Transport.message(),
            Message::TransportTick
        ));
        assert!(matches!(TickKind::Scan.message(), Message::ScanTick));
        assert!(matches!(TickKind::Idle.message(), Message::IdleTick));
    }

    #[test]
    fn test_active_kinds_lists_kinds_in_canonical_order() {
        let set = tick_policy(true, false, true, true, false, false, false);
        assert_eq!(
            set.active_kinds(),
            vec![
                (TickKind::Frame, Duration::from_millis(33)),
                (TickKind::Ui, Duration::from_millis(500)),
                (TickKind::Transport, Duration::from_millis(500)),
            ]
        );
    }

    #[test]
    fn test_active_kinds_is_idle_only_when_nothing_else_is_due() {
        let set = tick_policy(false, false, false, false, false, false, false);
        assert_eq!(
            set.active_kinds(),
            vec![(TickKind::Idle, Duration::from_millis(5000))]
        );
    }

    #[test]
    fn test_tick_identity_distinguishes_kinds_for_equal_periods() {
        let period = Duration::from_millis(500);
        assert_ne!(
            tick_identity(TickKind::Ui, period),
            tick_identity(TickKind::Transport, period)
        );
        assert_ne!(
            tick_identity(TickKind::Transport, period),
            tick_identity(TickKind::Scan, period)
        );
        assert_ne!(
            tick_identity(TickKind::Ui, period),
            tick_identity(TickKind::Ui, Duration::from_millis(5000))
        );
        assert_eq!(
            tick_identity(TickKind::Ui, period),
            tick_identity(TickKind::Ui, period)
        );
    }
}
