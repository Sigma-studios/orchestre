//! Note editor: piano roll for melodic instruments, drum grid for drums.

use std::collections::HashSet;

use egui::{
    Align2, Color32, CursorIcon, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};
use orchestre_core::edit;
use orchestre_core::{
    DrumPiece, Id, Note, PPQ, Tick, Track, is_black_key, pitch_name, snap_floor, snap_round,
};

use crate::app::{Drag, OrchestreApp};
use crate::theme;
use crate::ui::timeline::{self, KEYS_W, RULER_H};

/// The visible rows, top to bottom.
struct Rows {
    pitches: Vec<u8>,
    h: f32,
    drums: bool,
    locked: bool,
}

impl Rows {
    fn for_track(track: &Track) -> Rows {
        if track.instrument.is_drums() {
            return Rows {
                pitches: (0..DrumPiece::ALL.len() as u8).collect(),
                h: 24.0,
                drums: true,
                locked: false,
            };
        }
        let (lo, hi) = track.pitch_range();
        let key = track.effective_key();
        let pitches: Vec<u8> = (lo..=hi)
            .rev()
            .filter(|&p| key.is_none_or(|k| k.contains(p)))
            .collect();
        Rows {
            pitches,
            h: if key.is_some() { 20.0 } else { 15.0 },
            drums: false,
            locked: key.is_some(),
        }
    }

    fn row_of(&self, pitch: u8) -> Option<usize> {
        self.pitches.iter().position(|&p| p == pitch)
    }

    /// Pitch steps (see `edit::shift_pitch`) for moving down by `drow` rows.
    fn steps_for_rows(&self, drow: i32) -> i32 {
        if self.drums { drow } else { -drow }
    }
}

struct Geo {
    grid: Rect,
    scroll: f32,
    h: f32,
}

impl Geo {
    fn row_top(&self, row: usize) -> f32 {
        self.grid.top() + row as f32 * self.h - self.scroll
    }

    fn row_at(&self, y: f32) -> i32 {
        ((y - self.grid.top() + self.scroll) / self.h).floor() as i32
    }

    fn row_at_f(&self, y: f32) -> f32 {
        (y - self.grid.top() + self.scroll) / self.h
    }
}

fn note_rect(app: &OrchestreApp, geo: &Geo, rows: &Rows, n: &Note) -> Option<Rect> {
    let row = rows.row_of(n.pitch)?;
    let x0 = app.view.tick_to_x(geo.grid.left(), n.start as f64);
    let x1 = app.view.tick_to_x(geo.grid.left(), n.end() as f64);
    let y = geo.row_top(row);
    Some(Rect::from_min_max(
        pos2(x0, y + 1.0),
        pos2(x1.max(x0 + 3.0), y + geo.h - 1.0),
    ))
}

enum Hit {
    Body(Id),
    Edge(Id),
}

fn hit_test(app: &OrchestreApp, geo: &Geo, rows: &Rows, track: &Track, pos: Pos2) -> Option<Hit> {
    track.notes.iter().rev().find_map(|n| {
        let r = note_rect(app, geo, rows, n)?;
        if !r.contains(pos) {
            return None;
        }
        let edge = (r.width() / 3.0).min(7.0);
        Some(if !rows.drums && pos.x >= r.right() - edge {
            Hit::Edge(n.id)
        } else {
            Hit::Body(n.id)
        })
    })
}

fn with_track(app: &mut OrchestreApp, f: impl FnOnce(&mut Track)) {
    if let Some(t) = app.selected.and_then(|id| app.project.track_mut(id)) {
        f(t);
        app.touch();
    }
}

/// Put the original copies of dragged notes back, before re-applying the drag.
fn restore(track: &mut Track, orig: &[Note]) {
    for o in orig {
        if let Some(n) = track.notes.iter_mut().find(|n| n.id == o.id) {
            *n = *o;
        }
    }
}

