//! Bottom multitrack view: one lane per instrument, showing roughly when it
//! makes sound. Clicking a lane opens its note editor above.

use egui::{Align2, Color32, FontId, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use orchestre_core::{Category, InstrumentChoice, Track, seconds_to_ticks};

use crate::app::OrchestreApp;
use crate::theme;
use crate::ui::timeline::{self, HEADER_W, RULER_H};

const LANE_H: f32 = 46.0;

pub fn show(app: &mut OrchestreApp, ui: &mut Ui) {
    let rows = app.project.tracks.len() as f32;
    let wanted = RULER_H + rows * LANE_H + 52.0;
    egui::Panel::bottom("lanes")
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
            app.view.width = ruler_rect.width();
            if !app.view.fitted && ruler_rect.width() > 100.0 {
                fit_song(app, ruler_rect.width());
            }
            ui.painter().rect_filled(
                Rect::from_min_max(full.min, pos2(full.left() + HEADER_W, full.top() + RULER_H)),
                0.0,
                theme::PANEL_LIGHT,
            );
            ui.painter().text(
                pos2(full.left() + 12.0, full.top() + RULER_H / 2.0),
                Align2::LEFT_CENTER,
                "TRACKS",
                FontId::proportional(11.0),
                theme::TEXT_DIM,
            );
            timeline::ruler(app, ui, ruler_rect);

            let body = Rect::from_min_max(pos2(full.left(), full.top() + RULER_H), full.max);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(&mut child, |ui| {
                    let ids: Vec<_> = app.project.tracks.iter().map(|t| t.id).collect();
                    for (i, id) in ids.into_iter().enumerate() {
                        lane(app, ui, id, i);
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        add_instrument_menu(app, ui);
                        if app.project.tracks.is_empty() {
                            ui.label(
                                RichText::new("← Start by adding an instrument")
                                    .color(theme::TEXT_DIM),
                            );
                        }
                    });
                    ui.add_space(6.0);
                });
            let strip = Rect::from_min_max(pos2(body.left() + HEADER_W, body.top()), body.max);
            timeline::navigate(ui, strip, &mut app.view);
            // Content lives in child UIs; claim the space so the panel keeps its size.
            ui.allocate_rect(full, Sense::hover());
        });
}

/// Initial zoom: show the whole song.
fn fit_song(app: &mut OrchestreApp, width: f32) {
    app.view.fitted = true;
    let len = app
        .project
        .length_ticks()
        .max(app.project.time_sig.bar_ticks() * 4) as f32;
    app.view.zoom = (width * 0.98 / len * orchestre_core::PPQ as f32).clamp(8.0, 200.0);
    app.view.scroll = 0.0;
}

fn lane(app: &mut OrchestreApp, ui: &mut Ui, id: u64, index: usize) {
    let Some(track) = app.project.track(id).cloned() else {
        return;
    };
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, LANE_H), Sense::hover());
    let header = Rect::from_min_size(rect.min, vec2(HEADER_W, LANE_H));
    let strip = Rect::from_min_max(pos2(rect.left() + HEADER_W, rect.top()), rect.max);
    let selected = app.selected == Some(id);
    let color = theme::track_color(track.color);

    // Strip first (behind), so header widgets stay on top.
    let strip_resp = ui.interact(strip, ui.id().with(("strip", id)), Sense::click());
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
    timeline::draw_grid(&p, strip, &app.view, &app.project, false);
    draw_activity(app, &p, strip, &track, color);
    timeline::draw_playhead(&p, strip, &app.view, app.position);
    p.hline(
        strip.x_range(),
        strip.bottom() - 0.5,
        Stroke::new(1.0, theme::BG),
    );
    if strip_resp.clicked() {
        app.select_track(if selected { None } else { Some(id) });
    }
    strip_resp.on_hover_text(if selected {
        "Click to close the editor"
    } else {
        "Click to edit this track's notes"
    });

    // Header.
    let header_resp = ui.interact(header, ui.id().with(("header", id)), Sense::click());
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
        app.select_track(if selected { None } else { Some(id) });
    }

    let mut hui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(header.shrink2(vec2(12.0, 4.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    hui.add_space(2.0);
    hui.vertical(|ui| {
        ui.add_space(3.0);
        let rec = app.rec.active && selected;
        let name = if rec {
            format!("⏺ {}", track.name)
        } else {
            track.name.clone()
        };
        let color = if rec {
            Color32::from_rgb(240, 90, 90)
        } else {
            Color32::WHITE
        };
        ui.label(RichText::new(name).strong().color(color));
        ui.label(
            RichText::new(track.instrument.label())
                .size(11.0)
                .color(theme::TEXT_DIM),
        );
    });
    hui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let level = app.levels.get(&id).copied().unwrap_or(0.0);
        crate::ui::transport::meter(ui, level, vec2(6.0, LANE_H - 16.0));
        let mut solo = track.solo;
        if ui
            .toggle_value(&mut solo, RichText::new("S").strong())
            .on_hover_text("Solo: hear only this track")
            .changed()
        {
            if let Some(t) = app.project.track_mut(id) {
                t.solo = solo;
            }
            app.touch();
        }
        let mut mute = track.mute;
        if ui
            .toggle_value(&mut mute, RichText::new("M").strong())
            .on_hover_text("Mute")
            .changed()
        {
            if let Some(t) = app.project.track_mut(id) {
                t.mute = mute;
            }
            app.touch();
        }
    });

    header_resp.context_menu(|ui| track_menu(app, ui, id, index));
}

