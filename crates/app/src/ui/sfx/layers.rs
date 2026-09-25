//! Bottom panel: one lane per layer on a timeline in seconds. Drag a
//! layer to change when it starts; click one to shape it above.

use egui::{Align2, Color32, FontId, Painter, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use orchestre_core::Id;
use orchestre_core::sfx::GeneratorKind;
use orchestre_core::sfx::presets::{PRESETS, SfxCategory};

use crate::app::OrchestreApp;
use crate::sfx::{LayerDrag, SfxView, WAVE_BINS};
use crate::theme;
use crate::ui::timeline::{HEADER_W, RULER_H};

const LANE_H: f32 = 42.0;
/// Layer starts snap to this, unless Alt is held.
pub const SNAP: f32 = 0.005;

pub fn show(app: &mut OrchestreApp, ui: &mut Ui) {
    let rows = app.sfx.sound.layers.len() as f32;
    let wanted = RULER_H + rows * LANE_H + 52.0;
    egui::Panel::bottom("sfx_layers")
        .resizable(true)
        .default_size(wanted.clamp(150.0, 330.0))
        .min_size(110.0)
        .frame(egui::Frame::new().fill(theme::PANEL))
        .show(ui, |ui| {
            let full = ui.available_rect_before_wrap();
            let ruler_rect = Rect::from_min_max(
                pos2(full.left() + HEADER_W, full.top()),
                pos2(full.right(), full.top() + RULER_H),
            );
            if !app.sfx.view.fitted && ruler_rect.width() > 100.0 {
                fit(app, ruler_rect.width());
            }
            ui.painter().rect_filled(
                Rect::from_min_max(full.min, pos2(full.left() + HEADER_W, full.top() + RULER_H)),
                0.0,
                theme::PANEL_LIGHT,
            );
            ui.painter().text(
                pos2(full.left() + 12.0, full.top() + RULER_H / 2.0),
                Align2::LEFT_CENTER,
                "LAYERS",
                FontId::proportional(11.0),
                theme::TEXT_DIM,
            );
            ruler(app, ui, ruler_rect);

            let body = Rect::from_min_max(pos2(full.left(), full.top() + RULER_H), full.max);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(&mut child, |ui| {
                    let ids: Vec<_> = app.sfx.sound.layers.iter().map(|l| l.id).collect();
                    for (i, id) in ids.into_iter().enumerate() {
                        lane(app, ui, id, i);
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        add_layer_menu(app, ui);
                        if app.sfx.sound.layers.is_empty() {
                            ui.label(
                                RichText::new(
                                    "← Start by adding a layer, or pick a preset on the left",
                                )
                                .color(theme::TEXT_DIM),
                            );
                        }
                    });
                    ui.add_space(6.0);
                });
            let strip = Rect::from_min_max(pos2(body.left() + HEADER_W, body.top()), body.max);
            navigate(ui, strip, &mut app.sfx.view);
            ui.allocate_rect(full, Sense::hover());
        });
}

/// Show the whole sound, with a little room after it.
fn fit(app: &mut OrchestreApp, width: f32) {
    let v = &mut app.sfx.view;
    v.fitted = true;
    let len = (app.sfx.sound.length() + app.sfx.sound.space * 0.5).max(0.3);
    v.zoom = (width * 0.9 / len).clamp(40.0, 20000.0);
    v.scroll = 0.0;
}

/// Scroll (trackpad / Shift+wheel) and zoom (Ctrl/Cmd+wheel, pinch).
pub fn navigate(ui: &Ui, rect: Rect, view: &mut SfxView) {
    let Some(pos) = ui.input(|i| i.pointer.hover_pos()) else {
        return;
    };
    if !rect.contains(pos) {
        return;
    }
    let (delta, zoom, shift) =
        ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.modifiers.shift));
    if zoom != 1.0 {
        view.zoom_at(rect.left(), pos.x, zoom);
    } else if shift && delta.x == 0.0 {
        view.scroll_by_px(delta.y);
    } else if delta.x != 0.0 {
        view.scroll_by_px(delta.x);
    }
}

/// Time between grid lines: the finest step that is still readable.
pub fn grid_step(view: &SfxView) -> f32 {
    [
        0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0,
    ]
    .into_iter()
    .find(|&s| s * view.zoom >= 60.0)
    .unwrap_or(10.0)
}