pub fn show(app: &mut OrchestreApp, ui: &mut Ui, rect: Rect) {
    let Some(track) = app.selected_track().cloned() else {
        return;
    };
    let rows = Rows::for_track(&track);
    let color = theme::track_color(track.color);

    let ruler = Rect::from_min_max(
        pos2(rect.left() + KEYS_W, rect.top()),
        pos2(rect.right(), rect.top() + RULER_H),
    );
    let corner = Rect::from_min_max(rect.min, pos2(rect.left() + KEYS_W, rect.top() + RULER_H));
    let keys = Rect::from_min_max(
        pos2(rect.left(), rect.top() + RULER_H),
        pos2(rect.left() + KEYS_W, rect.bottom()),
    );
    let grid = Rect::from_min_max(pos2(rect.left() + KEYS_W, rect.top() + RULER_H), rect.max);

    // Vertical scroll, initially centered on the track's notes (or middle C).
    let content_h = rows.pitches.len() as f32 * rows.h;
    let max_scroll = (content_h - grid.height()).max(0.0);
    let center_row = {
        let mut pitches: Vec<u8> = track.notes.iter().map(|n| n.pitch).collect();
        pitches.sort_unstable();
        let target = pitches.get(pitches.len() / 2).copied().unwrap_or(60);
        rows.pitches.iter().position(|&p| p <= target).unwrap_or(0)
    };
    let initial = if rows.drums {
        0.0
    } else {
        center_row as f32 * rows.h - grid.height() / 2.0
    };
    let dy = timeline::navigate(ui, grid.union(keys), &mut app.view);
    let scroll = app.view.roll_scroll.entry(track.id).or_insert(initial);
    *scroll = (*scroll - dy).clamp(0.0, max_scroll);
    let geo = Geo {
        grid,
        scroll: *scroll,
        h: rows.h,
    };

    timeline::ruler(app, ui, ruler);
    draw_corner(ui, corner, &track);

    let resp = ui.interact(
        grid,
        ui.id().with(("roll", track.id)),
        Sense::click_and_drag(),
    );
    handle_mouse(app, ui, &resp, &geo, &rows, &track);

    // Re-read: the interaction may have changed the notes.
    let Some(track) = app.selected_track().cloned() else {
        return;
    };
    let p = ui.painter_at(grid);
    draw_rows(&p, &geo, &rows);
    timeline::draw_grid(&p, grid, &app.view, &app.project, true);
    draw_notes(app, &p, &geo, &rows, &track, color);
    draw_ghost(app, ui, &p, &geo, &rows, &track, color, &resp);
    if let Some(Drag::Select {
        origin, current, ..
    }) = &app.drag
    {
        let x0 = app.view.tick_to_x(grid.left(), origin.0);
        let x1 = app.view.tick_to_x(grid.left(), current.0);
        let y0 = grid.top() + origin.1 * geo.h - geo.scroll;
        let y1 = grid.top() + current.1 * geo.h - geo.scroll;
        let r = Rect::from_two_pos(pos2(x0, y0), pos2(x1, y1));
        p.rect_filled(r, 2.0, theme::SELECT.gamma_multiply(0.15));
        p.rect_stroke(r, 2.0, Stroke::new(1.0, theme::SELECT), StrokeKind::Inside);
    }
    timeline::draw_playhead(&p, grid, &app.view, app.position);
    if track.notes.is_empty() {
        p.text(
            grid.center(),
            Align2::CENTER_CENTER,
            "Click to add a note, or press and drag to set its length",
            FontId::proportional(16.0),
            Color32::from_white_alpha(70),
        );
    }

    draw_keys(app, ui, keys, &geo, &rows, &track);
}

fn draw_corner(ui: &Ui, corner: Rect, track: &Track) {
    let p = ui.painter_at(corner);
    p.rect_filled(corner, 0.0, theme::PANEL_LIGHT);
    let text = match track.effective_key() {
        Some(k) => format!("🔒 {}", k.label()),
        None if track.instrument.is_drums() => "Drums".into(),
        None => "Notes".into(),
    };
    p.text(
        corner.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(10.5),
        theme::TEXT_DIM,
    );
}

