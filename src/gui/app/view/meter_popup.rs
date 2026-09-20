//! VU-meter configuration popup overlay rendering.

use iced::Element;

use crate::gui::dimensions::{METER_POPUP_HEIGHT, METER_POPUP_WIDTH, OVERLAY_EDGE_MARGIN};

use super::super::{AudoxidyApp, Message};

impl AudoxidyApp {
    pub(super) fn meter_popup_overlay(&self) -> Option<Element<'_, Message>> {
        if self.player_ui_state.meter_popup_open {
            if let Some(pos) = self.player_ui_state.meter_popup_pos {
                let state_arc = self.core.audio_manager().state();
                let s = state_arc.read();
                let popup_content = crate::gui::widgets::meter_popup_view(
                    s.meter_hold_time_ms,
                    s.meter_rms_window_ms,
                    self.player_ui_state.meter.ms_mode,
                    s.meter_infinite_hold,
                    s.meter_enabled,
                );

                // Área de bloqueo para cerrar el popup al hacer click fuera
                let popup_blocker = iced::widget::mouse_area(
                    iced::widget::Space::new()
                        .width(iced::Length::Fill)
                        .height(iced::Length::Fill),
                )
                .on_press(Message::MeterPopupClose)
                .on_right_press(Message::MeterPopupClose);

                // Ajuste de bordes (Stay within window)
                let popup_w = METER_POPUP_WIDTH;
                let popup_h = METER_POPUP_HEIGHT; // aprox
                let mut target_x = pos.x;
                let mut target_y = pos.y;

                if target_x + popup_w > self.window_size.0 as f32 {
                    target_x = (self.window_size.0 as f32 - popup_w - OVERLAY_EDGE_MARGIN)
                        .max(OVERLAY_EDGE_MARGIN);
                }
                if target_y + popup_h > self.window_size.1 as f32 {
                    target_y = (self.window_size.1 as f32 - popup_h - OVERLAY_EDGE_MARGIN)
                        .max(OVERLAY_EDGE_MARGIN);
                }
                let mut final_stack = iced::widget::Stack::new();
                final_stack = final_stack.push(popup_blocker).push(
                    iced::widget::container(popup_content)
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
                return Some(final_stack.into());
            }
        }
        None
    }
}
