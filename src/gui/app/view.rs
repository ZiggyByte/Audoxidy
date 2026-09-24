//! Application view composition shell.
//!
//! Holds `AudoxidyApp::view` and declares the overlay submodules that
//! render the modal dialogs, the native context menu, and the VU-meter popup.

mod context_menu;
pub(in crate::gui::app) mod dialogs;
mod meter_popup;

use iced::Element;

use crate::gui::dimensions::PLAYLIST_PANEL_WIDTH;

use super::{AudoxidyApp, Message};

impl AudoxidyApp {
    pub fn view(&self) -> Element<'_, Message> {
        let player_view =
            crate::gui::player::view(self.core.audio_manager(), &self.player_ui_state);
        let playlist_view =
            crate::gui::playlist::view(&self.playlist_manager, self.core.audio_manager());
        let (filters_view, filters_menu_overlay) =
            crate::gui::library_filters::view(&self.filters_manager);
        let library_view = crate::gui::library::view(
            &self.library_manager,
            self.core.database(),
            &self.player_ui_state.current_art_id,
            &self.player_ui_state.meter,
            false, // clipping — TODO: wire from meter data
            self.core.audio_manager().state().read().meter_enabled,
        );

        // Apilamos el reproductor (carátula y controles) arriba de la playlist en una sola columna izquierda
        let left_column = iced::widget::column![player_view, playlist_view]
            .width(iced::Length::Shrink)
            .height(iced::Length::Fill)
            .align_x(iced::Alignment::Start);

        let main_row = iced::widget::row![left_column, filters_view, library_view]
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .align_y(iced::Alignment::Start);

        let final_content: Element<'_, Message> = {
            let mut stack = iced::widget::Stack::new().push(main_row);

            if self.audio_center_manager.open {
                let ac_view = crate::gui::audio_center::view(
                    &self.audio_center_manager,
                    self.core.audio_manager(),
                );

                // Contenedor posicionado o centrado para el Control Center
                let positioned_modal = if let Some(pos) = self.audio_center_manager.window_pos {
                    iced::widget::container(ac_view)
                        .width(iced::Length::Fill)
                        .height(iced::Length::Fill)
                        .align_x(iced::Alignment::Start)
                        .align_y(iced::Alignment::Start)
                        .padding(iced::Padding {
                            top: pos.y,
                            right: 0.0,
                            bottom: 0.0,
                            left: pos.x,
                        })
                } else {
                    iced::widget::container(ac_view)
                        .width(iced::Length::Fill)
                        .height(iced::Length::Fill)
                        .center_x(iced::Fill)
                        .center_y(iced::Fill)
                };

                stack = stack.push(positioned_modal);
            }
            stack
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
                .into()
        };

        let app_underlay = iced::widget::mouse_area(
            iced::widget::Space::new()
                .width(iced::Length::Fill)
                .height(iced::Length::Fill),
        )
        .on_press(Message::GlobalClick)
        .on_right_press(Message::GlobalClick)
        .interaction(iced::mouse::Interaction::Idle);

        let mut final_stack = iced::widget::Stack::new()
            .push(app_underlay)
            .push(final_content);

        // --- Capa Global para Menú de Filtros ---
        if let Some(menu) = filters_menu_overlay {
            let filters_underlay = iced::widget::mouse_area(
                iced::widget::Space::new()
                    .width(iced::Length::Fill)
                    .height(iced::Length::Fill),
            )
            .on_press(Message::GlobalClick)
            .on_right_press(Message::GlobalClick)
            .interaction(iced::mouse::Interaction::Idle);

            let positioned_menu = iced::widget::container(menu)
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
                .padding(iced::Padding {
                    top: 0.0,
                    bottom: 0.0,
                    left: PLAYLIST_PANEL_WIDTH,
                    right: 0.0,
                })
                .align_x(iced::alignment::Horizontal::Left)
                .align_y(iced::alignment::Vertical::Top);

            final_stack = final_stack.push(filters_underlay).push(positioned_menu);
        }

        // --- Capa de Diálogos Modales ---
        let mut final_stack = final_stack;

        if let Some(dialog_overlay) = self.dialog_overlay() {
            final_stack = final_stack.push(dialog_overlay);
        }

        if let Some(context_menu_overlay) = self.context_menu_overlay() {
            final_stack = final_stack.push(context_menu_overlay);
        }

        if let Some(meter_popup_overlay) = self.meter_popup_overlay() {
            final_stack = final_stack.push(meter_popup_overlay);
        }

        // El contenido va envuelto en `CursorOff`, que fuerza el cursor por
        // defecto e impide que los widgets clicables lo cambien. Las franjas de
        // redimensionado se apilan por encima y son lo único que cambia el
        // puntero, mostrando la flecha de redimensionado. Con la ventana
        // maximizada no se añaden, de modo que el redimensionado queda
        // desactivado por completo.
        let content = iced::widget::container(final_stack)
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .center_x(iced::Fill)
            .center_y(iced::Fill);

        let mut shell = iced::widget::Stack::new()
            .push(CursorOff::new(content, iced::mouse::Interaction::None));

        if !self.window_maximized {
            shell = shell.push(resize_handles());
        }

        shell
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .into()
    }
}

