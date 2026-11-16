//! External control channel tests.
//!
//! The tests exercise the sender, the receiver identity and the event payload as
//! values: no D-Bus connection, no media panel and no audio device is opened, so
//! the channel contract that the bridge threads rely on is asserted in isolation.

use super::*;

#[test]
fn channel_round_trip() {
    let (tx, rx) = external_control_channel();
    tx.send(ExternalControlEvent::Next);
    assert_eq!(rx.recv().ok(), Some(ExternalControlEvent::Next));

    // Sin emisor el receptor queda desconectado: el siguiente recv es Err.
    drop(tx);
    assert!(rx.recv().is_err());
}

#[test]
fn event_is_send_and_clone() {
    let event = ExternalControlEvent::SeekTo(12.5);
    let clone = event;
    let handle = std::thread::spawn(move || clone);
    assert_eq!(handle.join().ok(), Some(ExternalControlEvent::SeekTo(12.5)));
}

#[test]
fn receiver_hash_is_stable() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let (_tx_a, rx_a) = crossbeam::channel::unbounded();
    let (_tx_b, rx_b) = crossbeam::channel::unbounded();

    let mut hasher_clone = DefaultHasher::new();
    ExternalControlReceiver(rx_a.clone()).hash(&mut hasher_clone);
    let mut hasher_same = DefaultHasher::new();
    ExternalControlReceiver(rx_a).hash(&mut hasher_same);
    let mut hasher_other = DefaultHasher::new();
    ExternalControlReceiver(rx_b).hash(&mut hasher_other);

    assert_eq!(hasher_clone.finish(), hasher_same.finish());
    assert_eq!(hasher_clone.finish(), hasher_other.finish());
}
