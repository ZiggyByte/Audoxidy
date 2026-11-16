//! Preload send tests: the opened preload chain must stay `Send`, so the decoder
//! thread can hand it to the playback side without moving the whole decoder.
//!
//! The test exercises a decoder internal without opening an output device.

use super::*;

#[test]
fn test_opened_preload_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<OpenedPreload>();
}
