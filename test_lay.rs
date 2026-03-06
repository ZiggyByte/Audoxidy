use eframe::egui;
fn main() {
    let ctx = egui::Context::default();
    let job = egui::text::LayoutJob::default();
    let galley = ctx.fonts(|f| f.layout_job(job));
}
