use egui::{Color32, CornerRadius, Stroke};

pub const BG: Color32 = Color32::from_rgb(22, 24, 30);
pub const PANEL: Color32 = Color32::from_rgb(30, 33, 41);
pub const PANEL_LIGHT: Color32 = Color32::from_rgb(38, 42, 52);
pub const GRID_BG: Color32 = Color32::from_rgb(26, 28, 35);
pub const GRID_BG_ALT: Color32 = Color32::from_rgb(30, 32, 40);
pub const ROW_DARK: Color32 = Color32::from_rgb(21, 23, 29);
pub const LINE_BAR: Color32 = Color32::from_rgb(92, 98, 116);
pub const LINE_BEAT: Color32 = Color32::from_rgb(56, 60, 74);
pub const LINE_SUB: Color32 = Color32::from_rgb(38, 41, 51);
pub const TEXT_DIM: Color32 = Color32::from_rgb(140, 146, 162);
pub const ACCENT: Color32 = Color32::from_rgb(255, 196, 80);
pub const PLAYHEAD: Color32 = Color32::from_rgb(255, 214, 110);
pub const SELECT: Color32 = Color32::from_rgb(120, 180, 255);

pub fn track_color(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|style| {
        let v = &mut style.visuals;
        v.panel_fill = PANEL;
        v.window_fill = PANEL_LIGHT;
        v.extreme_bg_color = BG;
        v.faint_bg_color = PANEL_LIGHT;
        v.selection.bg_fill = Color32::from_rgb(70, 110, 170);
        v.hyperlink_color = ACCENT;
        v.window_corner_radius = CornerRadius::same(10);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(48, 52, 64));
        for w in [
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            w.corner_radius = CornerRadius::same(6);
        }
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.spacing.slider_width = 130.0;
        // Dropdowns size themselves from this, number fields and buttons
        // from their text and padding: match them so rows line up.
        style.spacing.interact_size.y = 23.0;
        style.interaction.tooltip_delay = 0.4;
    });
}
