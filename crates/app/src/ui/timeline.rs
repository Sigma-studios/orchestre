//! Drawing and navigation shared by the note editor and the track lanes.

use egui::{Align2, Color32, FontId, Painter, Rect, Sense, Stroke, Ui, pos2};
use orchestre_core::Project;

use crate::app::{OrchestreApp, View};
use crate::theme;

/// Width of everything left of the timeline (sidebar + keyboard in the
/// editor, track headers in the lanes), so both timelines line up.
pub const HEADER_W: f32 = 318.0;
pub const KEYS_W: f32 = 92.0;
pub const RULER_H: f32 = 22.0;

/// Alternating bar shading plus bar / beat / grid lines.
pub fn draw_grid(p: &Painter, rect: Rect, view: &View, project: &Project, dense: bool) {
    let ts = project.time_sig;
    let bar = ts.bar_ticks();
    let beat = ts.beat_ticks();
    let grid = project.grid_ticks();
    let ppt = view.px_per_tick();
    let t0 = view.x_to_tick(rect.left(), rect.left()).max(0.0) as i64;
    let t1 = view.x_to_tick(rect.left(), rect.right()) as i64 + 1;

    let first_bar = t0 / bar;
    for b in first_bar..=(t1 / bar) {
        if b % 2 == 1 {
            let x0 = view
                .tick_to_x(rect.left(), (b * bar) as f64)
                .max(rect.left());
            let x1 = view
                .tick_to_x(rect.left(), ((b + 1) * bar) as f64)
                .min(rect.right());
            if x1 > x0 {
                p.rect_filled(
                    Rect::from_x_y_ranges(x0..=x1, rect.y_range()),
                    0.0,
                    Color32::from_white_alpha(5),
                );
            }
        }
    }

    // Choose the finest subdivision that is still readable.
    let sub = if dense && grid as f32 * ppt >= 7.0 {
        Some(grid)
    } else if beat as f32 * ppt >= 7.0 {
        Some(beat)
    } else {
        None
    };
    let step = sub.unwrap_or(bar);
    let mut t = t0 / step * step;
    while t <= t1 {
        let x = view.tick_to_x(rect.left(), t as f64);
        if x >= rect.left() {
            let color = if t % bar == 0 {
                theme::LINE_BAR
            } else if t % beat == 0 {
                theme::LINE_BEAT
            } else {
                theme::LINE_SUB
            };
            p.vline(x, rect.y_range(), Stroke::new(1.0, color));
        }
        t += step;
    }

    // Dim the area after the end of the song.
    let end_x = view.tick_to_x(rect.left(), project.length_ticks() as f64);
    if end_x < rect.right() {
        let r = Rect::from_x_y_ranges(end_x.max(rect.left())..=rect.right(), rect.y_range());
        p.rect_filled(r, 0.0, Color32::from_black_alpha(90));
    }
}

pub fn draw_playhead(p: &Painter, rect: Rect, view: &View, position: f64) {
    let x = view.tick_to_x(rect.left(), position);
    if x >= rect.left() && x <= rect.right() {
        p.vline(x, rect.y_range(), Stroke::new(2.0, theme::PLAYHEAD));
    }
}

/// Horizontal scroll (trackpad / Shift+wheel) and zoom (Ctrl/Cmd+wheel,
/// pinch) when the pointer is over `rect`. Returns the unused vertical
/// scroll delta, for callers that scroll vertically themselves.
pub fn navigate(ui: &Ui, rect: Rect, view: &mut View) -> f32 {
    let Some(pos) = ui.input(|i| i.pointer.hover_pos()) else {
        return 0.0;
    };
    if !rect.contains(pos) {
        return 0.0;
    }
    let (delta, zoom, shift) =
        ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.modifiers.shift));
    if zoom != 1.0 {
        view.zoom_at(rect.left(), pos.x, zoom);
        return 0.0;
    }
    if shift && delta.x == 0.0 {
        view.scroll_by_px(delta.y);
        return 0.0;
    }
    if delta.x != 0.0 {
        view.scroll_by_px(delta.x);
    }
    delta.y
}

/// Bar-number ruler. Click or drag to move the playhead.
pub fn ruler(app: &mut OrchestreApp, ui: &mut Ui, rect: Rect) {
    let resp = ui.interact(rect, ui.id().with("ruler"), Sense::click_and_drag());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, theme::PANEL_LIGHT);
    let ts = app.project.time_sig;
    let bar = ts.bar_ticks();
    let view = &app.view;
    let bar_px = bar as f32 * view.px_per_tick();
    // Label every n-th bar so numbers don't overlap.
    let every = [1, 2, 4, 8, 16, 32]
        .into_iter()
        .find(|&n| n as f32 * bar_px >= 28.0)
        .unwrap_or(64);
    let t0 = view.x_to_tick(rect.left(), rect.left()).max(0.0) as i64 / bar;
    let t1 = view.x_to_tick(rect.left(), rect.right()) as i64 / bar + 1;
    for b in t0..=t1 {
        let x = view.tick_to_x(rect.left(), (b * bar) as f64);
        let major = b % every == 0;
        let h = if major {
            rect.height()
        } else {
            rect.height() * 0.35
        };
        p.vline(
            x,
            (rect.bottom() - h)..=rect.bottom(),
            Stroke::new(1.0, theme::LINE_BAR),
        );
        if major {
            p.text(
                pos2(x + 4.0, rect.center().y),
                Align2::LEFT_CENTER,
                format!("{}", b + 1),
                FontId::proportional(12.0),
                theme::TEXT_DIM,
            );
        }
    }
    let end_x = view.tick_to_x(rect.left(), app.project.length_ticks() as f64);
    if end_x < rect.right() {
        p.rect_filled(
            Rect::from_x_y_ranges(end_x.max(rect.left())..=rect.right(), rect.y_range()),
            0.0,
            Color32::from_black_alpha(90),
        );
    }
    let x = view.tick_to_x(rect.left(), app.position);
    if x >= rect.left() && x <= rect.right() {
        let tri = vec![
            pos2(x - 6.0, rect.top()),
            pos2(x + 6.0, rect.top()),
            pos2(x, rect.top() + 9.0),
        ];
        p.add(egui::Shape::convex_polygon(
            tri,
            theme::PLAYHEAD,
            Stroke::NONE,
        ));
        p.vline(x, rect.y_range(), Stroke::new(2.0, theme::PLAYHEAD));
    }
    if (resp.clicked() || resp.dragged())
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let step = app.project.grid_ticks();
        let tick = app.view.x_to_tick(rect.left(), pos.x).max(0.0) as i64;
        app.seek(orchestre_core::snap_round(tick, step) as f64);
    }
    resp.on_hover_text("Click to move the playhead");
}
