//! Published playback identity and cover URL tests.
//!
//! The playback key and the position bucket are pure values, and the cover URL is
//! resolved from a private directory the test owns. Nothing here publishes to a
//! real desktop media panel: the panel is only reached through the production
//! seam, which these tests never construct.

use crate::utils::test_support::TempTestDir;

#[test]
fn cover_file_url() {
    // Se usa `super::` porque la función de test comparte nombre con el
    // helper y lo ocultaría en el ámbito del módulo.
    assert_eq!(super::cover_file_url(""), None);
    assert_eq!(
        super::cover_file_url("cache/covers/does-not-exist.avif"),
        None
    );

    let dir = TempTestDir::new("media-controls-cover");
    let file = dir.path().join("audoxidy_cover.avif");
    std::fs::write(&file, b"x").unwrap();
    let url = super::cover_file_url(file.to_str().unwrap()).unwrap();
    assert!(url.starts_with("file://"));
    assert!(url.ends_with(".avif"));
}

#[test]
fn position_bucket_truncates_seconds() {
    assert_eq!(super::position_bucket_secs(12.99), 12);
    assert_eq!(super::position_bucket_secs(13.0), 13);
}

#[test]
fn position_bucket_clamps_non_positive() {
    assert_eq!(super::position_bucket_secs(-5.0), 0);
    assert_eq!(super::position_bucket_secs(0.0), 0);
}

#[test]
fn position_bucket_handles_non_finite() {
    assert_eq!(super::position_bucket_secs(f64::NAN), 0);
    assert_eq!(super::position_bucket_secs(f64::INFINITY), 0);
}

#[test]
fn playback_publish_is_change_driven() {
    let base: super::PlaybackKey = (
        true,
        12,
        "Title".to_string(),
        "Artist".to_string(),
        "Album".to_string(),
        None,
    );

    assert!(super::playback_should_publish(None, &base));
    assert!(!super::playback_should_publish(Some(&base), &base));

    let mut changed_playing = base.clone();
    changed_playing.0 = false;
    assert!(super::playback_should_publish(
        Some(&base),
        &changed_playing
    ));

    let mut changed_bucket = base.clone();
    changed_bucket.1 = 13;
    assert!(super::playback_should_publish(Some(&base), &changed_bucket));

    let mut changed_track = base.clone();
    changed_track.2 = "Other".to_string();
    assert!(super::playback_should_publish(Some(&base), &changed_track));
}