fn draw_rows(p: &egui::Painter, geo: &Geo, rows: &Rows) {
    p.rect_filled(geo.grid, 0.0, theme::GRID_BG);
    for (i, &pitch) in rows.pitches.iter().enumerate() {
        let y = geo.row_top(i);
        if y > geo.grid.bottom() || y + geo.h < geo.grid.top() {
            continue;
        }
        let r = Rect::from_x_y_ranges(geo.grid.x_range(), y..=y + geo.h);
        let dark = if rows.drums {
            i % 2 == 1
        } else {
            !rows.locked && is_black_key(pitch)
        };
        if dark {
            p.rect_filled(r, 0.0, theme::ROW_DARK);
        }
        // Emphasize octave boundaries, and the kit / percussion split.
        let octave = !rows.drums && pitch % 12 == 0 && !rows.locked;
        let kit_end = rows.drums && pitch as usize + 1 == DrumPiece::KIT_LEN;
        if octave || kit_end {
            p.hline(
                geo.grid.x_range(),
                y + geo.h,
                Stroke::new(1.0, theme::LINE_BEAT),
            );
        } else {
            p.hline(
                geo.grid.x_range(),
                y + geo.h,
                Stroke::new(1.0, Color32::from_black_alpha(60)),
            );
        }
    }
}

fn draw_notes(
    app: &OrchestreApp,
    p: &egui::Painter,
    geo: &Geo,
    rows: &Rows,
    track: &Track,
    color: Color32,
) {
    let font = FontId::proportional(10.5);
    for n in &track.notes {
        let Some(r) = note_rect(app, geo, rows, n) else {
            continue;
        };
        if r.right() < geo.grid.left()
            || r.left() > geo.grid.right()
            || r.bottom() < geo.grid.top()
            || r.top() > geo.grid.bottom()
        {
            continue;
        }
        let selected = app.selection.contains(&n.id);
        let fill = if selected {
            color.lerp_to_gamma(Color32::WHITE, 0.45)
        } else {
            color.gamma_multiply(0.55 + 0.45 * n.vel)
        };
        p.rect_filled(r, 3.0, fill);
        let stroke = if selected {
            Stroke::new(1.5, Color32::WHITE)
        } else {
            Stroke::new(1.0, Color32::from_black_alpha(90))
        };
        p.rect_stroke(r, 3.0, stroke, StrokeKind::Inside);
        if !rows.drums && r.width() > 30.0 && geo.h >= 14.0 {
            p.text(
                pos2(r.left() + 4.0, r.center().y),
                Align2::LEFT_CENTER,
                pitch_name(n.pitch),
                font.clone(),
                Color32::from_black_alpha(200),
            );
        }
    }
}

/// Faint preview of the note a click would add.
#[allow(clippy::too_many_arguments)]
fn draw_ghost(
    app: &OrchestreApp,
    ui: &Ui,
    p: &egui::Painter,
    geo: &Geo,
    rows: &Rows,
    track: &Track,
    color: Color32,
    resp: &egui::Response,
) {
    if app.drag.is_some() || !app.selection.is_empty() {
        return;
    }
    let Some(pos) = resp.hover_pos() else { return };
    if hit_test(app, geo, rows, track, pos).is_some() || ui.input(|i| i.pointer.any_down()) {
        return;
    }
    let row = geo.row_at(pos.y);
    let Some(&pitch) = usize::try_from(row).ok().and_then(|r| rows.pitches.get(r)) else {
        return;
    };
    let step = app.project.grid_ticks();
    let start = snap_floor(app.view.x_to_tick(geo.grid.left(), pos.x) as Tick, step);
    let len = if rows.drums { step } else { app.note_len };
    let ghost = Note {
        id: 0,
        start,
        len,
        pitch,
        vel: 0.8,
    };
    if let Some(r) = note_rect(app, geo, rows, &ghost) {
        p.rect_filled(r, 3.0, color.gamma_multiply(0.25));
        p.rect_stroke(
            r,
            3.0,
            Stroke::new(1.0, color.gamma_multiply(0.6)),
            StrokeKind::Inside,
        );
    }
}

