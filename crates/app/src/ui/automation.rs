//! Automation strip under the note editor: draw how a setting (filter,
//! volume, echo…) changes over the song.

use egui::{Align2, Color32, CursorIcon, FontId, Pos2, Rect, RichText, Sense, Stroke, Ui, pos2};
use orchestre_core::{AutoLane, AutoPoint, AutoTarget, Instrument, Tick, Track, snap_round};

use crate::app::OrchestreApp;
use crate::theme;
use crate::ui::timeline::{self, KEYS_W};

/// Height of the strip's header bar, and of the lane when open.
pub const BAR_H: f32 = 24.0;
pub const LANE_H: f32 = 120.0;
const POINT_R: f32 = 5.0;

pub struct AutoView {
    pub open: bool,
    pub target: AutoTarget,
    /// Point being dragged, by index in its lane.
    drag: Option<usize>,
}

impl Default for AutoView {
    fn default() -> Self {
        AutoView {
            open: false,
            target: AutoTarget::Filter,
            drag: None,
        }
    }
}

/// Height the strip needs.
pub fn height(app: &OrchestreApp) -> f32 {
    if app.automation.open {
        BAR_H + LANE_H
    } else {
        BAR_H
    }
}

fn targets(track: &Track) -> Vec<AutoTarget> {
    AutoTarget::ALL
        .into_iter()
        .filter(|&t| {
            t != AutoTarget::Brightness || matches!(track.instrument, Instrument::Synth(_))
        })
        .collect()
}

