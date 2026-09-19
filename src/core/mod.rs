//! UI-agnostic domain layer for Audoxidy.
//!
//! This module is the boundary between the application's orchestration and its
//! front-ends. It owns policy over the audio manager, the database and the
//! library scanner: persistence ordering, engine call sequencing, and the shared
//! domain state guarded by the documented lock order. It exposes synchronous,
//! non-blocking commands returning plain data or a typed core error, plus an
//! event stream for edge transitions.
//!
//! Nothing here depends on a front-end or on the GUI toolkit, and no file under
//! this directory may import either. Each front-end owns its own presentation
//! state — selection, focus, scroll offset, marquee, hover and dialogs — so the
//! hot read paths stay synchronous and free of cross-boundary lock contention.
//!
//! # Integration surface
//!
//! [`Core`] is constructed once with injected filesystem paths and shared Arc
//! handles, then handed to a front-end. Any front-end — the desktop GUI, a
//! future terminal UI, or a mobile shell — integrates through the same surface:
//!
//! - **Construction** — [`Core::new`] takes the audio manager, database and
//!   scanner handles plus [`CorePaths`]; [`Core::paths`] returns the injected
//!   locations. Paths default to [`CorePaths::desktop_default`].
//! - **Commands** — synchronous methods grouped by concern: [`playback`] for
//!   transport, load and crossfade; [`library`] for scan, query, delete and
//!   stats; [`playlist`] for playlist state; [`settings`] for persisted audio,
//!   DSP and EQ-preset data; [`audio_routing`] for applying audio settings and
//!   EQ presets to the engine; [`covers`] for cover-path policy; and
//!   [`maintenance`] for GC policy. Commands return plain data or [`CoreError`]
//!   and never block the caller.
//! - **Events** — [`Core::event_receiver`] exposes the [`CoreEvent`] receiver
//!   and [`Core::emit`] publishes to it. The stream carries edge transitions
//!   (scan lifecycle, library dirty, track change, memory pressure) over a
//!   bounded channel; a front-end bridges it into its own runtime. The desktop
//!   shell uses a hash-stable subscription, the same shape as its external
//!   media-control bridge.
//! - **State ownership** — presentation state belongs to the front-end; Core
//!   holds only domain state and mutates it under the documented lock order.
//!
//! The boundary is enforced by the test-only guard module below, which fails the
//! suite if a file under this directory imports the GUI toolkit or the GUI
//! module, or if a code comment carries a planning identifier.

pub mod audio_routing;
pub mod context;
pub mod covers;
pub mod error;
pub mod events;
pub mod library;
pub mod maintenance;
pub mod playback;
pub mod playlist;
pub mod settings;
pub mod types;

pub use context::Core;
pub use error::CoreError;
pub use events::{CoreEvent, core_event_channel};
pub use maintenance::CoreIntent;
pub use playback::NextSongInfo;
pub use types::{CorePaths, EqPresetSnapshot};

#[cfg(test)]
mod guard_tests;
