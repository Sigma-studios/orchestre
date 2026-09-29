//! The patch editor (expert): modules as boxes, cables between their
//! ports. Outputs are on the right of a module, inputs on the left.

use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2, pos2, vec2};
use orchestre_core::patch::{MODULE_KINDS, Macro, Module, NodeId, Patch};
use orchestre_core::{FilterType, Id, Instrument, LFO_BEATS, LfoWave, Wave};

use crate::app::OrchestreApp;
use crate::theme;

const NODE_W: f32 = 214.0;
const TITLE_H: f32 = 22.0;
const PORT_H: f32 = 18.0;
const PARAM_H: f32 = 22.0;
const PORT_R: f32 = 5.0;

#[derive(Default)]
pub struct PatchView {
    /// Track whose patch is open.
    pub track: Option<Id>,
    pan: Vec2,
    /// Cable being drawn from this output.
    wiring: Option<(NodeId, u8)>,
    /// Where the "add module" menu was opened, in patch coordinates.
    menu_at: Option<Vec2>,
}

pub fn open(app: &mut OrchestreApp, track: Id) {
    app.patch_view.track = Some(track);
    app.patch_view.pan = Vec2::ZERO;
}

pub fn window(app: &mut OrchestreApp, ctx: &egui::Context) {
    let Some(id) = app.patch_view.track else {
        return;
    };
    let Some(track) = app.project.track(id) else {
        app.patch_view.track = None;
        return;
    };
    let Instrument::Patch(patch) = &track.instrument else {
        app.patch_view.track = None;
        return;
    };
    let before = (**patch).clone();
    let mut patch = before.clone();
    let mut open = true;
    let screen = ctx.content_rect();
    egui::Window::new(format!("Patch editor — {}", track.name))
        .id(egui::Id::new("patch_editor"))
        .open(&mut open)
        .default_pos(screen.min + vec2(20.0, 40.0))
        .default_size([screen.width() - 60.0, screen.height() - 120.0])
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| {
            toolbar(app, ui, &mut patch);
            ui.separator();
            canvas(&mut app.patch_view, ui, &mut patch);
        });
    if !open {
        app.patch_view.track = None;
    }
    if patch != before
        && let Some(t) = app.project.track_mut(id)
    {
        t.instrument = Instrument::Patch(Box::new(patch));
        app.touch();
    }
}

fn toolbar(app: &mut OrchestreApp, ui: &mut Ui, p: &mut Patch) {
    ui.horizontal(|ui| {
        ui.menu_button("+ Add module", |ui| {
            for (kind, desc) in MODULE_KINDS {
                if ui.button(kind).on_hover_text(desc).clicked() {
                    let at = -app.patch_view.pan + vec2(60.0, 60.0);
                    if let Some(m) = Module::new(kind) {
                        p.add(m, [at.x, at.y]);
                    }
                    ui.close();
                }
            }
        });
        ui.separator();
        ui.checkbox(&mut p.mono, "One note at a time");
        if p.mono {
            ui.add(
                egui::Slider::new(&mut p.glide, 0.0..=0.5)
                    .show_value(false)
                    .text("Glide"),
            );
        }
        ui.add(
            egui::Slider::new(&mut p.chorus, 0.0..=1.0)
                .show_value(false)
                .text("Chorus"),
        );
        ui.add(
            egui::Slider::new(&mut p.gain, 0.0..=1.2)
                .show_value(false)
                .text("Level"),
        );
    });
    ui.label(
        RichText::new(
            "Drag from an output (right side) to an input (left side) to connect. \
             Drag the number on a cable to set how much it acts. Right-click a module, \
             cable or setting for more; right-click the background to add modules.",
        )
        .size(11.0)
        .color(theme::TEXT_DIM),
    );
}

/// Rows of settings drawn inside a module.
fn param_rows(m: &Module) -> usize {
    match m {
        Module::Osc { .. } => 4,
        Module::Lfo { beats, .. } => {
            if *beats > 0.0 {
                2
            } else {
                3
            }
        }
        Module::Envelope { .. } => 4,
        Module::Filter { .. } => 3,
        Module::Amp { .. } | Module::Drive { .. } => 1,
        Module::Mixer { .. } => 4,
        _ => 0,
    }
}

fn node_size(m: &Module) -> Vec2 {
    let ports = m.inputs().len().max(m.outputs().len());
    vec2(
        NODE_W,
        TITLE_H + ports as f32 * PORT_H + param_rows(m) as f32 * PARAM_H + 8.0,
    )
}

