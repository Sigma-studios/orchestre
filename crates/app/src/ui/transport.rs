use egui::{Color32, RichText, Ui};
use orchestre_core::{Grid, TimeSig};

use crate::app::OrchestreApp;
use crate::theme;

pub fn show(app: &mut OrchestreApp, ui: &mut Ui) {
    egui::Panel::top("transport")
        .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(egui::Margin::symmetric(12, 8)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("♫ Orchestre").strong().size(18.0).color(theme::ACCENT));
                ui.add_space(12.0);

                file_menu(app, ui);
                ui.separator();

                let play_label = if app.playing { "⏹ Stop" } else { "▶ Play" };
                let play = egui::Button::new(RichText::new(play_label).size(15.0))
                    .fill(if app.playing { Color32::from_rgb(120, 60, 50) } else { Color32::from_rgb(50, 110, 70) })
                    .min_size(egui::vec2(84.0, 28.0));
                if ui.add(play).on_hover_text("Play / stop (Space)").clicked() {
                    app.toggle_play();
                }
                if ui.button("⏮").on_hover_text("Back to start (Enter)").clicked() {
                    app.seek(0.0);
                }
                ui.toggle_value(&mut app.loop_on, "🔁 Loop").on_hover_text("Loop the whole song");
                ui.toggle_value(&mut app.follow, "Follow").on_hover_text("Scroll to follow the playhead");

                ui.separator();
                position_display(app, ui);
                ui.separator();

                ui.label("Tempo");
                let mut bpm = app.project.bpm;
                let r = ui.add(egui::DragValue::new(&mut bpm).range(40.0..=240.0).speed(0.5).suffix(" bpm").fixed_decimals(0));
                if r.changed() {
                    app.project.bpm = bpm.round();
                    app.touch();
                }

                ui.label("Beat");
                let ts = app.project.time_sig;
                egui::ComboBox::from_id_salt("timesig").width(56.0).selected_text(ts.label()).show_ui(ui, |ui| {
                    for p in TimeSig::PRESETS {
                        if ui.selectable_label(p == ts, p.label()).clicked() && p != ts {
                            app.project.time_sig = p;
                            app.touch();
                        }
                    }
                });

                ui.label("Snap");
                let grid = app.project.grid;
                egui::ComboBox::from_id_salt("grid").width(90.0).selected_text(grid.label()).show_ui(ui, |ui| {
                    for g in Grid::ALL {
                        if ui.selectable_label(g == grid, g.label()).clicked() && g != grid {
                            app.project.grid = g;
                            app.touch();
                        }
                    }
                })
                .response
                .on_hover_text("Notes snap to this rhythm. Hold Alt while dragging to place freely.");

                ui.label("Bars");
                let mut bars = app.project.length_bars;
                if ui.add(egui::DragValue::new(&mut bars).range(1..=512)).on_hover_text("Song length").changed() {
                    let bar = app.project.time_sig.bar_ticks();
                    let min = ((app.project.content_end() + bar - 1) / bar).max(1) as u32;
                    app.project.length_bars = bars.max(min);
                    app.touch();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let master = app.levels.get(&0).copied().unwrap_or(0.0);
                    meter(ui, master, egui::vec2(90.0, 10.0));
                    #[cfg(not(target_arch = "wasm32"))]
                    if let Some(midi) = &app.midi {
                        ui.label(RichText::new("🎹 MIDI").color(theme::TEXT_DIM)).on_hover_text(midi.device_names.join("\n"));
                    }
                    keyboard_menu(app, ui)
                        .on_hover_text(
                            "Play the selected track with your keyboard.\n\
                             Melodic: the home row (A S D F… or Q S D F… on AZERTY) plays white keys, the row above plays black keys.\n\
                             Drums: each home-row key plays one drum, in order (kick, snare, clap…).\n\
                             Change octave with - and =",
                        );
                    if let Some(err) = app.audio.error() {
                        ui.label(RichText::new("⚠ No audio").color(Color32::from_rgb(240, 120, 100))).on_hover_text(err);
                    }
                    if ui.add_enabled(app.history.can_redo(), egui::Button::new("↪")).on_hover_text("Redo (Ctrl/Cmd+Shift+Z)").clicked() {
                        app.redo();
                    }
                    if ui.add_enabled(app.history.can_undo(), egui::Button::new("↩")).on_hover_text("Undo (Ctrl/Cmd+Z)").clicked() {
                        app.undo();
                    }
                });
            });
        });
}