/// Thickness of the invisible resize strips, in logical pixels.
const RESIZE_HANDLE_HIT: f32 = 2.0;

/// The eight resize strips in stacking order: edges first, corners last.
///
/// Later entries sit above earlier ones, so the corners take the press before
/// the edges and the pointer shows the diagonal cursor in the overlap.
const RESIZE_HANDLE_DIRECTIONS: [iced::window::Direction; 8] = [
    iced::window::Direction::North,
    iced::window::Direction::South,
    iced::window::Direction::West,
    iced::window::Direction::East,
    iced::window::Direction::NorthWest,
    iced::window::Direction::NorthEast,
    iced::window::Direction::SouthWest,
    iced::window::Direction::SouthEast,
];

/// Which axis a resize direction stretches the window along.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResizeAxis {
    Horizontal,
    Vertical,
    Both,
}

/// Axis a resize direction stretches the window along.
///
/// A corner is exactly a direction whose axis is [`ResizeAxis::Both`].
fn resize_axis(direction: iced::window::Direction) -> ResizeAxis {
    use iced::window::Direction;
    match direction {
        Direction::North | Direction::South => ResizeAxis::Vertical,
        Direction::East | Direction::West => ResizeAxis::Horizontal,
        Direction::NorthWest
        | Direction::NorthEast
        | Direction::SouthWest
        | Direction::SouthEast => ResizeAxis::Both,
    }
}

/// Cursor interaction shown while hovering a resize strip.
///
/// The diagonal cursor points along the NW↔SE axis (`Down`) or the NE↔SW axis
/// (`Up`); edge strips show the matching single-axis cursor.
fn resize_interaction(direction: iced::window::Direction) -> iced::mouse::Interaction {
    use iced::mouse::Interaction;
    use iced::window::Direction;
    match direction {
        Direction::North | Direction::South => Interaction::ResizingVertically,
        Direction::East | Direction::West => Interaction::ResizingHorizontally,
        Direction::NorthWest | Direction::SouthEast => Interaction::ResizingDiagonallyDown,
        Direction::NorthEast | Direction::SouthWest => Interaction::ResizingDiagonallyUp,
    }
}

/// Size of a strip: the perpendicular axis is fixed to the hit thickness.
fn resize_strip_extent(direction: iced::window::Direction) -> (iced::Length, iced::Length) {
    match resize_axis(direction) {
        ResizeAxis::Vertical => (iced::Length::Fill, iced::Length::Fixed(RESIZE_HANDLE_HIT)),
        ResizeAxis::Horizontal => (iced::Length::Fixed(RESIZE_HANDLE_HIT), iced::Length::Fill),
        ResizeAxis::Both => (
            iced::Length::Fixed(RESIZE_HANDLE_HIT),
            iced::Length::Fixed(RESIZE_HANDLE_HIT),
        ),
    }
}