fn title_color(m: &Module) -> Color32 {
    match m {
        Module::Osc { .. } | Module::Noise => Color32::from_rgb(70, 120, 190),
        Module::Filter { .. } | Module::Drive { .. } | Module::Ring => {
            Color32::from_rgb(170, 110, 60)
        }
        Module::Envelope { .. } | Module::Lfo { .. } | Module::Keyboard => {
            Color32::from_rgb(110, 90, 170)
        }
        Module::Output => Color32::from_rgb(70, 150, 90),
        Module::Amp { .. } | Module::Mixer { .. } => Color32::from_rgb(90, 96, 112),
    }
}

fn node_rect(origin: Pos2, pos: [f32; 2], m: &Module) -> Rect {
    Rect::from_min_size(origin + vec2(pos[0], pos[1]), node_size(m))
}

fn input_pos(r: Rect, i: usize) -> Pos2 {
    pos2(r.left(), r.top() + TITLE_H + (i as f32 + 0.5) * PORT_H)
}

fn output_pos(r: Rect, i: usize) -> Pos2 {
    pos2(r.right(), r.top() + TITLE_H + (i as f32 + 0.5) * PORT_H)
}

fn cable_shape(a: Pos2, b: Pos2, color: Color32) -> egui::epaint::CubicBezierShape {
    let d = ((b.x - a.x).abs() * 0.5).max(40.0);
    egui::epaint::CubicBezierShape::from_points_stroke(
        [a, a + vec2(d, 0.0), b - vec2(d, 0.0), b],
        false,
        Color32::TRANSPARENT,
        Stroke::new(2.5, color),
    )
}