fn draw_keys(
    app: &mut OrchestreApp,
    ui: &mut Ui,
    keys: Rect,
    geo: &Geo,
    rows: &Rows,
    track: &Track,
) {
    let p = ui.painter_at(keys);
    p.rect_filled(keys, 0.0, theme::PANEL);
    let held: HashSet<u8> = app
        .held_keys
        .values()
        .filter(|(t, _)| *t == track.id)
        .map(|(_, p)| *p)
        .collect();
    let font = FontId::proportional(if rows.drums { 11.0 } else { 10.0 });
    let accent = theme::track_color(track.color);
    for (i, &pitch) in rows.pitches.iter().enumerate() {
        let y = geo.row_top(i);
        if y > keys.bottom() || y + geo.h < keys.top() {
            continue;
        }
        let r = Rect::from_x_y_ranges(keys.x_range(), y..=y + geo.h).shrink2(vec2(0.0, 0.5));
        let playing = held.contains(&pitch);
        let (fill, text_color) = if rows.drums {
            (
                if i % 2 == 0 {
                    theme::PANEL_LIGHT
                } else {
                    theme::PANEL
                },
                Color32::from_gray(210),
            )
        } else if is_black_key(pitch) {
            (Color32::from_gray(35), Color32::from_gray(170))
        } else {
            (Color32::from_gray(215), Color32::from_gray(40))
        };
        let fill = if playing { accent } else { fill };
        p.rect_filled(r, 2.0, fill);
        let label = if rows.drums {
            Some(DrumPiece::ALL[pitch as usize].label().to_string())
        } else if rows.locked || pitch % 12 == 0 {
            Some(pitch_name(pitch))
        } else {
            None
        };
        if let Some(label) = label {
            p.text(
                pos2(r.right() - 5.0, r.center().y),
                Align2::RIGHT_CENTER,
                label,
                font.clone(),
                text_color,
            );
        }
        // Keycap showing which computer key plays this row.
        if let Some(letter) = crate::input::key_label_for(app, pitch, rows.drums) {
            let size = (geo.h - 3.0).clamp(11.0, 17.0);
            let cap = Rect::from_center_size(
                pos2(r.left() + 4.0 + size / 2.0, r.center().y),
                vec2(size, size),
            );
            let (cap_fill, cap_text) = if playing {
                (Color32::WHITE, Color32::BLACK)
            } else {
                (Color32::from_rgb(70, 76, 94), Color32::from_gray(235))
            };
            p.rect_filled(cap, 3.0, cap_fill);
            p.text(
                cap.center(),
                Align2::CENTER_CENTER,
                letter,
                FontId::monospace(size * 0.68),
                cap_text,
            );
        }
    }
    let resp = ui.interact(keys, ui.id().with(("keys", track.id)), Sense::click());
    if resp.clicked()
        && let Some(pos) = resp.interact_pointer_pos()
        && let Some(&pitch) = usize::try_from(geo.row_at(pos.y))
            .ok()
            .and_then(|r| rows.pitches.get(r))
    {
        app.audition(track.id, pitch);
    }
    resp.on_hover_text(
        "Click to hear it. The small key labels show which computer keys play each row.",
    );
}

