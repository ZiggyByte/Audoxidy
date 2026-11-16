//! Shell tests for the resize handles along the window edges.
//!
//! They pin the direction set, the stacking order that keeps the corner strips
//! above the edge strips, the axis each direction stretches, the cursor
//! interaction it reports and its fixed extent. No widget tree is built for
//! them.

use super::*;
use iced::mouse::Interaction;
use iced::window::Direction;

/// `Direction` has no `PartialEq`, so compare by discriminant.
fn same(a: Direction, b: Direction) -> bool {
    std::mem::discriminant(&a) == std::mem::discriminant(&b)
}

/// A corner is a direction that stretches both axes.
fn is_corner(direction: Direction) -> bool {
    resize_axis(direction) == ResizeAxis::Both
}

const ALL: [Direction; 8] = [
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
    Direction::NorthWest,
    Direction::NorthEast,
    Direction::SouthWest,
    Direction::SouthEast,
];

#[test]
fn every_resize_direction_appears_exactly_once() {
    assert_eq!(RESIZE_HANDLE_DIRECTIONS.len(), ALL.len());
    for expected in ALL {
        let count = RESIZE_HANDLE_DIRECTIONS
            .iter()
            .filter(|direction| same(**direction, expected))
            .count();
        assert_eq!(count, 1, "direction {expected:?} must appear once");
    }
}

#[test]
fn corner_strips_stack_above_the_edges() {
    let last_edge = RESIZE_HANDLE_DIRECTIONS
        .iter()
        .rposition(|direction| !is_corner(*direction))
        .expect("there are edge strips");
    let first_corner = RESIZE_HANDLE_DIRECTIONS
        .iter()
        .position(|direction| is_corner(*direction))
        .expect("there are corner strips");

    assert!(
        last_edge < first_corner,
        "every corner must be stacked after every edge"
    );
}

#[test]
fn edge_and_corner_counts_are_four_each() {
    let corners = RESIZE_HANDLE_DIRECTIONS
        .iter()
        .filter(|direction| is_corner(**direction))
        .count();
    assert_eq!(corners, 4);
    assert_eq!(RESIZE_HANDLE_DIRECTIONS.len() - corners, 4);
}

#[test]
fn resize_axis_matches_direction() {
    assert_eq!(resize_axis(Direction::North), ResizeAxis::Vertical);
    assert_eq!(resize_axis(Direction::South), ResizeAxis::Vertical);
    assert_eq!(resize_axis(Direction::East), ResizeAxis::Horizontal);
    assert_eq!(resize_axis(Direction::West), ResizeAxis::Horizontal);
    for corner in [
        Direction::NorthWest,
        Direction::NorthEast,
        Direction::SouthWest,
        Direction::SouthEast,
    ] {
        assert_eq!(resize_axis(corner), ResizeAxis::Both);
    }
}

#[test]
fn resize_interaction_matches_direction() {
    assert_eq!(
        resize_interaction(Direction::North),
        Interaction::ResizingVertically
    );
    assert_eq!(
        resize_interaction(Direction::South),
        Interaction::ResizingVertically
    );
    assert_eq!(
        resize_interaction(Direction::East),
        Interaction::ResizingHorizontally
    );
    assert_eq!(
        resize_interaction(Direction::West),
        Interaction::ResizingHorizontally
    );
    assert_eq!(
        resize_interaction(Direction::NorthWest),
        Interaction::ResizingDiagonallyDown
    );
    assert_eq!(
        resize_interaction(Direction::SouthEast),
        Interaction::ResizingDiagonallyDown
    );
    assert_eq!(
        resize_interaction(Direction::NorthEast),
        Interaction::ResizingDiagonallyUp
    );
    assert_eq!(
        resize_interaction(Direction::SouthWest),
        Interaction::ResizingDiagonallyUp
    );
}

#[test]
fn resize_strip_extent_fixes_only_the_perpendicular_axis() {
    let hit = iced::Length::Fixed(RESIZE_HANDLE_HIT);

    assert_eq!(
        resize_strip_extent(Direction::North),
        (iced::Length::Fill, hit)
    );
    assert_eq!(
        resize_strip_extent(Direction::West),
        (hit, iced::Length::Fill)
    );
    let square = resize_strip_extent(Direction::SouthEast);
    assert_eq!(square.0, hit);
    assert_eq!(square.1, hit);
}

#[test]
fn resize_strip_alignment_pins_each_edge() {
    assert_eq!(
        resize_strip_alignment(Direction::North),
        (iced::Alignment::Center, iced::Alignment::Start)
    );
    assert_eq!(
        resize_strip_alignment(Direction::SouthEast),
        (iced::Alignment::End, iced::Alignment::End)
    );
    assert_eq!(
        resize_strip_alignment(Direction::NorthWest),
        (iced::Alignment::Start, iced::Alignment::Start)
    );
}
