//! Native context-menu overlay rendering.

use iced::Element;

use crate::gui::dimensions::{
    CONTEXT_MENU_PADDING, CONTEXT_MENU_WIDTH, CONTROL_MD, OVERLAY_EDGE_MARGIN,
};

use super::super::{AudoxidyApp, Message};

impl AudoxidyApp {
    pub(super) fn context_menu_overlay(&self) -> Option<Element<'_, Message>> {
        if let Some((pos, entries)) = &self.context_menu {
            let menu_content = crate::gui::widgets::build_context_menu_content(entries.clone());

            // Área de bloqueo para el menú
            let menu_blocker = iced::widget::mouse_area(
                iced::widget::Space::new()
                    .width(iced::Length::Fill)
                    .height(iced::Length::Fill),
            )
            .on_press(Message::GlobalClick)
            .on_right_press(Message::GlobalClick);

            // Ajuste de bordes (Stay within window)
            let mut target_x = pos.x;
            let mut target_y = pos.y;
            let menu_w = CONTEXT_MENU_WIDTH;
            let menu_h = (entries.len() as f32 * CONTROL_MD) + (2.0 * CONTEXT_MENU_PADDING); // aprox

            if target_x + menu_w > self.window_size.0 as f32 {
                target_x = (self.window_size.0 as f32 - menu_w - OVERLAY_EDGE_MARGIN)
                    .max(OVERLAY_EDGE_MARGIN);
            }
            if target_y + menu_h > self.window_size.1 as f32 {
                target_y = (self.window_size.1 as f32 - menu_h - OVERLAY_EDGE_MARGIN)
                    .max(OVERLAY_EDGE_MARGIN);
            }
            let mut final_stack = iced::widget::Stack::new();
            final_stack = final_stack.push(menu_blocker).push(
                iced::widget::container(menu_content)
                    .width(iced::Length::Fill)
                    .height(iced::Length::Fill)
                    .padding(iced::Padding {
                        top: target_y,
                        left: target_x,
                        ..Default::default()
                    })
                    .align_x(iced::Alignment::Start)
                    .align_y(iced::Alignment::Start),
            );
            Some(final_stack.into())
        } else {
            None
        }
    }
}