fn file_menu(app: &mut OrchestreApp, ui: &mut Ui) {
    ui.menu_button("File", |ui| {
        if ui.button("New song").clicked() {
            app.replace_project(orchestre_core::Project::default());
            app.notify("New song (Undo brings the old one back)");
        }
        if ui.button("Demo song").clicked() {
            app.replace_project(orchestre_core::Project::demo());
        }
        ui.separator();
        if ui.button("Open…").clicked() {
            crate::io::open(app);
        }
        if ui.button("Save…").clicked() {
            crate::io::save(app);
        }
        ui.separator();
        if ui.button("Export WAV…").clicked() {
            crate::io::export_wav(app);
        }
    });
}

fn keyboard_menu(app: &mut OrchestreApp, ui: &mut Ui) -> egui::Response {
    let text = RichText::new(format!(
        "⌨ {} · Octave {}",
        app.kb_layout.label(),
        app.octave
    ))
    .color(theme::TEXT_DIM);
    ui.menu_button(text, |ui| {
        ui.label(RichText::new("Keyboard layout").strong());
        let auto = !app.kb_layout_manual;
        if ui
            .radio(
                auto,
                format!("Automatic (detected: {})", app.kb_layout.label()),
            )
            .clicked()
        {
            app.kb_layout_manual = false;
        }
        for l in crate::input::KbLayout::ALL {
            if ui
                .radio(app.kb_layout_manual && app.kb_layout == l, l.label())
                .clicked()
            {
                app.kb_layout = l;
                app.kb_layout_manual = true;
            }
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Octave");
            if ui.button("−").clicked() {
                app.octave = (app.octave - 1).max(0);
            }
            ui.label(app.octave.to_string());
            if ui.button("+").clicked() {
                app.octave = (app.octave + 1).min(8);
            }
        });
    })
    .response
}

fn position_display(app: &OrchestreApp, ui: &mut Ui) {
    let ts = app.project.time_sig;
    let t = app.position.max(0.0) as i64;
    let bar = t / ts.bar_ticks() + 1;
    let beat = (t % ts.bar_ticks()) / ts.beat_ticks() + 1;
    let secs = t as f64 * app.project.seconds_per_tick();
    ui.label(
        RichText::new(format!("{bar:>3}.{beat}"))
            .monospace()
            .size(16.0)
            .strong(),
    )
    .on_hover_text("Bar . beat");
    ui.label(
        RichText::new(format!("{}:{:04.1}", (secs / 60.0) as u32, secs % 60.0))
            .monospace()
            .color(theme::TEXT_DIM),
    );
}

/// Horizontal level meter.
pub fn meter(ui: &mut Ui, level: f32, size: egui::Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 3.0, theme::BG);
    // Map roughly -48..0 dB to the meter width.
    let db = 20.0 * level.max(1e-5).log10();
    let frac = ((db + 48.0) / 48.0).clamp(0.0, 1.0);
    let color = if level > 0.95 {
        Color32::from_rgb(240, 90, 80)
    } else if db > -6.0 {
        Color32::from_rgb(240, 200, 80)
    } else {
        Color32::from_rgb(90, 200, 120)
    };
    let mut fill = rect;
    fill.set_width(rect.width() * frac);
    p.rect_filled(fill, 3.0, color);
}