pub fn draw_grid(p: &Painter, rect: Rect, view: &SfxView) {
    let step = grid_step(view);
    let t0 = view.x_to_time(rect.left(), rect.left()).max(0.0);
    let t1 = view.x_to_time(rect.left(), rect.right());
    let mut k = (t0 / step).floor() as i64;
    while (k as f32) * step <= t1 {
        let x = view.time_to_x(rect.left(), k as f32 * step);
        let major = k % 5 == 0;
        p.vline(
            x,
            rect.y_range(),
            Stroke::new(
                1.0,
                if major {
                    theme::LINE_BEAT
                } else {
                    theme::LINE_SUB
                },
            ),
        );
        k += 1;
    }
}

/// Playheads of recent plays.
pub fn draw_playheads(app: &OrchestreApp, p: &Painter, rect: Rect) {
    for &(start, len) in &app.sfx.plays {
        let t = (app.now - start) as f32;
        if t > len {
            continue;
        }
        let x = app.sfx.view.time_to_x(rect.left(), t);
        if x >= rect.left() && x <= rect.right() {
            p.vline(x, rect.y_range(), Stroke::new(2.0, theme::PLAYHEAD));
        }
    }
}

fn ruler(app: &mut OrchestreApp, ui: &mut Ui, rect: Rect) {
    let resp = ui.interact(rect, ui.id().with("sfx_ruler"), Sense::click());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, theme::PANEL_LIGHT);
    let view = &app.sfx.view;
    let step = grid_step(view);
    let t0 = view.x_to_time(rect.left(), rect.left()).max(0.0);
    let t1 = view.x_to_time(rect.left(), rect.right());
    let mut k = (t0 / step).floor() as i64;
    while (k as f32) * step <= t1 {
        let t = k as f32 * step;
        let x = view.time_to_x(rect.left(), t);
        p.vline(
            x,
            (rect.bottom() - rect.height() * 0.4)..=rect.bottom(),
            Stroke::new(1.0, theme::LINE_BAR),
        );
        p.text(
            pos2(x + 4.0, rect.center().y),
            Align2::LEFT_CENTER,
            super::format_secs(t),
            FontId::proportional(11.0),
            theme::TEXT_DIM,
        );
        k += 1;
    }
    draw_playheads(app, &p, rect);
    if resp.clicked() {
        app.play_sound();
    }
    resp.on_hover_text("Click to play");
}

