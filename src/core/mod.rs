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
//! No behavior lives here yet; the submodule seams are introduced as the engine,
//! database and scan concerns move behind this boundary. The planned seams,
//! documented here only, are:
//!
//! - an injected filesystem path bundle, replacing hardcoded locations;
//! - the synchronous command surface over the audio manager and the database;
//! - the edge-transition event stream consumed by a front-end; and
//! - the shared domain state, mutated only under the documented lock order.
//!
//! The boundary is enforced by the test-only guard module below, which fails the
//! suite if a file under this directory imports the GUI toolkit or the GUI
//! module, or if a code comment carries a planning identifier.

pub mod context;
pub mod error;
pub mod events;
pub mod library;
pub mod playback;
pub mod settings;
pub mod types;

pub use context::Core;
pub use error::CoreError;
pub use events::{CoreEvent, core_event_channel};
pub use playback::NextSongInfo;
pub use types::{CorePaths, EqPresetSnapshot};

#[cfg(test)]
mod guard_tests;
