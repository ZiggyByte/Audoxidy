//! Tests for the Core event channel and the events it carries.
//!
//! They pin the bounded capacity a slow consumer relies on, the round trip of
//! the edge events through the channel pair, and the `Send + Clone` bounds the
//! front-end bridge needs. Producers and consumers are the channel ends
//! themselves, so no runtime, subscription or GUI type is involved.

use super::*;

#[test]
fn test_core_event_channel_round_trip() {
    let (tx, rx) = core_event_channel();
    tx.send(CoreEvent::ScanStarted).unwrap();
    tx.send(CoreEvent::ScanProgress {
        files_processed: 42,
    })
    .unwrap();
    tx.send(CoreEvent::LibraryDirty).unwrap();

    assert!(matches!(rx.recv().unwrap(), CoreEvent::ScanStarted));
    assert!(matches!(
        rx.recv().unwrap(),
        CoreEvent::ScanProgress {
            files_processed: 42
        }
    ));
    assert!(matches!(rx.recv().unwrap(), CoreEvent::LibraryDirty));
}

#[test]
fn test_core_event_is_send_and_clone() {
    fn assert_send<T: Send>() {}
    fn assert_clone<T: Clone>() {}
    assert_send::<CoreEvent>();
    assert_clone::<CoreEvent>();
}

#[test]
fn test_core_event_channel_bounded_capacity() {
    let (tx, _rx) = core_event_channel();
    // The channel is bounded(64). After sending 64 events, the 65th
    // should block (we test the non-blocking path by using try_send).
    for _ in 0..64 {
        tx.try_send(CoreEvent::LibraryDirty).unwrap();
    }
    assert!(tx.try_send(CoreEvent::LibraryDirty).is_err());
}