fn handle_mouse(
    app: &mut OrchestreApp,
    ui: &Ui,
    resp: &egui::Response,
    geo: &Geo,
    rows: &Rows,
    track: &Track,
) {
    let (shift, alt, command) =
        ui.input(|i| (i.modifiers.shift, i.modifiers.alt, i.modifiers.command));
    let step = app.project.grid_ticks();
    let left = geo.grid.left();

    app.hover_tick = resp
        .hover_pos()
        .map(|p| app.view.x_to_tick(left, p.x).max(0.0) as Tick);
    if let Some(pos) = resp.hover_pos()
        && app.drag.is_none()
    {
        match hit_test(app, geo, rows, track, pos) {
            Some(Hit::Edge(_)) => ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal),
            Some(Hit::Body(_)) => ui.ctx().set_cursor_icon(CursorIcon::Grab),
            None => {}
        }
    }

    if resp.drag_started()
        && let Some(origin) = ui.input(|i| i.pointer.press_origin())
    {
        let tick = app.view.x_to_tick(left, origin.x);
        let grab_note = |app: &mut OrchestreApp, id: Id| {
            if !app.selection.contains(&id) {
                if !shift {
                    app.selection.clear();
                }
                app.selection.insert(id);
            }
            track
                .notes
                .iter()
                .filter(|n| app.selection.contains(&n.id))
                .copied()
                .collect::<Vec<_>>()
        };
        app.drag = match hit_test(app, geo, rows, track, origin) {
            Some(Hit::Edge(id)) => {
                let orig = grab_note(app, id);
                Some(Drag::Resize {
                    grab: id,
                    anchor_tick: tick,
                    orig,
                })
            }
            Some(Hit::Body(id)) => {
                let orig = grab_note(app, id);
                Some(Drag::Move {
                    grab: id,
                    anchor_tick: tick,
                    anchor_row: geo.row_at(origin.y),
                    orig,
                    last_pitch: track.note(id).map(|n| n.pitch),
                })
            }
            // Shift or Ctrl/Cmd + drag on empty space: select an area
            // (Shift adds to the current selection).
            None if shift || command => {
                let base = if shift {
                    app.selection.clone()
                } else {
                    HashSet::new()
                };
                app.selection = base.clone();
                let at = (tick, geo.row_at_f(origin.y));
                Some(Drag::Select {
                    origin: at,
                    current: at,
                    base,
                })
            }
            // Plain drag on empty space: draw a note (or paint drum hits).
            None => {
                app.selection.clear();
                let len = if rows.drums {
                    step
                } else {
                    step.min(app.note_len)
                };
                add_note_at(app, geo, rows, track, origin, len).map(|(id, start, pitch)| {
                    if rows.drums {
                        Drag::Paint { pitch }
                    } else {
                        Drag::Draw { id, start }
                    }
                })
            }
        };
    }

    if resp.dragged()
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let tick = app.view.x_to_tick(left, pos.x);
        let row = geo.row_at(pos.y);
        let row_f = geo.row_at_f(pos.y);
        match app.drag.take() {
            Some(Drag::Move {
                grab,
                anchor_tick,
                anchor_row,
                orig,
                last_pitch,
            }) => {
                let raw = (tick - anchor_tick).round() as Tick;
                let g = orig.iter().find(|n| n.id == grab).copied();
                let dt = match g {
                    Some(g) if !alt => snap_round(g.start + raw, step) - g.start,
                    _ => raw,
                };
                let steps = rows.steps_for_rows(row - anchor_row);
                let ids: HashSet<Id> = orig.iter().map(|n| n.id).collect();
                let mut new_pitch = None;
                with_track(app, |t| {
                    restore(t, &orig);
                    edit::move_notes(t, &ids, dt, steps);
                    new_pitch = t.note(grab).map(|n| n.pitch);
                });
                if new_pitch != last_pitch
                    && let Some(p) = new_pitch
                {
                    app.audition(track.id, p);
                }
                app.drag = Some(Drag::Move {
                    grab,
                    anchor_tick,
                    anchor_row,
                    orig,
                    last_pitch: new_pitch,
                });
            }
            Some(Drag::Resize {
                grab,
                anchor_tick,
                orig,
            }) => {
                let raw = (tick - anchor_tick).round() as Tick;
                if let Some(g) = orig.iter().find(|n| n.id == grab).copied() {
                    let new_end = if alt {
                        g.end() + raw
                    } else {
                        snap_round(g.end() + raw, step)
                    };
                    let min_len = if alt { PPQ / 32 } else { step };
                    let dlen = new_end - g.end();
                    let ids: HashSet<Id> = orig.iter().map(|n| n.id).collect();
                    with_track(app, |t| {
                        restore(t, &orig);
                        edit::resize_notes(t, &ids, dlen, min_len);
                    });
                    app.note_len = (g.len + dlen).max(min_len);
                }
                app.drag = Some(Drag::Resize {
                    grab,
                    anchor_tick,
                    orig,
                });
            }
            Some(Drag::Draw { id, start }) => {
                let len = drawn_length(start, tick, step, alt);
                with_track(app, |t| {
                    if let Some(n) = t.notes.iter_mut().find(|n| n.id == id) {
                        n.len = len;
                    }
                });
                // The next plain click reuses this length.
                app.note_len = len;
                app.drag = Some(Drag::Draw { id, start });
            }
            Some(Drag::Paint { pitch }) => {
                let at = snap_floor(tick.max(0.0) as Tick, step);
                if !app
                    .selected_track()
                    .is_some_and(|t| t.notes.iter().any(|n| n.start == at && n.pitch == pitch))
                {
                    let id = app.project.new_id();
                    with_track(app, |t| {
                        t.notes.push(Note {
                            id,
                            start: at,
                            len: step,
                            pitch,
                            vel: 0.8,
                        })
                    });
                    app.audition(track.id, pitch);
                }
                app.drag = Some(Drag::Paint { pitch });
            }
            Some(Drag::Select { origin, base, .. }) => {
                let (t0, t1) = (origin.0.min(tick), origin.0.max(tick));
                let (r0, r1) = (
                    origin.1.min(row_f).floor().max(0.0) as usize,
                    origin.1.max(row_f).floor().max(0.0) as usize,
                );
                let in_rows: Vec<u8> = rows
                    .pitches
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| (r0..=r1).contains(i))
                    .map(|(_, p)| *p)
                    .collect();
                let mut sel = base.clone();
                if let (Some(&lo), Some(&hi)) = (in_rows.iter().min(), in_rows.iter().max()) {
                    sel.extend(edit::notes_in_rect(
                        track,
                        t0 as Tick,
                        t1.ceil() as Tick,
                        lo,
                        hi,
                    ));
                }
                app.selection = sel;
                app.drag = Some(Drag::Select {
                    origin,
                    current: (tick, row_f),
                    base,
                });
            }
            None => {}
        }
        // Auto-scroll when dragging past the edges.
        let margin = 24.0;
        if pos.x > geo.grid.right() - margin {
            app.view.scroll_by_px(-6.0);
        } else if pos.x < geo.grid.left() + margin && app.view.scroll > 0.0 {
            app.view.scroll_by_px(6.0);
        }
    }

    if resp.drag_stopped() {
        app.drag = None;
    }

    if resp.clicked()
        && let Some(pos) = resp.interact_pointer_pos()
    {
        match hit_test(app, geo, rows, track, pos) {
            Some(Hit::Body(id) | Hit::Edge(id)) => {
                if shift {
                    if !app.selection.remove(&id) {
                        app.selection.insert(id);
                    }
                } else {
                    app.selection = HashSet::from([id]);
                }
                if let Some(n) = track.note(id) {
                    app.audition(track.id, n.pitch);
                }
            }
            None if !app.selection.is_empty() && !shift => app.selection.clear(),
            None => {
                let len = if rows.drums { step } else { app.note_len };
                add_note_at(app, geo, rows, track, pos, len);
            }
        }
    }

    if resp.secondary_clicked()
        && let Some(pos) = resp.interact_pointer_pos()
        && let Some(Hit::Body(id) | Hit::Edge(id)) = hit_test(app, geo, rows, track, pos)
    {
        app.selection.remove(&id);
        with_track(app, |t| edit::delete_notes(t, &HashSet::from([id])));
    }
}

