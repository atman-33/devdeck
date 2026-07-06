#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod app;
mod git;
mod models;
mod storage;
mod theme;
mod update;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1020.0, 680.0])
            .with_min_inner_size([720.0, 400.0])
            .with_title("DevDeck — Developer Workspace Manager"),
        ..Default::default()
    };
    eframe::run_native(
        "devdeck",
        options,
        Box::new(|cc| {
            setup_fonts(&cc.egui_ctx);
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(app::DevDeckApp::new()))
        }),
    )
}

/// egui's bundled fonts have no CJK glyphs; project names and paths may be
/// Japanese, so load a system font as a fallback when available.
fn setup_fonts(ctx: &egui::Context) {
    let candidates = [
        "C:/Windows/Fonts/YuGothM.ttc",
        "C:/Windows/Fonts/meiryo.ttc",
        "C:/Windows/Fonts/msgothic.ttc",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts
                .font_data
                .insert("jp-fallback".into(), egui::FontData::from_owned(bytes));
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts
                    .families
                    .entry(family)
                    .or_default()
                    .push("jp-fallback".into());
            }
            ctx.set_fonts(fonts);
            return;
        }
    }
}
