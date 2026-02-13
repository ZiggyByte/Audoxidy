use eframe::egui;

pub const COLOR_BG: egui::Color32 = egui::Color32::from_rgb(0, 0, 0); // #000000
#[allow(dead_code)]
pub const COLOR_SURFACE: egui::Color32 = egui::Color32::from_rgb(17, 17, 17); // #111111
pub const COLOR_CONTRAST: egui::Color32 = egui::Color32::from_rgb(17, 17, 17); // #111111 (Usamos surface como contraste base)
pub const COLOR_ACCENT: egui::Color32 = egui::Color32::from_rgb(255, 0, 61); // #FF003D
pub const COLOR_TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(198, 198, 198); // #c6c6c6
pub const COLOR_TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(113, 113, 113); // #717171

pub fn setup_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    
    visuals.panel_fill = COLOR_BG;
    visuals.window_fill = COLOR_CONTRAST;
    visuals.widgets.noninteractive.bg_fill = COLOR_BG;
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, COLOR_TEXT_SECONDARY);
    
    visuals.widgets.inactive.bg_fill = COLOR_CONTRAST;
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, COLOR_TEXT_PRIMARY);
    
    visuals.widgets.hovered.bg_fill = COLOR_CONTRAST;
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.5, COLOR_ACCENT);
    
    visuals.widgets.active.bg_fill = COLOR_ACCENT;
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);

    visuals.selection.bg_fill = COLOR_ACCENT;
    
    ctx.set_visuals(visuals);
    setup_fonts(ctx);
}

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // Cargar Noto Sans para texto general
    fonts.font_data.insert(
        "noto_sans".to_owned(),
        egui::FontData::from_static(include_bytes!("../../assets/fonts/NotoSans-Regular.ttf")).into(),
    );

    // Cargar Stage Wander para el logo
    fonts.font_data.insert(
        "stage_wander".to_owned(),
        egui::FontData::from_static(include_bytes!("../../assets/fonts/Stage Wanders.ttf")).into(),
    );

    fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap()
        .insert(0, "noto_sans".to_owned());

    fonts.families.insert(
        egui::FontFamily::Name("logo".into()),
        vec!["stage_wander".to_owned()],
    );

    ctx.set_fonts(fonts);
}
