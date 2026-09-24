//! The panel above the lanes: the selected track's sidebar and note editor.

use egui::{Color32, FontId, Rect, RichText, Ui, pos2};

use crate::app::OrchestreApp;
use crate::theme;
use crate::ui::timeline::{HEADER_W, KEYS_W};

pub fn show(app: &mut OrchestreApp, ui: &mut Ui) {
    let full = ui.max_rect();
    if app.selected_track().is_none() {
        welcome(app, ui, full);
        return;
    }
    let side = Rect::from_min_max(
        full.min,
        pos2(full.left() + HEADER_W - KEYS_W, full.bottom()),
    );
    let roll = Rect::from_min_max(pos2(side.right(), full.top()), full.max);

    ui.painter().rect_filled(side, 0.0, theme::PANEL);
    let mut side_ui =
        ui.new_child(egui::UiBuilder::new().max_rect(side.shrink2(egui::vec2(10.0, 0.0))));
    crate::ui::sidebar::show(app, &mut side_ui);

    crate::ui::roll::show(app, ui, roll);
    record_overlay(app, ui, roll);
    toast(app, ui, roll);
}

fn welcome(app: &mut OrchestreApp, ui: &mut Ui, rect: Rect) {
    ui.painter().rect_filled(rect, 0.0, theme::BG);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Center)),
    );
    child.add_space((rect.height() * 0.28).max(20.0));
    child.label(RichText::new("♫").size(48.0).color(theme::ACCENT));
    if app.project.tracks.is_empty() {
        child.label(RichText::new("An empty song. Let's add an instrument!").size(20.0));
        child.add_space(10.0);
        crate::ui::lanes::add_instrument_menu(app, &mut child);
    } else {
        child.label(RichText::new("Click a track below to write its notes").size(20.0));
        child.add_space(6.0);
        child.label(RichText::new("Space plays and stops the song").color(theme::TEXT_DIM));
    }
    toast(app, ui, rect);
}

/// Count-in number, and a "recording" badge while recording.
fn record_overlay(app: &OrchestreApp, ui: &Ui, rect: Rect) {
    if !app.rec.active {
        return;
    }
    let p = ui.painter();
    let red = Color32::from_rgb(220, 60, 60);
    if let Some(beats) = app.rec.counting {
        p.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            beats.to_string(),
            FontId::proportional(96.0),
            Color32::from_white_alpha(200),
        );
        p.text(
            rect.center() + egui::vec2(0.0, 64.0),
            egui::Align2::CENTER_TOP,
            "Get ready…",
            FontId::proportional(18.0),
            Color32::from_white_alpha(160),
        );
        return;
    }
    // Blinks once per beat.
    let beat = app.project.time_sig.beat_ticks() as f64;
    let on = (app.estimated_position() / beat).fract() < 0.5;
    let badge = Rect::from_min_size(
        rect.right_top() + egui::vec2(-130.0, 30.0),
        egui::vec2(118.0, 26.0),
    );
    p.rect_filled(
        badge,
        13.0,
        Color32::from_rgba_unmultiplied(40, 20, 20, 220),
    );
    p.circle_filled(
        badge.left_center() + egui::vec2(15.0, 0.0),
        6.0,
        if on { red } else { red.gamma_multiply(0.35) },
    );
    p.text(
        badge.left_center() + egui::vec2(28.0, 0.0),
        egui::Align2::LEFT_CENTER,
        "RECORDING",
        FontId::proportional(13.0),
        Color32::WHITE,
    );
}

fn toast(app: &OrchestreApp, ui: &Ui, rect: Rect) {
    let Some((msg, _)) = &app.toast else { return };
    let p = ui.painter();
    let font = FontId::proportional(14.0);
    let galley = p.layout_no_wrap(msg.clone(), font, Color32::WHITE);
    let size = galley.size() + egui::vec2(24.0, 14.0);
    let r = Rect::from_center_size(pos2(rect.center().x, rect.bottom() - 30.0), size);
    p.rect_filled(r, 8.0, Color32::from_rgba_unmultiplied(20, 22, 28, 235));
    p.rect_stroke(
        r,
        8.0,
        egui::Stroke::new(1.0, theme::ACCENT.gamma_multiply(0.6)),
        egui::StrokeKind::Inside,
    );
    p.galley(r.center() - galley.size() / 2.0, galley, Color32::WHITE);
}