/// Add a note under `pos` (snapped to the grid). Returns (id, start, pitch).
fn add_note_at(
    app: &mut OrchestreApp,
    geo: &Geo,
    rows: &Rows,
    track: &Track,
    pos: Pos2,
    len: Tick,
) -> Option<(Id, Tick, u8)> {
    let &pitch = usize::try_from(geo.row_at(pos.y))
        .ok()
        .and_then(|r| rows.pitches.get(r))?;
    let step = app.project.grid_ticks();
    let start = snap_floor(app.view.x_to_tick(geo.grid.left(), pos.x) as Tick, step).max(0);
    if track
        .notes
        .iter()
        .any(|n| n.start == start && n.pitch == pitch)
    {
        return None;
    }
    let id = app.project.new_id();
    with_track(app, |t| {
        t.notes.push(Note {
            id,
            start,
            len,
            pitch,
            vel: 0.8,
        })
    });
    app.audition(track.id, pitch);
    Some((id, start, pitch))
}

/// Length of a note being drawn from `start` to the pointer at `tick`:
/// snapped to the grid (at least one step), or free with Alt held.
fn drawn_length(start: Tick, tick: f64, step: Tick, free: bool) -> Tick {
    if free {
        (tick.round() as Tick - start).max(PPQ / 32)
    } else {
        (snap_round(tick.round() as Tick, step) - start).max(step)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawn_length_snaps_and_never_collapses() {
        let step = PPQ / 4;
        assert_eq!(drawn_length(0, 700.0, step, false), 720);
        assert_eq!(
            drawn_length(960, 1000.0, step, false),
            step,
            "at least one step"
        );
        assert_eq!(
            drawn_length(960, 500.0, step, false),
            step,
            "dragging backwards"
        );
        assert_eq!(drawn_length(0, 700.0, step, true), 700);
        assert_eq!(drawn_length(0, 3.0, step, true), PPQ / 32);
    }
}