/// Alignment that pins the strip to its edge or corner of the window.
fn resize_strip_alignment(
    direction: iced::window::Direction,
) -> (iced::Alignment, iced::Alignment) {
    use iced::window::Direction;
    match direction {
        Direction::North => (iced::Alignment::Center, iced::Alignment::Start),
        Direction::South => (iced::Alignment::Center, iced::Alignment::End),
        Direction::West => (iced::Alignment::Start, iced::Alignment::Center),
        Direction::East => (iced::Alignment::End, iced::Alignment::Center),
        Direction::NorthWest => (iced::Alignment::Start, iced::Alignment::Start),
        Direction::NorthEast => (iced::Alignment::End, iced::Alignment::Start),
        Direction::SouthWest => (iced::Alignment::Start, iced::Alignment::End),
        Direction::SouthEast => (iced::Alignment::End, iced::Alignment::End),
    }
}

/// Transparent strips pinned to the window edges and corners.
///
/// Living on the top layer of the stack they intercept the press before the
/// drag bars and the scrollbars, show the resize cursor and trigger the
/// operating system's resize drag. The corners are stacked last so they take
/// priority over the edges.
fn resize_handles() -> Element<'static, Message> {
    use iced::widget::{Space, Stack, container, mouse_area};

    fn strip(direction: iced::window::Direction) -> Element<'static, Message> {
        let (width, height) = resize_strip_extent(direction);
        let (align_x, align_y) = resize_strip_alignment(direction);
        container(
            mouse_area(Space::new().width(width).height(height))
                .on_press(Message::WindowResize(direction))
                .interaction(resize_interaction(direction)),
        )
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .align_x(align_x)
        .align_y(align_y)
        .into()
    }

    let mut stack = Stack::new();
    for direction in RESIZE_HANDLE_DIRECTIONS {
        stack = stack.push(strip(direction));
    }

    stack
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .into()
}

/// Envuelve un contenido forzando el cursor indicado en toda su superficie.
///
/// Se usa con `Interaction::None` para anular el cambio de puntero que los
/// widgets clicables aplican por defecto; devuelve `Idle` (flecha normal) en
/// lugar de `None` para que la pila de widgets no delegue en el hijo.
pub struct CursorOff<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    interaction: iced::mouse::Interaction,
}

impl<'a, Message, Theme, Renderer> CursorOff<'a, Message, Theme, Renderer> {
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        interaction: iced::mouse::Interaction,
    ) -> Self {
        Self {
            content: content.into(),
            interaction,
        }
    }
}

impl<Message, Theme, Renderer> iced::advanced::Widget<Message, Theme, Renderer>
    for CursorOff<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn size(&self) -> iced::Size<iced::Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        self.content.as_widget_mut().layout(tree, renderer, limits)
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport)
    }

    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<iced::advanced::widget::Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut iced::advanced::widget::Tree) {
        self.content.as_widget().diff(tree)
    }

    fn operate(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
    }

    fn update(
        &mut self,
        state: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        viewport: &iced::Rectangle,
    ) {
        self.content.as_widget_mut().update(
            state, event, layout, cursor, renderer, clipboard, shell, viewport,
        )
    }

    fn mouse_interaction(
        &self,
        state: &iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
        renderer: &Renderer,
    ) -> iced::advanced::mouse::Interaction {
        let _ = self
            .content
            .as_widget()
            .mouse_interaction(state, layout, cursor, viewport, renderer);
        if self.interaction == iced::advanced::mouse::Interaction::None {
            iced::advanced::mouse::Interaction::Idle
        } else {
            self.interaction
        }
    }

    fn overlay<'b>(
        &'b mut self,
        state: &'b mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'b>,
        renderer: &Renderer,
        viewport: &iced::Rectangle,
        translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, Theme, Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(state, layout, renderer, viewport, translation)
    }
}

impl<'a, Message, Theme, Renderer> From<CursorOff<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(widget: CursorOff<'a, Message, Theme, Renderer>) -> Self {
        Element::new(widget)
    }
}

#[cfg(test)]
mod resize_shell_tests {
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
}
