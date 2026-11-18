//! Tests for the resident-set diagnostic: the `smaps_rollup` parser is the only
//! logic worth pinning, because a silent parse failure would log zeros and make
//! every sample look like a reclaimed process.
//!
//! The whole module is Linux-only: `field_kb` has no implementation elsewhere.
#![cfg(target_os = "linux")]

use super::field_kb;

const SAMPLE: &str = "\
55f0e1a00000-7fff00000000 ---p 00000000 00:00 0                          [rollup]
Rss:               281456 kB
Pss:               174902 kB
Pss_Anon:          120388 kB
Pss_File:           54514 kB
Private_Dirty:      98120 kB
";

#[test]
fn reads_each_field_from_a_rollup_sample() {
    assert_eq!(field_kb(SAMPLE, "Rss:"), 281_456);
    assert_eq!(field_kb(SAMPLE, "Pss:"), 174_902);
    assert_eq!(field_kb(SAMPLE, "Pss_Anon:"), 120_388);
    assert_eq!(field_kb(SAMPLE, "Private_Dirty:"), 98_120);
}

#[test]
fn reads_a_field_absent_from_the_sample_as_zero() {
    assert_eq!(field_kb(SAMPLE, "Swap:"), 0);
    assert_eq!(field_kb("", "Rss:"), 0);
}

#[test]
fn reads_a_malformed_value_as_zero_instead_of_panicking() {
    assert_eq!(field_kb("Rss:       not-a-number kB\n", "Rss:"), 0);
    assert_eq!(field_kb("Rss:\n", "Rss:"), 0);
}
