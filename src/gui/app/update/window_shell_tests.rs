//! Shell tests for window move and resize geometry.
//!
//! They pin the drag threshold that starts a move, the off-screen restore
//! predicate, and the width a viewport re-anchor predicts before and after the
//! first layout. Every assertion reads pure values: no window is opened and no
//! renderer is started.

use super::*;

fn point(x: f32, y: f32) -> iced::Point {
    iced::Point::new(x, y)
}

#[test]
fn reanchor_uses_the_measured_width_after_a_layout() {
    let minimum = crate::gui::library::grid_metrics::GridMetrics::minimum();
    let predicted = predicted_reported_width(1200.0, minimum, 100.0);

    assert!((predicted - 1300.0).abs() < f32::EPSILON);

    let inferred = minimum.columns as f32 * minimum.card_width + ALBUM_SCROLLBAR_RAIL;
    assert!(
        predicted > inferred + 500.0,
        "la anchura medida no puede confundirse con la inversión del mínimo"
    );
}

#[test]
fn reanchor_falls_back_to_the_inverse_before_the_first_layout() {
    let minimum = crate::gui::library::grid_metrics::GridMetrics::minimum();
    let expected = minimum.columns as f32 * minimum.card_width + ALBUM_SCROLLBAR_RAIL + 50.0;

    let predicted = predicted_reported_width(0.0, minimum, 50.0);

    assert!((predicted - expected).abs() < 0.001);
}

#[test]
fn drag_threshold_ignores_short_clicks() {
    // Press and a tiny movement stay below the 4 px threshold.
    let origin = point(100.0, 100.0);
    assert!(!exceeds_drag_threshold(origin, point(102.0, 102.0)));
    // A purely horizontal move of 3 px is still under it.
    assert!(!exceeds_drag_threshold(origin, point(103.0, 100.0)));
}

#[test]
fn drag_threshold_is_strict_at_the_boundary() {
    let origin = point(0.0, 0.0);
    // Exactly the threshold distance squared (4 px) does not start the drag.
    assert!(!exceeds_drag_threshold(origin, point(4.0, 0.0)));
    // One pixel further does.
    assert!(exceeds_drag_threshold(origin, point(5.0, 0.0)));
}

#[test]
fn drag_threshold_handles_diagonals_and_both_directions() {
    let origin = point(50.0, 50.0);
    // Diagonal movement past the threshold in either direction starts it.
    assert!(exceeds_drag_threshold(origin, point(53.0, 53.0)));
    assert!(exceeds_drag_threshold(origin, point(47.0, 47.0)));
    assert!(exceeds_drag_threshold(origin, point(47.0, 53.0)));
}

#[test]
fn window_fully_off_screen_detects_every_outside_case() {
    let size = (800, 600);
    let monitor = iced::Size::new(1920.0, 1080.0);

    assert!(window_fully_off_screen(point(1920.0, 0.0), size, monitor));
    assert!(window_fully_off_screen(point(0.0, 1080.0), size, monitor));
    assert!(window_fully_off_screen(point(-800.0, 0.0), size, monitor));
    assert!(window_fully_off_screen(point(0.0, -600.0), size, monitor));
}

#[test]
fn window_fully_off_screen_keeps_partially_visible_windows() {
    let size = (800, 600);
    let monitor = iced::Size::new(1920.0, 1080.0);

    // Overlapping an edge by a single pixel is still restored as-is.
    assert!(!window_fully_off_screen(point(1919.0, 0.0), size, monitor));
    assert!(!window_fully_off_screen(point(-799.0, 0.0), size, monitor));
    assert!(!window_fully_off_screen(point(0.0, 1079.0), size, monitor));
    assert!(!window_fully_off_screen(point(0.0, -599.0), size, monitor));
    assert!(!window_fully_off_screen(point(100.0, 100.0), size, monitor));
}
