use eframe::egui::{self, Color32, FontFamily, FontId, Rounding, Stroke, TextStyle, Vec2};

// ---- palette (dark, modern) ----
pub const BG: Color32 = Color32::from_rgb(0x10, 0x12, 0x19); // app background
pub const PANEL: Color32 = Color32::from_rgb(0x15, 0x18, 0x20); // top/bottom bars
pub const CARD: Color32 = Color32::from_rgb(0x1b, 0x1f, 0x29); // project cards
pub const CARD_BORDER: Color32 = Color32::from_rgb(0x28, 0x2d, 0x3a);
pub const CARD_SELECTED: Color32 = Color32::from_rgb(0x20, 0x26, 0x38);
pub const INPUT_BG: Color32 = Color32::from_rgb(0x11, 0x14, 0x1b);

pub const ACCENT: Color32 = Color32::from_rgb(0x7c, 0x8d, 0xff); // indigo
pub const ACCENT_DARK: Color32 = Color32::from_rgb(0x4f, 0x63, 0xe6);
pub const TEXT: Color32 = Color32::from_rgb(0xdb, 0xdf, 0xe9);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x8b, 0x92, 0xa4);

// status colors (fg on tinted bg chips)
pub const GREEN: Color32 = Color32::from_rgb(0x5b, 0xd6, 0x8a);
pub const GREEN_BG: Color32 = Color32::from_rgb(0x14, 0x2a, 0x1e);
pub const ORANGE: Color32 = Color32::from_rgb(0xff, 0x9d, 0x5c);
pub const ORANGE_BG: Color32 = Color32::from_rgb(0x33, 0x21, 0x14);
pub const AMBER: Color32 = Color32::from_rgb(0xf5, 0xc9, 0x4c);
pub const AMBER_BG: Color32 = Color32::from_rgb(0x30, 0x28, 0x12);
pub const BLUE: Color32 = Color32::from_rgb(0x6f, 0xb5, 0xff);
pub const BLUE_BG: Color32 = Color32::from_rgb(0x14, 0x24, 0x38);
pub const PURPLE: Color32 = Color32::from_rgb(0xc0, 0xa9, 0xff);
pub const PURPLE_BG: Color32 = Color32::from_rgb(0x25, 0x1f, 0x3b);
pub const RED: Color32 = Color32::from_rgb(0xff, 0x7a, 0x7a);
pub const RED_BG: Color32 = Color32::from_rgb(0x33, 0x17, 0x17);
pub const GRAY_BG: Color32 = Color32::from_rgb(0x23, 0x27, 0x31);

pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(13.5, FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(13.5, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(11.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(19.0, FontFamily::Proportional),
    );

    style.spacing.item_spacing = Vec2::new(8.0, 7.0);
    style.spacing.button_padding = Vec2::new(12.0, 5.5);
    style.spacing.interact_size.y = 28.0;

    let mut v = egui::Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = CARD;
    v.window_stroke = Stroke::new(1.0, CARD_BORDER);
    v.window_rounding = Rounding::same(12.0);
    v.extreme_bg_color = INPUT_BG;
    v.override_text_color = Some(TEXT);

    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, CARD_BORDER);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);

    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0x24, 0x29, 0x36);
    v.widgets.inactive.bg_fill = Color32::from_rgb(0x24, 0x29, 0x36);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    // visible outline for checkboxes and text inputs
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x3a, 0x41, 0x54));
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x2f, 0x36, 0x48);
    v.widgets.hovered.bg_fill = Color32::from_rgb(0x2f, 0x36, 0x48);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT_DARK);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(0x3a, 0x42, 0x59);
    v.widgets.active.bg_fill = Color32::from_rgb(0x3a, 0x42, 0x59);

    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.rounding = Rounding::same(8.0);
    }

    v.selection.bg_fill = Color32::from_rgba_unmultiplied(0x7c, 0x8d, 0xff, 55);
    v.selection.stroke = Stroke::new(1.0, ACCENT);

    style.visuals = v;
    ctx.set_style(style);
}

/// Small rounded status chip: colored text on a tinted background.
pub fn chip(ui: &mut egui::Ui, text: &str, fg: Color32, bg: Color32) -> egui::Response {
    egui::Frame::none()
        .fill(bg)
        .rounding(Rounding::same(9.0))
        .inner_margin(egui::Margin::symmetric(8.0, 2.5))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).color(fg).size(11.5).strong());
        })
        .response
}

/// Accent-filled call-to-action button.
pub fn primary_button(ui: &mut egui::Ui, enabled: bool, text: &str) -> egui::Response {
    let fill = if enabled {
        ACCENT_DARK
    } else {
        Color32::from_rgb(0x2a, 0x2f, 0x3d)
    };
    let color = if enabled { Color32::WHITE } else { TEXT_DIM };
    ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(text).color(color).strong())
            .fill(fill)
            .rounding(Rounding::same(8.0)),
    )
}
