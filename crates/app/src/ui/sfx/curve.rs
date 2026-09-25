//! Editing a value that changes over a layer's length, and a vowel chart.

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use orchestre_core::sfx::{Curve, VOWEL_NAMES};

use crate::theme;

pub struct Spec<'a> {
    pub label: &'a str,
    pub lo: f32,
    pub hi: f32,
    /// Spread values evenly by ratio (frequencies).
    pub log: bool,
    /// The layer's length in seconds, to show times.
    pub len: f32,
    pub format: &'a dyn Fn(f32) -> String,
}

impl Spec<'_> {
    fn norm(&self, v: f32) -> f32 {
        let n = if self.log {
            (v.max(1e-3) / self.lo).ln() / (self.hi / self.lo).ln()
        } else {
            (v - self.lo) / (self.hi - self.lo)
        };
        n.clamp(0.0, 1.0)
    }

    fn value(&self, n: f32) -> f32 {
        let n = n.clamp(0.0, 1.0);
        if self.log {
            self.lo * (self.hi / self.lo).powf(n)
        } else {
            self.lo + (self.hi - self.lo) * n
        }
    }
}

const HEIGHT: f32 = 58.0;
const GRAB: f32 = 8.0;

/// A small graph of `curve` over the layer's length. Click to add a point,
/// drag one to move it, right-click one to remove it. Returns whether the
/// curve changed.
pub fn edit(ui: &mut Ui, curve: &mut Curve, spec: &Spec) -> bool {
    let width = ui.available_width().clamp(160.0, 420.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, HEIGHT), Sense::click_and_drag());
    let area = rect.shrink2(vec2(6.0, 6.0));
    let to_screen = |t: f32, v: f32| {
        pos2(
            area.left() + t * area.width(),
            area.bottom() - spec.norm(v) * area.height(),
        )
    };
    let from_screen = |p: Pos2| {
        let t = ((p.x - area.left()) / area.width()).clamp(0.0, 1.0);
        let v = spec.value((area.bottom() - p.y) / area.height());
        (t, v)
    };
    let nearest = |p: Pos2, c: &Curve| {
        c.points()
            .iter()
            .enumerate()
            .map(|(i, q)| (i, to_screen(q[0], q[1]).distance(p)))
            .filter(|&(_, d)| d <= GRAB)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    };

    let id = resp.id.with("point");
    let mut changed = false;
    if resp.drag_started()
        && let Some(pos) = resp.interact_pointer_pos()
    {
        // Dragging on empty space starts a new point there.
        let index = match nearest(pos, curve) {
            Some(i) => Some(i),
            None => {
                let (t, v) = from_screen(pos);
                let i = curve.insert(t, v);
                changed |= i.is_some();
                i
            }
        };
        ui.data_mut(|d| d.insert_temp(id, index));
    }
    if resp.dragged()
        && let Some(pos) = resp.interact_pointer_pos()
        && let Some(Some(i)) = ui.data(|d| d.get_temp::<Option<usize>>(id))
    {
        let (t, v) = from_screen(pos);
        let before = curve.points()[i];
        curve.set(i, t, v);
        changed |= curve.points()[i] != before;
    }
    if resp.drag_stopped() {
        ui.data_mut(|d| d.remove::<Option<usize>>(id));
    }
    if resp.clicked()
        && let Some(pos) = resp.interact_pointer_pos()
        && nearest(pos, curve).is_none()
    {
        let (t, v) = from_screen(pos);
        changed |= curve.insert(t, v).is_some();
    }
    if resp.secondary_clicked()
        && let Some(pos) = resp.interact_pointer_pos()
        && let Some(i) = nearest(pos, curve)
    {
        curve.remove(i);
        changed = true;
    }

    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, theme::GRID_BG);
    for k in 1..4 {
        let x = area.left() + area.width() * k as f32 / 4.0;
        p.vline(x, area.y_range(), Stroke::new(1.0, theme::LINE_SUB));
    }
    p.text(
        rect.left_top() + vec2(6.0, 3.0),
        Align2::LEFT_TOP,
        spec.label,
        FontId::proportional(11.0),
        theme::TEXT_DIM,
    );
    let pts = curve.points();
    let mut line = vec![to_screen(0.0, pts[0][1])];
    line.extend(pts.iter().map(|q| to_screen(q[0], q[1])));
    line.push(to_screen(1.0, pts[pts.len() - 1][1]));
    p.add(egui::Shape::line(line, Stroke::new(1.5, theme::ACCENT)));
    let hover = resp.hover_pos().and_then(|pos| nearest(pos, curve));
    for (i, q) in pts.iter().enumerate() {
        let r = if hover == Some(i) { 5.0 } else { 3.5 };
        p.circle_filled(to_screen(q[0], q[1]), r, theme::ACCENT);
    }
    if let Some(pos) = resp.hover_pos() {
        let (t, v) = match hover {
            Some(i) => (pts[i][0], pts[i][1]),
            None => (from_screen(pos).0, curve.at(from_screen(pos).0)),
        };
        let text = format!(
            "{} at {}",
            (spec.format)(v),
            super::format_secs(t * spec.len)
        );
        p.text(
            rect.right_top() + vec2(-6.0, 3.0),
            Align2::RIGHT_TOP,
            text,
            FontId::proportional(11.0),
            Color32::WHITE,
        );
    }
    resp.on_hover_text(
        "Over the layer's length. Click: add a point · drag: move it · right-click: remove it",
    );
    changed
}

/// Where the vowel goes over the layer, on the classic vowel chart (front
/// on the left, open at the bottom).
pub fn vowel_chart(ui: &mut Ui, openness: &Curve, frontness: &Curve) {
    let (rect, _) = ui.allocate_exact_size(vec2(170.0, 110.0), Sense::hover());
    let area = rect.shrink2(vec2(18.0, 12.0));
    let at = |o: f32, f: f32| {
        pos2(
            area.right() - f * area.width(),
            area.top() + o * area.height(),
        )
    };
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, theme::GRID_BG);
    p.rect_stroke(
        Rect::from_min_max(at(0.0, 1.0), at(1.0, 0.0)),
        0.0,
        Stroke::new(1.0, theme::LINE_BEAT),
        egui::StrokeKind::Middle,
    );
    for (name, o, f) in VOWEL_NAMES {
        p.text(
            at(o, f),
            Align2::CENTER_CENTER,
            name,
            FontId::proportional(11.0),
            theme::TEXT_DIM,
        );
    }
    let path: Vec<Pos2> = (0..=32)
        .map(|i| {
            let t = i as f32 / 32.0;
            at(openness.at(t), frontness.at(t))
        })
        .collect();
    let (start, end) = (path[0], path[path.len() - 1]);
    p.add(egui::Shape::line(path, Stroke::new(2.0, theme::ACCENT)));
    p.circle_stroke(start, 4.0, Stroke::new(1.5, theme::ACCENT));
    p.circle_filled(end, 3.5, theme::ACCENT);
    p.text(
        rect.left_top() + vec2(5.0, 2.0),
        Align2::LEFT_TOP,
        "Vowel path",
        FontId::proportional(10.0),
        theme::TEXT_DIM,
    );
}
