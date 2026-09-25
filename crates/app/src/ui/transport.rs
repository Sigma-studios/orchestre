use egui::{Color32, RichText, Ui};
use orchestre_core::{Family, Key, Scale, TimeSig};

use crate::app::OrchestreApp;
use crate::theme;

pub fn show(app: &mut OrchestreApp, ui: &mut Ui) {
    egui::Panel::top("transport")
        .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(egui::Margin::symmetric(12, 8)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("♫").strong().size(20.0).color(theme::ACCENT)).on_hover_text("Orchestre");
                ui.add_space(4.0);

                file_menu(app, ui);
                ui.separator();
                crate::ui::sfx::mode_switch(app, ui);
                ui.separator();

                let running = app.playing || app.rec.counting.is_some();
                let play_label = if running { "⏹ Stop" } else { "▶ Play" };
                let play = egui::Button::new(RichText::new(play_label).size(15.0))
                    .fill(if running { Color32::from_rgb(120, 60, 50) } else { Color32::from_rgb(50, 110, 70) })
                    .min_size(egui::vec2(84.0, 28.0));
                if ui.add(play).on_hover_text("Play / stop (Space)").clicked() {
                    app.toggle_play();
                }
                record_controls(app, ui);
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

                key_picker(app, ui);

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
                    meter(ui, master, egui::vec2(60.0, 10.0));
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
        ui.separator();
        if ui.button("Settings…").clicked() {
            app.settings_open = true;
        }
    });
}

/// Song key: popular keys by major/minor feel, then the scale variants
/// that fit the chosen key.
fn key_picker(app: &mut OrchestreApp, ui: &mut Ui) {
    ui.label("Key");
    let current = app.project.key;
    let naming = app.naming();
    let text = current.map_or("None".to_string(), |k| k.label_in(naming));
    let mut pick: Option<Option<Key>> = None;
    egui::ComboBox::from_id_salt("songkey")
        .width(115.0)
        .height(520.0)
        .selected_text(text)
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(current.is_none(), "None (all notes)")
                .clicked()
            {
                pick = Some(None);
            }
            for (family, title) in [
                (Family::Major, "Major keys (happy, bright)"),
                (Family::Minor, "Minor keys (sad, moody)"),
            ] {
                ui.separator();
                ui.label(RichText::new(title).size(11.0).color(theme::TEXT_DIM));
                for preset in Key::PRESETS
                    .into_iter()
                    .filter(|k| k.scale.family() == family)
                {
                    let on = current.is_some_and(|k| k.base() == preset);
                    if ui.selectable_label(on, preset.label_in(naming)).clicked() {
                        // Keep the chosen scale variant when the feel doesn't change.
                        let scale = match current {
                            Some(k) if k.scale.family() == family => k.scale,
                            _ => preset.scale,
                        };
                        pick = Some(Some(Key::new(preset.root, scale)));
                    }
                }
            }
            if let Some(k) = current {
                ui.separator();
                ui.label(
                    RichText::new(format!("Notes to use in {}", k.base().label_in(naming)))
                        .size(11.0)
                        .color(theme::TEXT_DIM),
                );
                for scale in Scale::ALL
                    .into_iter()
                    .filter(|s| s.family() == k.scale.family())
                {
                    if ui
                        .selectable_label(k.scale == scale, capitalize(scale.label_in(naming)))
                        .on_hover_text(scale.description())
                        .clicked()
                    {
                        pick = Some(Some(Key::new(k.root, scale)));
                    }
                }
            }
        })
        .response
        .on_hover_text(
            "The song's key: tracks locked to it only show notes that sound good together",
        );
    if let Some(key) = pick
        && key != current
    {
        app.request_song_key(key);
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

fn record_controls(app: &mut OrchestreApp, ui: &mut Ui) {
    let target = app.selected_track().map(|t| t.name.clone());
    let recording = app.rec.active;
    let red = Color32::from_rgb(220, 60, 60);
    let text =
        RichText::new("⏺ Rec")
            .size(15.0)
            .color(if recording { Color32::WHITE } else { red });
    let button = egui::Button::new(text)
        .fill(if recording {
            red
        } else {
            ui.visuals().widgets.inactive.weak_bg_fill
        })
        .min_size(egui::vec2(60.0, 28.0));
    let hover = match &target {
        Some(name) if recording => format!("Recording into “{name}” — click to stop"),
        Some(name) => format!("Record what you play on the keyboard into “{name}”"),
        None => "Select a track first, then record into it".to_string(),
    };
    if ui
        .add_enabled(target.is_some() || recording, button)
        .on_hover_text(hover)
        .on_disabled_hover_text("Select a track first, then record into it")
        .clicked()
    {
        app.toggle_record();
    }
    ui.menu_button("⏷", |ui| {
        ui.label(RichText::new("Recording").strong());
        ui.checkbox(&mut app.rec.count_in, "Count in one bar")
            .on_hover_text("Clicks for a bar before recording starts");
        if ui
            .checkbox(&mut app.rec.click, "Metronome while recording")
            .changed()
            && app.rec.active
        {
            let on = app.rec.click;
            app.send(orchestre_dsp::Cmd::SetMetronome(on));
        }
        ui.checkbox(&mut app.rec.snap, "Snap notes to the grid")
            .on_hover_text("Recorded notes are moved to the nearest Snap position");
        ui.separator();
        ui.label(
            RichText::new("With Loop on, each pass adds notes on top.")
                .size(11.5)
                .color(theme::TEXT_DIM),
        );
    })
    .response
    .on_hover_text("Recording options");
}

fn keyboard_menu(app: &mut OrchestreApp, ui: &mut Ui) -> egui::Response {
    // Shown in the chosen naming: the octave from C4 is "3" in French.
    let shown = app.naming().octave(app.octave as i32);
    let text = RichText::new(format!("⌨ Oct {shown}")).color(theme::TEXT_DIM);
    ui.menu_button(text, |ui| {
        ui.label(RichText::new("Keyboard layout").strong());
        let auto = !app.settings.kb_layout_manual;
        if ui
            .radio(
                auto,
                format!("Automatic (detected: {})", app.settings.kb_layout.label()),
            )
            .clicked()
        {
            app.settings.kb_layout_manual = false;
        }
        for l in crate::input::KbLayout::ALL {
            if ui
                .radio(
                    app.settings.kb_layout_manual && app.settings.kb_layout == l,
                    l.label(),
                )
                .clicked()
            {
                app.settings.kb_layout = l;
                app.settings.kb_layout_manual = true;
            }
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Octave");
            if ui.button("−").clicked() {
                app.octave = (app.octave - 1).max(0);
            }
            ui.label(shown.to_string());
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
    let time = format!("{}:{:04.1}", (secs / 60.0) as u32, secs % 60.0);
    ui.label(
        RichText::new(format!("{bar:>3}.{beat}"))
            .monospace()
            .size(16.0)
            .strong(),
    )
    .on_hover_text(format!("Bar {bar}, beat {beat} ({time})"));
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
