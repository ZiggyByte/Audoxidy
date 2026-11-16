//! Preload handoff tests: a decoded batch appended to the preload head must be
//! moved whole, never truncated at the soft cap and never dropped when the head
//! is already full.
//!
//! The tests exercise the decoder's private buffer handoff without opening an
//! output device: they feed plain sample vectors into the helpers the promotion
//! path uses, so the contract is pinned on any machine.

use super::*;

#[test]
fn test_preload_batch_is_never_truncated_at_the_cap() {
    let cap = 100_usize;
    let mut head: std::collections::VecDeque<f64> =
        (0..cap - 10).map(|value| value as f64).collect();
    let mut batch: Vec<f64> = (1_000..1_100).map(|value| value as f64).collect();

    let appended = append_preload_batch(&mut head, &mut batch);

    assert_eq!(appended, 100);
    assert!(
        head.len() >= cap,
        "the last batch may overshoot the soft cap"
    );
    assert_eq!(head.len(), cap - 10 + 100);
    assert_eq!(head.back().copied(), Some(1_099.0));
    assert!(
        batch.is_empty(),
        "the whole batch must be moved, never dropped"
    );
}

#[test]
fn test_a_full_head_never_loses_the_batch_tail() {
    let mut head: std::collections::VecDeque<f64> = (0..10).map(|value| value as f64).collect();
    let mut batch: Vec<f64> = (10..20).map(|value| value as f64).collect();

    assert_eq!(append_preload_batch(&mut head, &mut batch), 10);
    assert_eq!(head.len(), 20);
    assert_eq!(head.front().copied(), Some(0.0));
    assert_eq!(head.back().copied(), Some(19.0));
}