fn canvas(view: &mut PatchView, ui: &mut Ui, p: &mut Patch) {
    let area = ui.available_rect_before_wrap();
    let bg = ui.interact(area, ui.id().with("patch_bg"), Sense::click_and_drag());
    if bg.dragged() {
        view.pan += bg.drag_delta();
    }
    let origin = area.min + view.pan;
    if bg.secondary_clicked()
        && let Some(pos) = bg.interact_pointer_pos()
    {
        view.menu_at = Some(pos - origin);
    }
    bg.context_menu(|ui| {
        ui.label(RichText::new("Add module").color(theme::TEXT_DIM));
        for (kind, desc) in MODULE_KINDS {
            if ui.button(kind).on_hover_text(desc).clicked() {
                let at = view.menu_at.unwrap_or(vec2(40.0, 40.0));
                if let Some(m) = Module::new(kind) {
                    p.add(m, [at.x, at.y]);
                }
                ui.close();
            }
        }
    });

    let painter = ui.painter_at(area);
    painter.rect_filled(area, 0.0, theme::GRID_BG);
    // Dot grid, moving with the pan.
    let step = 24.0;
    let off = vec2(view.pan.x.rem_euclid(step), view.pan.y.rem_euclid(step));
    let mut y = area.top() + off.y;
    while y < area.bottom() {
        let mut x = area.left() + off.x;
        while x < area.right() {
            painter.circle_filled(pos2(x, y), 1.0, theme::LINE_SUB);
            x += step;
        }
        y += step;
    }

    // Cables, under the modules.
    let rects: Vec<(NodeId, Rect)> = p
        .nodes
        .iter()
        .map(|n| (n.id, node_rect(origin, n.pos, &n.module)))
        .collect();
    let rect_of = |id: NodeId| rects.iter().find(|r| r.0 == id).map(|r| r.1);
    let cable_ends = |c: &orchestre_core::patch::Cable| {
        let (a, b) = (rect_of(c.from)?, rect_of(c.to)?);
        Some((
            output_pos(a, c.from_port as usize),
            input_pos(b, c.to_port as usize),
        ))
    };
    for c in &p.cables {
        if let Some((a, b)) = cable_ends(c) {
            painter.add(cable_shape(a, b, theme::ACCENT.gamma_multiply(0.8)));
        }
    }

    // Modules.
    let mut delete = None;
    let mut add_macro = None;
    let mut duplicate = None;
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let mut drop_target = None;
    for node in p.nodes.iter_mut() {
        let r = node_rect(origin, node.pos, &node.module);
        if !r.intersects(area) {
            continue;
        }
        painter.rect_filled(r, 6.0, theme::PANEL_LIGHT);
        painter.rect_stroke(
            r,
            6.0,
            Stroke::new(1.0, theme::LINE_BEAT),
            egui::StrokeKind::Inside,
        );
        let title = Rect::from_min_size(r.min, vec2(r.width(), TITLE_H));
        painter.rect_filled(
            title,
            egui::CornerRadius {
                nw: 6,
                ne: 6,
                sw: 0,
                se: 0,
            },
            title_color(&node.module),
        );
        painter.text(
            title.left_center() + vec2(8.0, 0.0),
            Align2::LEFT_CENTER,
            node.module.label(),
            FontId::proportional(13.0),
            Color32::WHITE,
        );
        let tr = ui.interact(
            title,
            ui.id().with(("node", node.id)),
            Sense::click_and_drag(),
        );
        if tr.dragged() {
            let d = tr.drag_delta();
            node.pos[0] += d.x;
            node.pos[1] += d.y;
        }
        let deletable = !matches!(node.module, Module::Output);
        tr.context_menu(|ui| {
            if deletable && ui.button("Duplicate").clicked() {
                duplicate = Some(node.id);
                ui.close();
            }
            if deletable && ui.button("Delete module").clicked() {
                delete = Some(node.id);
                ui.close();
            }
            if !deletable {
                ui.label("Every patch needs its output");
            }
        });

        let font = FontId::proportional(11.0);
        for (i, (name, _)) in node.module.inputs().iter().enumerate() {
            let pt = input_pos(r, i);
            let hot = view.wiring.is_some() && pointer.is_some_and(|q| q.distance(pt) < 12.0);
            if hot {
                drop_target = Some((node.id, i as u8));
            }
            painter.circle(
                pt,
                if hot { PORT_R + 2.0 } else { PORT_R },
                theme::BG,
                Stroke::new(1.5, theme::TEXT_DIM),
            );
            painter.text(
                pt + vec2(9.0, 0.0),
                Align2::LEFT_CENTER,
                *name,
                font.clone(),
                theme::TEXT_DIM,
            );
        }
        for (i, name) in node.module.outputs().iter().enumerate() {
            let pt = output_pos(r, i);
            painter.circle_filled(pt, PORT_R, theme::ACCENT);
            painter.text(
                pt - vec2(9.0, 0.0),
                Align2::RIGHT_CENTER,
                *name,
                font.clone(),
                theme::TEXT_DIM,
            );
            let pr = ui.interact(
                Rect::from_center_size(pt, Vec2::splat(14.0)),
                ui.id().with(("out", node.id, i)),
                Sense::drag(),
            );
            if pr.drag_started() {
                view.wiring = Some((node.id, i as u8));
            }
            if pr.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            }
        }

        let ports = node.module.inputs().len().max(node.module.outputs().len());
        let params = Rect::from_min_max(
            pos2(
                r.left() + 8.0,
                r.top() + TITLE_H + ports as f32 * PORT_H + 2.0,
            ),
            pos2(r.right() - 8.0, r.bottom() - 4.0),
        );
        if param_rows(&node.module) > 0 {
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(params)
                    .id_salt(("params", node.id)),
            );
            child.set_clip_rect(area.intersect(r));
            child.spacing_mut().slider_width = 64.0;
            child.spacing_mut().item_spacing.y = 2.0;
            if let Some(i) = module_params(&mut child, &mut node.module) {
                add_macro = Some((node.id, i));
            }
        }
    }

    // Cable amounts, drawn over the modules so they are never hidden.
    let mut delete_cable = None;
    for (ci, c) in p.cables.iter_mut().enumerate() {
        let Some((a, b)) = cable_ends(c) else {
            continue;
        };
        let mid = Rect::from_center_size(a + (b - a) * 0.5, vec2(44.0, 16.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(mid).id_salt(("cable", ci)));
        child.set_clip_rect(area);
        child.style_mut().spacing.interact_size.y = 14.0;
        let resp = child
            .add(
                egui::DragValue::new(&mut c.amount)
                    .speed(0.02)
                    .range(-48.0..=48.0)
                    .max_decimals(2),
            )
            .on_hover_text("How much this cable acts · right-click to remove");
        resp.context_menu(|ui| {
            if ui.button("Remove cable").clicked() {
                delete_cable = Some(ci);
                ui.close();
            }
        });
    }
    if let Some(ci) = delete_cable {
        p.cables.remove(ci);
    }

    // A cable being drawn follows the pointer until dropped on an input.
    if let Some((from, port)) = view.wiring {
        if let (Some(r), Some(q)) = (rect_of(from), pointer) {
            painter.add(cable_shape(output_pos(r, port as usize), q, theme::SELECT));
        }
        if ui.input(|i| !i.pointer.any_down()) {
            if let Some((to, to_port)) = drop_target {
                p.connect(from, port, to, to_port);
            }
            view.wiring = None;
        }
    }
    if let Some(id) = duplicate
        && let Some(n) = p.node(id).cloned()
    {
        p.add(n.module, [n.pos[0] + 30.0, n.pos[1] + 30.0]);
    }
    if let Some(id) = delete {
        p.remove(id);
    }
    if let Some((node, param)) = add_macro {
        let name = p
            .node_mut(node)
            .and_then(|n| {
                let label = n.module.label();
                n.module
                    .params_mut()
                    .get(param as usize)
                    .map(|(name, _, _)| format!("{label} {}", name.to_lowercase()))
            })
            .unwrap_or_default();
        if !p.macros.iter().any(|m| m.node == node && m.param == param) {
            p.macros.push(Macro { name, node, param });
        }
    }
}