fn track_menu(app: &mut OrchestreApp, ui: &mut Ui, id: u64, index: usize) {
    if ui.button("✏ Rename").clicked() {
        let name = app
            .project
            .track(id)
            .map(|t| t.name.clone())
            .unwrap_or_default();
        app.renaming = Some((id, name));
        ui.close();
    }
    if ui.button("⧉ Duplicate").clicked() {
        duplicate_track(app, id);
        ui.close();
    }
    if ui
        .add_enabled(index > 0, egui::Button::new("⬆ Move up"))
        .clicked()
    {
        app.project.tracks.swap(index, index - 1);
        app.touch();
        ui.close();
    }
    if ui
        .add_enabled(
            index + 1 < app.project.tracks.len(),
            egui::Button::new("⬇ Move down"),
        )
        .clicked()
    {
        app.project.tracks.swap(index, index + 1);
        app.touch();
        ui.close();
    }
    ui.separator();
    if ui
        .button(RichText::new("🗑 Delete track").color(Color32::from_rgb(240, 120, 100)))
        .clicked()
    {
        app.confirm_delete_track = Some(id);
        ui.close();
    }
}

fn duplicate_track(app: &mut OrchestreApp, id: u64) {
    let Some(mut t) = app.project.track(id).cloned() else {
        return;
    };
    t.id = app.project.new_id();
    t.name = format!("{} copy", t.name);
    t.solo = false;
    for n in &mut t.notes {
        n.id = app.project.new_id();
    }
    let pos = app
        .project
        .tracks
        .iter()
        .position(|x| x.id == id)
        .map_or(app.project.tracks.len(), |i| i + 1);
    let new_id = t.id;
    app.project.tracks.insert(pos, t);
    app.select_track(Some(new_id));
    app.touch();
}

/// Note activity: merged blobs covering note durations plus the instrument's
/// release tail, with a tiny piano-roll sketch of the notes inside.
fn draw_activity(
    app: &OrchestreApp,
    p: &egui::Painter,
    strip: Rect,
    track: &Track,
    color: Color32,
) {
    if track.notes.is_empty() {
        return;
    }
    let view = &app.view;
    let tail = seconds_to_ticks(
        track.instrument.tail_seconds() as f64,
        app.project.bpm as f64,
    ) as i64;
    let mut spans: Vec<(i64, i64)> = track
        .notes
        .iter()
        .map(|n| (n.start, n.end() + tail))
        .collect();
    spans.sort_unstable();
    let mut merged: Vec<(i64, i64)> = Vec::new();
    for (s, e) in spans {
        match merged.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => merged.push((s, e)),
        }
    }
    let dim = if track.mute { 0.35 } else { 1.0 };
    let body = strip.shrink2(vec2(0.0, 5.0));
    for (s, e) in merged {
        let x0 = view.tick_to_x(strip.left(), s as f64);
        let x1 = view.tick_to_x(strip.left(), e as f64);
        if x1 < strip.left() || x0 > strip.right() {
            continue;
        }
        let r = Rect::from_x_y_ranges(x0..=x1.max(x0 + 2.0), body.y_range());
        p.rect_filled(r, 4.0, color.gamma_multiply(0.22 * dim));
    }
    let (lo, hi) = track.notes.iter().fold((u8::MAX, 0u8), |(lo, hi), n| {
        (lo.min(n.pitch), hi.max(n.pitch))
    });
    let range = (hi as f32 - lo as f32).max(6.0);
    let inner = body.shrink2(vec2(0.0, 3.0));
    for n in &track.notes {
        let x0 = view.tick_to_x(strip.left(), n.start as f64);
        let x1 = view.tick_to_x(strip.left(), n.end() as f64);
        if x1 < strip.left() || x0 > strip.right() {
            continue;
        }
        let frac = if track.instrument.is_drums() {
            // Drum rows go top to bottom.
            (n.pitch as f32 - lo as f32) / range
        } else {
            1.0 - (n.pitch as f32 - lo as f32) / range
        };
        let y = inner.top() + frac * inner.height();
        let r = Rect::from_x_y_ranges(x0..=(x1 - 1.0).max(x0 + 1.5), (y - 1.5)..=(y + 1.5));
        p.rect_filled(r, 1.0, color.gamma_multiply((0.5 + 0.5 * n.vel) * dim));
    }
}

pub fn add_instrument_menu(app: &mut OrchestreApp, ui: &mut Ui) {
    ui.menu_button(RichText::new("+ Add instrument").strong(), |ui| {
        ui.set_min_width(190.0);
        ui.label(
            RichText::new("Rest on a sound to hear it")
                .size(11.5)
                .color(theme::TEXT_DIM),
        );
        for cat in Category::ALL {
            let choices: Vec<InstrumentChoice> = InstrumentChoice::all()
                .into_iter()
                .filter(|c| c.category() == cat)
                .collect();
            if let [only] = choices[..] {
                instrument_entry(app, ui, only, cat.label());
                continue;
            }
            ui.menu_button(cat.label(), |ui| {
                ui.set_min_width(300.0);
                for choice in choices {
                    instrument_entry(app, ui, choice, choice.label());
                }
            });
        }
    });
}

/// One menu entry: adds the instrument on click, previews it on hover.
fn instrument_entry(app: &mut OrchestreApp, ui: &mut Ui, choice: InstrumentChoice, label: &str) {
    let resp = ui.horizontal(|ui| {
        let resp = ui.button(label);
        ui.label(
            RichText::new(choice.description())
                .size(11.5)
                .color(theme::TEXT_DIM),
        );
        resp
    });
    let button = resp.inner;
    crate::ui::preview::hover(app, &button.union(resp.response), choice);
    if button.clicked() {
        let id = app.project.add_track(choice);
        app.select_track(Some(id));
        app.touch();
        ui.close();
    }
}