pub fn show(app: &mut OrchestreApp, ui: &mut Ui, rect: Rect) {
    let Some(track) = app.selected_track().cloned() else {
        return;
    };
    let bar = Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + BAR_H));
    ui.painter().rect_filled(bar, 0.0, theme::PANEL_LIGHT);
    if !targets(&track).contains(&app.automation.target) {
        app.automation.target = AutoTarget::Filter;
    }
    let target = app.automation.target;

    let mut bar_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(bar.shrink2(egui::vec2(6.0, 1.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let automated = track.automation.len();
    let label = match (app.automation.open, automated) {
        (true, _) => "⏷ Automation".to_string(),
        (false, 0) => "⏵ Automation".to_string(),
        (false, n) => format!("⏵ Automation ({n})"),
    };
    if bar_ui
        .selectable_label(app.automation.open, label)
        .on_hover_text("Draw how a setting changes over the song, like a filter sweep")
        .clicked()
    {
        app.automation.open = !app.automation.open;
    }
    if app.automation.open {
        egui::ComboBox::from_id_salt("auto_target")
            .width(150.0)
            .selected_text(target.label())
            .show_ui(&mut bar_ui, |ui| {
                for t in targets(&track) {
                    let text = if track.lane(t).is_some() {
                        format!("● {}", t.label())
                    } else {
                        t.label().to_string()
                    };
                    ui.selectable_value(&mut app.automation.target, t, text);
                }
            });
        if track.lane(target).is_some()
            && bar_ui
                .button("Clear")
                .on_hover_text("Remove this automation: the setting goes back to its knob")
                .clicked()
        {
            edit(app, |t| t.automation.retain(|l| l.target != target));
        }
        bar_ui.label(
            RichText::new("Click: add point · drag: move · right-click: delete · Alt: no snapping")
                .size(11.0)
                .color(theme::TEXT_DIM),
        );
    }
    if !app.automation.open {
        return;
    }

    let lane_r = Rect::from_min_max(pos2(rect.left() + KEYS_W, bar.bottom()), rect.max);
    let label_r = Rect::from_min_max(
        pos2(rect.left(), bar.bottom()),
        pos2(lane_r.left(), rect.bottom()),
    );
    ui.painter().rect_filled(label_r, 0.0, theme::PANEL);
    let (lo, hi) = target.range();
    for (v, text) in [
        (hi, end_label(target, true)),
        (lo, end_label(target, false)),
    ] {
        let y = value_to_y(lane_r, v, lo, hi);
        ui.painter().text(
            pos2(label_r.right() - 6.0, y),
            Align2::RIGHT_CENTER,
            text,
            FontId::proportional(10.5),
            theme::TEXT_DIM,
        );
    }

    timeline::navigate(ui, lane_r, &mut app.view);
    let resp = ui.interact(
        lane_r,
        ui.id().with(("auto", track.id)),
        Sense::click_and_drag(),
    );
    interact(app, ui, &resp, lane_r, &track, target);

    let Some(track) = app.selected_track().cloned() else {
        return;
    };
    let p = ui.painter_at(lane_r);
    p.rect_filled(lane_r, 0.0, theme::GRID_BG);
    timeline::draw_grid(&p, lane_r, &app.view, &app.project, false);
    let color = theme::track_color(track.color);
    if lo < 0.0 {
        let y = value_to_y(lane_r, 0.0, lo, hi);
        p.hline(lane_r.x_range(), y, Stroke::new(1.0, theme::LINE_BEAT));
    }
    match track.lane(target) {
        Some(lane) => draw_lane(app, &p, lane_r, lane, lo, hi, color),
        None => {
            let y = value_to_y(lane_r, target.current(&track), lo, hi);
            p.hline(
                lane_r.x_range(),
                y,
                Stroke::new(1.5, color.gamma_multiply(0.35)),
            );
            p.text(
                lane_r.center(),
                Align2::CENTER_CENTER,
                format!("Click to start drawing {}", target.label().to_lowercase()),
                FontId::proportional(14.0),
                Color32::from_white_alpha(70),
            );
        }
    }
    timeline::draw_playhead(&p, lane_r, &app.view, app.estimated_position());
}

fn end_label(target: AutoTarget, top: bool) -> &'static str {
    match (target, top) {
        (AutoTarget::Filter, true) => "thin",
        (AutoTarget::Filter, false) => "muffled",
        (AutoTarget::Pan, true) => "right",
        (AutoTarget::Pan, false) => "left",
        (AutoTarget::Brightness, true) => "bright",
        (AutoTarget::Brightness, false) => "dark",
        (_, true) => "max",
        (_, false) => "off",
    }
}

fn value_to_y(r: Rect, v: f32, lo: f32, hi: f32) -> f32 {
    let inner = r.shrink2(egui::vec2(0.0, 8.0));
    inner.bottom() - (v - lo) / (hi - lo) * inner.height()
}

fn y_to_value(r: Rect, y: f32, lo: f32, hi: f32) -> f32 {
    let inner = r.shrink2(egui::vec2(0.0, 8.0));
    let v = lo + (inner.bottom() - y) / inner.height() * (hi - lo);
    // Settle on the middle of bipolar settings (off / centre) when near it.
    let v = if lo < 0.0 && v.abs() < (hi - lo) * 0.02 {
        0.0
    } else {
        v
    };
    v.clamp(lo, hi)
}

fn point_pos(app: &OrchestreApp, r: Rect, pt: &AutoPoint, lo: f32, hi: f32) -> Pos2 {
    pos2(
        app.view.tick_to_x(r.left(), pt.tick as f64),
        value_to_y(r, pt.value, lo, hi),
    )
}

fn edit(app: &mut OrchestreApp, f: impl FnOnce(&mut Track)) {
    if let Some(t) = app.selected.and_then(|id| app.project.track_mut(id)) {
        f(t);
        app.touch();
    }
}

fn interact(
    app: &mut OrchestreApp,
    ui: &Ui,
    resp: &egui::Response,
    r: Rect,
    track: &Track,
    target: AutoTarget,
) {
    let (lo, hi) = target.range();
    let alt = ui.input(|i| i.modifiers.alt);
    let snap = |app: &OrchestreApp, x: f32| -> Tick {
        let t = app.view.x_to_tick(r.left(), x).max(0.0).round() as Tick;
        if alt {
            t
        } else {
            snap_round(t, app.project.grid_ticks())
        }
    };
    let lane = track.lane(target);
    let hovered = resp.hover_pos().and_then(|pos| {
        lane?
            .points
            .iter()
            .position(|pt| point_pos(app, r, pt, lo, hi).distance(pos) <= POINT_R + 3.0)
    });
    if hovered.is_some() || app.automation.drag.is_some() {
        ui.ctx().set_cursor_icon(CursorIcon::Grab);
    }

    if resp.secondary_clicked()
        && let Some(i) = hovered
    {
        edit(app, |t| {
            if let Some(l) = t.automation.iter_mut().find(|l| l.target == target) {
                l.points.remove(i);
            }
            t.automation.retain(|l| !l.points.is_empty());
        });
        return;
    }

    if resp.drag_started() || resp.clicked() {
        let Some(pos) = resp.interact_pointer_pos() else {
            return;
        };
        let index = match hovered {
            Some(i) => i,
            None => {
                let point = AutoPoint {
                    tick: snap(app, pos.x),
                    value: y_to_value(r, pos.y, lo, hi),
                };
                let mut index = 0;
                edit(app, |t| {
                    if t.lane(target).is_none() {
                        t.automation.push(AutoLane {
                            target,
                            points: Vec::new(),
                        });
                    }
                    let l = t
                        .automation
                        .iter_mut()
                        .find(|l| l.target == target)
                        .unwrap();
                    // One point per tick: replace any already there.
                    l.points.retain(|p| p.tick != point.tick);
                    l.points.push(point);
                    l.sort();
                    index = l.points.iter().position(|p| *p == point).unwrap_or(0);
                });
                index
            }
        };
        if resp.drag_started() {
            app.automation.drag = Some(index);
        }
    }

    if let Some(i) = app.automation.drag {
        if !resp.dragged() {
            app.automation.drag = None;
            return;
        }
        let Some(pos) = resp.interact_pointer_pos() else {
            return;
        };
        let moved = AutoPoint {
            tick: snap(app, pos.x),
            value: y_to_value(r, pos.y, lo, hi),
        };
        let mut new_index = i;
        edit(app, |t| {
            let Some(l) = t.automation.iter_mut().find(|l| l.target == target) else {
                return;
            };
            if i >= l.points.len() {
                return;
            }
            l.points[i] = moved;
            l.sort();
            new_index = l.points.iter().position(|p| *p == moved).unwrap_or(i);
        });
        app.automation.drag = Some(new_index);
    }
}

fn draw_lane(
    app: &OrchestreApp,
    p: &egui::Painter,
    r: Rect,
    lane: &AutoLane,
    lo: f32,
    hi: f32,
    color: Color32,
) {
    let pts: Vec<Pos2> = lane
        .points
        .iter()
        .map(|pt| point_pos(app, r, pt, lo, hi))
        .collect();
    let (Some(first), Some(last)) = (pts.first(), pts.last()) else {
        return;
    };
    let mut line = vec![pos2(r.left().min(first.x), first.y)];
    line.extend(pts.iter().copied());
    line.push(pos2(r.right().max(last.x), last.y));
    let base = value_to_y(r, if lo < 0.0 { 0.0 } else { lo }, lo, hi);
    for w in line.windows(2) {
        let (a, b) = (w[0], w[1]);
        if b.x < r.left() || a.x > r.right() {
            continue;
        }
        // Fill towards the centre line; a segment crossing it is split
        // there so each piece stays convex.
        let fill = color.gamma_multiply(0.12);
        let (da, db) = (a.y - base, b.y - base);
        let pieces = if da * db < 0.0 {
            let c = pos2(a.x + (b.x - a.x) * da / (da - db), base);
            vec![vec![a, c, pos2(a.x, base)], vec![c, b, pos2(b.x, base)]]
        } else {
            vec![vec![a, b, pos2(b.x, base), pos2(a.x, base)]]
        };
        for piece in pieces {
            p.add(egui::Shape::convex_polygon(piece, fill, Stroke::NONE));
        }
    }
    p.add(egui::Shape::line(line, Stroke::new(2.0, color)));
    for pt in pts {
        p.circle(pt, POINT_R, theme::BG, Stroke::new(2.0, color));
    }
}
