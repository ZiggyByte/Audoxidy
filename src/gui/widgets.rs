use eframe::egui;
use crate::gui::theme::{COLOR_TEXT_PRIMARY, COLOR_ACCENT};

#[allow(dead_code)]
pub fn draw_logo(ui: &mut egui::Ui, font_size: f32) {
    ui.horizontal(|ui| {
        let text = "AuDoxiDY";
        let is_hovered = ui.interact(ui.max_rect(), ui.id(), egui::Sense::hover()).hovered();

        for (_i, c) in text.chars().enumerate() {
            let color = if is_hovered && (c == 'A' || c == 'o' || c == 'Y') {
                // TODO: Implement animation letter by letter
                COLOR_ACCENT
            } else {
                COLOR_TEXT_PRIMARY
            };

            ui.add(egui::Label::new(
                egui::RichText::new(c.to_string())
                    .color(color)
                    .font(egui::FontId::new(font_size, egui::FontFamily::Name("logo".into())))
            ));
            ui.add_space(-2.0); // Ajuste fino entre letras
        }
    });
}
