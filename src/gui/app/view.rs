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

/// Franjas transparentes de 2 px pegadas a los bordes de la ventana.
///
/// Al vivir en la capa superior de la pila interceptan el press antes que las
/// barras de arrastre y las scrollbars, muestran el cursor de redimensionado y
/// disparan el arrastre de redimensionado del sistema operativo. Las esquinas
/// se apilan al final para tener prioridad sobre los lados.
fn resize_handles() -> Element<'static, Message> {
    use iced::widget::{Space, Stack, container, mouse_area};
    use iced::window::Direction;
    use iced::{Alignment, Length};

    const HIT: f32 = 2.0;

    fn strip(
        width: Length,
        height: Length,
        align_x: Alignment,
        align_y: Alignment,
        direction: Direction,
        interaction: iced::mouse::Interaction,
    ) -> Element<'static, Message> {
        container(
            mouse_area(Space::new().width(width).height(height))
                .on_press(Message::WindowResize(direction))
                .interaction(interaction),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(align_x)
        .align_y(align_y)
        .into()
    }

    let mut stack = Stack::new();

    // Lados
    stack = stack.push(strip(
        Length::Fill,
        Length::Fixed(HIT),
        Alignment::Center,
        Alignment::Start,
        Direction::North,
        iced::mouse::Interaction::ResizingVertically,
    ));
    stack = stack.push(strip(
        Length::Fill,
        Length::Fixed(HIT),
        Alignment::Center,
        Alignment::End,
        Direction::South,
        iced::mouse::Interaction::ResizingVertically,
    ));
    stack = stack.push(strip(
        Length::Fixed(HIT),
        Length::Fill,
        Alignment::Start,
        Alignment::Center,
        Direction::West,
        iced::mouse::Interaction::ResizingHorizontally,
    ));
    stack = stack.push(strip(
        Length::Fixed(HIT),
        Length::Fill,
        Alignment::End,
        Alignment::Center,
        Direction::East,
        iced::mouse::Interaction::ResizingHorizontally,
    ));

    // Esquinas (por encima de los lados)
    stack = stack.push(strip(
        Length::Fixed(HIT),
        Length::Fixed(HIT),
        Alignment::Start,
        Alignment::Start,
        Direction::NorthWest,
        iced::mouse::Interaction::ResizingDiagonallyDown,
    ));
    stack = stack.push(strip(
        Length::Fixed(HIT),
        Length::Fixed(HIT),
        Alignment::End,
        Alignment::Start,
        Direction::NorthEast,
        iced::mouse::Interaction::ResizingDiagonallyUp,
    ));
    stack = stack.push(strip(
        Length::Fixed(HIT),
        Length::Fixed(HIT),
        Alignment::Start,
        Alignment::End,
        Direction::SouthWest,
        iced::mouse::Interaction::ResizingDiagonallyUp,
    ));
    stack = stack.push(strip(
        Length::Fixed(HIT),
        Length::Fixed(HIT),
        Alignment::End,
        Alignment::End,
        Direction::SouthEast,
        iced::mouse::Interaction::ResizingDiagonallyDown,
    ));

    stack.width(Length::Fill).height(Length::Fill).into()
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