/// The settings inside a module. Returns a setting to show in the easy
/// view, when asked for with a right-click.
fn module_params(ui: &mut Ui, m: &mut Module) -> Option<u8> {
    let mut picked = None;
    // Right-click on a number setting offers it as a knob in the sidebar.
    let mut offer = |resp: egui::Response, index: u8| {
        resp.context_menu(|ui| {
            if ui.button("Show as a knob in the sidebar").clicked() {
                picked = Some(index);
                ui.close();
            }
        });
    };
    match m {
        Module::Osc {
            wave,
            semitones,
            detune,
            width,
        } => {
            egui::ComboBox::from_id_salt("wave")
                .width(100.0)
                .selected_text(wave.label())
                .show_ui(ui, |ui| {
                    for w in Wave::ALL {
                        ui.selectable_value(wave, w, w.label());
                    }
                });
            offer(
                ui.add(slider(semitones, -24.0..=24.0, "pitch").step_by(1.0)),
                0,
            );
            offer(ui.add(slider(detune, -50.0..=50.0, "detune")), 1);
            offer(ui.add(slider(width, 0.05..=0.95, "width")), 2);
        }
        Module::Lfo { wave, rate, beats } => {
            egui::ComboBox::from_id_salt("lfo")
                .width(100.0)
                .selected_text(wave.label())
                .show_ui(ui, |ui| {
                    for w in LfoWave::ALL {
                        ui.selectable_value(wave, w, w.label());
                    }
                });
            let current = LFO_BEATS
                .iter()
                .find(|b| (b.0 - *beats).abs() < 1e-4)
                .map_or("Custom", |b| b.1);
            egui::ComboBox::from_id_salt("sync")
                .width(100.0)
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for (b, label) in LFO_BEATS {
                        ui.selectable_value(beats, b, label);
                    }
                });
            if *beats <= 0.0 {
                offer(ui.add(slider(rate, 0.05..=20.0, "Hz").logarithmic(true)), 0);
            }
        }
        Module::Envelope { adsr } => {
            offer(
                ui.add(slider(&mut adsr.attack, 0.001..=3.0, "attack").logarithmic(true)),
                0,
            );
            offer(
                ui.add(slider(&mut adsr.decay, 0.001..=4.0, "decay").logarithmic(true)),
                1,
            );
            offer(ui.add(slider(&mut adsr.sustain, 0.0..=1.0, "sustain")), 2);
            offer(
                ui.add(slider(&mut adsr.release, 0.001..=5.0, "release").logarithmic(true)),
                3,
            );
        }
        Module::Filter {
            kind,
            cutoff,
            resonance,
        } => {
            egui::ComboBox::from_id_salt("kind")
                .width(130.0)
                .selected_text(kind.label())
                .show_ui(ui, |ui| {
                    for f in FilterType::ALL {
                        ui.selectable_value(kind, f, f.label());
                    }
                });
            offer(
                ui.add(slider(cutoff, 30.0..=16000.0, "cutoff").logarithmic(true)),
                0,
            );
            offer(ui.add(slider(resonance, 0.0..=0.98, "res")), 1);
        }
        Module::Amp { level } => offer(ui.add(slider(level, 0.0..=1.0, "level")), 0),
        Module::Mixer { levels } => {
            for (i, l) in levels.iter_mut().enumerate() {
                offer(
                    ui.add(slider(l, 0.0..=1.0, &format!("in {}", i + 1))),
                    i as u8,
                );
            }
        }
        Module::Drive { amount } => offer(ui.add(slider(amount, 0.0..=1.0, "amount")), 0),
        Module::Keyboard | Module::Noise | Module::Ring | Module::Output => {}
    }
    picked
}

fn slider<'a>(
    v: &'a mut f32,
    range: std::ops::RangeInclusive<f32>,
    text: &str,
) -> egui::Slider<'a> {
    egui::Slider::new(v, range).text(text.to_string())
}