fn lane(app: &mut OrchestreApp, ui: &mut Ui, id: Id, index: usize) {
    let Some(layer) = app.sfx.sound.layer(id).cloned() else {
        return;
    };
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, LANE_H), Sense::hover());
    let header = Rect::from_min_size(rect.min, vec2(HEADER_W, LANE_H));
    let strip = Rect::from_min_max(pos2(rect.left() + HEADER_W, rect.top()), rect.max);
    let selected = app.sfx.selected == Some(id);
    let color = theme::track_color(layer.color);
    let audible = app.sfx.sound.audible(&layer);

    // Strip: the layer as a block, with its waveform.
    let strip_resp = ui.interact(
        strip,
        ui.id().with(("sfx_strip", id)),
        Sense::click_and_drag(),
    );
    let p = ui.painter_at(strip);
    p.rect_filled(
        strip,
        0.0,
        if selected {
            theme::GRID_BG_ALT
        } else {
            theme::GRID_BG
        },
    );
    draw_grid(&p, strip, &app.sfx.view);
    let view = &app.sfx.view;
    let x0 = view.time_to_x(strip.left(), layer.start);
    let x1 = view.time_to_x(strip.left(), layer.end());
    let block = Rect::from_x_y_ranges(
        x0..=x1.max(x0 + 3.0),
        strip.shrink2(vec2(0.0, 4.0)).y_range(),
    );
    let dim = if audible { 1.0 } else { 0.35 };
    p.rect_filled(block, 4.0, color.gamma_multiply(0.18 * dim));
    let sound = app.sfx.sound.clone();
    let env = app.sfx.waves.get(&sound, &layer).to_vec();
    let tint = color.gamma_multiply(0.9 * dim);
    draw_wave(
        &p,
        strip.left(),
        block,
        &env,
        &app.sfx.view,
        layer.start,
        tint,
    );
    if selected {
        p.rect_stroke(
            block,
            4.0,
            Stroke::new(1.5, color),
            egui::StrokeKind::Inside,
        );
    }
    draw_playheads(app, &p, strip);
    p.hline(
        strip.x_range(),
        strip.bottom() - 0.5,
        Stroke::new(1.0, theme::BG),
    );

    let alt = ui.input(|i| i.modifiers.alt);
    let pointer_t = strip_resp
        .interact_pointer_pos()
        .map(|pos| app.sfx.view.x_to_time(strip.left(), pos.x));
    if strip_resp.drag_started()
        && let (Some(t), Some(pos)) = (pointer_t, strip_resp.interact_pointer_pos())
        && block.expand2(vec2(4.0, 0.0)).contains(pos)
    {
        app.sfx.selected = Some(id);
        app.sfx.drag = Some(LayerDrag {
            id,
            grab: t,
            orig: layer.start,
        });
    }
    if strip_resp.dragged()
        && let (Some(d), Some(t)) = (&app.sfx.drag, pointer_t)
        && d.id == id
    {
        let mut start = (d.orig + t - d.grab).max(0.0);
        if !alt {
            start = (start / SNAP).round() * SNAP;
        }
        if let Some(l) = app.sfx.sound.layer_mut(id)
            && l.start != start
        {
            l.start = start;
            app.sfx.touch();
        }
    }
    if strip_resp.drag_stopped() {
        app.sfx.drag = None;
    }
    if strip_resp.clicked() {
        app.sfx.selected = if selected { None } else { Some(id) };
    }
    strip_resp.on_hover_text(
        "Drag to change when this layer starts (Alt: no snapping). Click to shape it.",
    );

    // Header.
    let header_resp = ui.interact(header, ui.id().with(("sfx_header", id)), Sense::click());
    let hp = ui.painter_at(header);
    hp.rect_filled(
        header,
        0.0,
        if selected {
            theme::PANEL_LIGHT
        } else {
            theme::PANEL
        },
    );
    hp.rect_filled(
        Rect::from_min_size(header.min, vec2(5.0, LANE_H)),
        0.0,
        color,
    );
    if selected {
        hp.rect_stroke(
            header.shrink(0.5),
            0.0,
            Stroke::new(1.0, color),
            egui::StrokeKind::Inside,
        );
    }
    hp.hline(
        header.x_range(),
        header.bottom() - 0.5,
        Stroke::new(1.0, theme::BG),
    );
    if header_resp.clicked() {
        app.sfx.selected = if selected { None } else { Some(id) };
    }

    let mut hui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(header.shrink2(vec2(12.0, 4.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    hui.add_space(2.0);
    hui.vertical(|ui| {
        ui.set_max_width(HEADER_W - 170.0);
        ui.add_space(2.0);
        ui.add(
            egui::Label::new(RichText::new(&layer.name).strong().color(Color32::WHITE)).truncate(),
        );
        let kind = layer.generator.kind();
        let info = format!("{} · {}", kind.label(), super::format_secs(layer.start));
        ui.add(egui::Label::new(RichText::new(info).size(11.0).color(theme::TEXT_DIM)).truncate());
    });
    hui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let small = |text: &str| {
            egui::Button::new(RichText::new(text).size(12.0)).min_size(vec2(22.0, 22.0))
        };
        if ui.add(small("🗑")).on_hover_text("Delete layer").clicked() {
            app.sfx.delete_layer(id);
            app.notify("Layer deleted (Undo brings it back)");
        }
        ui.add_space(4.0);
        let mut solo = layer.solo;
        if ui
            .toggle_value(&mut solo, RichText::new("S").strong())
            .on_hover_text("Solo: hear only this layer")
            .changed()
            && let Some(l) = app.sfx.sound.layer_mut(id)
        {
            l.solo = solo;
            app.sfx.touch();
        }
        let mut mute = layer.mute;
        if ui
            .toggle_value(&mut mute, RichText::new("M").strong())
            .on_hover_text("Mute")
            .changed()
            && let Some(l) = app.sfx.sound.layer_mut(id)
        {
            l.mute = mute;
            app.sfx.touch();
        }
    });

    header_resp.context_menu(|ui| layer_menu(app, ui, id, index));
}

