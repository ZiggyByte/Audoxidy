//! Preload buffer tests: promoting the predecoded head into the pending queue,
//! and reusing the drained buffer for the next preload, must not lose or copy a
//! sample.
//!
//! The tests exercise the decoder's private buffer exchange without opening an
//! output device.

use super::*;
use std::collections::VecDeque;

#[test]
fn test_promotion_moves_the_head_without_copying_or_losing_samples() {
    let mut predecode: VecDeque<f64> = (0..8).map(|value| value as f64).collect();
    let mut pending: VecDeque<f64> = VecDeque::new();

    promote_preload_head(&mut pending, &mut predecode);

    assert_eq!(pending.len(), 8);
    assert_eq!(pending.front().copied(), Some(0.0));
    assert_eq!(pending.back().copied(), Some(7.0));
    assert!(
        predecode.is_empty(),
        "el buffer queda listo para reutilizarse"
    );
}

#[test]
fn test_promotion_appends_when_the_pending_queue_still_has_data() {
    let mut predecode: VecDeque<f64> = vec![1.0, 2.0].into();
    let mut pending: VecDeque<f64> = vec![9.0].into();

    promote_preload_head(&mut pending, &mut predecode);

    assert_eq!(
        pending.iter().copied().collect::<Vec<_>>(),
        vec![9.0, 1.0, 2.0]
    );
    assert!(predecode.is_empty());
}

#[test]
fn test_drained_pending_buffer_is_reused_by_the_next_preload() {
    let mut pending: VecDeque<f64> = VecDeque::with_capacity(64);
    let mut predecode: VecDeque<f64> = VecDeque::new();
    let reusable = pending.capacity();

    reuse_drained_preload_buffer(&mut pending, &mut predecode);

    assert_eq!(predecode.capacity(), reusable);
    assert!(predecode.is_empty());
    assert_eq!(pending.capacity(), 0);
}