fn layer_menu(app: &mut OrchestreApp, ui: &mut Ui, id: Id, index: usize) {
    if ui.button("⧉ Duplicate").clicked() {
        app.sfx.duplicate_layer(id);
        ui.close();
    }
    let count = app.sfx.sound.layers.len();
    if ui
        .add_enabled(index > 0, egui::Button::new("⬆ Move up"))
        .clicked()
    {
        app.sfx.sound.layers.swap(index, index - 1);
        app.sfx.touch();
        ui.close();
    }
    if ui
        .add_enabled(index + 1 < count, egui::Button::new("⬇ Move down"))
        .clicked()
    {
        app.sfx.sound.layers.swap(index, index + 1);
        app.sfx.touch();
        ui.close();
    }
    ui.separator();
    if ui
        .button(RichText::new("🗑 Delete layer").color(Color32::from_rgb(240, 120, 100)))
        .clicked()
    {
        app.sfx.delete_layer(id);
        ui.close();
    }
}

/// A mirrored waveform of peaks (`WAVE_BINS` per second from `start`),
/// inside `rect`, on a timeline whose zero is at `left` (before scrolling).
#[allow(clippy::too_many_arguments)]
pub fn draw_wave(
    p: &Painter,
    left: f32,
    rect: Rect,
    env: &[f32],
    view: &SfxView,
    start: f32,
    color: Color32,
) {
    let clip = p.clip_rect().intersect(rect);
    if env.is_empty() || clip.width() <= 0.0 {
        return;
    }
    let cy = rect.center().y;
    let half = rect.height() * 0.46;
    let step = 2.0;
    let mut x = clip.left();
    while x <= clip.right() {
        let t0 = view.x_to_time(left, x) - start;
        let t1 = t0 + step / view.zoom;
        let (b0, b1) = ((t0 * WAVE_BINS) as isize, (t1 * WAVE_BINS).ceil() as isize);
        let peak = (b0.max(0)..b1.max(b0 + 1))
            .filter_map(|b| env.get(b as usize))
            .fold(0.0f32, |m, &v| m.max(v));
        if peak > 0.002 {
            // Square root: quiet tails stay visible.
            let h = (peak.min(1.0).sqrt() * half).max(0.5);
            p.vline(x, (cy - h)..=(cy + h), Stroke::new(step, color));
        }
        x += step;
    }
}

pub fn add_layer_menu(app: &mut OrchestreApp, ui: &mut Ui) {
    ui.menu_button(RichText::new("+ Add layer").strong(), |ui| {
        ui.set_min_width(220.0);
        for kind in GeneratorKind::ALL {
            let resp = ui.horizontal(|ui| {
                let b = ui.button(kind.label());
                ui.label(
                    RichText::new(kind.description())
                        .size(11.5)
                        .color(theme::TEXT_DIM),
                );
                b
            });
            if resp.inner.clicked() {
                app.sfx.add_layer(kind);
                ui.close();
            }
        }
        ui.separator();
        ui.menu_button("Copy layers from a preset", |ui| {
            for cat in SfxCategory::ALL {
                ui.menu_button(cat.label(), |ui| {
                    for p in PRESETS.iter().filter(|p| p.category == cat) {
                        let resp = ui.button(p.name).on_hover_text(p.description);
                        if resp.hovered() {
                            app.sfx.preview.hovered = Some(crate::sfx::Preview::Preset(p.name));
                        }
                        if resp.clicked() {
                            app.sfx.add_layers_from(&p.sound());
                            ui.close();
                        }
                    }
                });
            }
        });
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(lib) = &app.sfx.library
            && lib.root.count() > 0
        {
            let files = lib.root.all_sounds();
            ui.menu_button("Copy layers from my sounds", |ui| {
                for (name, path) in files {
                    if ui.button(&name).clicked() {
                        let sound = std::fs::read_to_string(&path)
                            .map_err(|e| e.to_string())
                            .and_then(|j| orchestre_core::sfx::file::parse_sound(&j));
                        match sound {
                            Ok(s) => app.sfx.add_layers_from(&s),
                            Err(e) => app.notify(format!("Could not open {name}: {e}")),
                        }
                        ui.close();
                    }
                }
            });
        }
    });
}
