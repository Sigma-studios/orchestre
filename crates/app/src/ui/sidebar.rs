//! Left sidebar of the note editor: instrument sound, key lock, mix.

use egui::{Color32, RichText, Slider, Ui};
use orchestre_core::edit::notes_outside_key;
use orchestre_core::{
    DrumKit, DrumParams, DrumPiece, FxParams, Instrument, Key, PPQ, PianoParams, SynthParams,
    SynthPreset, Track, Wave,
};

use crate::app::{KeyLockPrompt, OrchestreApp};
use crate::theme;

pub fn show(app: &mut OrchestreApp, ui: &mut Ui) {
    let Some(track) = app.selected_track().cloned() else {
        return;
    };
    let mut t = track.clone();

    ui.spacing_mut().slider_width = 95.0;
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                ui.painter()
                    .circle_filled(r.center(), 6.0, theme::track_color(t.color));
                ui.add(
                    egui::TextEdit::singleline(&mut t.name)
                        .desired_width(140.0)
                        .font(egui::TextStyle::Heading),
                );
                if ui.button("×").on_hover_text("Close editor (Esc)").clicked() {
                    app.select_track(None);
                }
            });
            ui.add_space(4.0);

            instrument_picker(ui, &mut t);
            if !t.instrument.is_drums() {
                key_lock_picker(app, ui, &track);
            }
            note_tools(app, ui, &mut t);
            ui.separator();

            match &mut t.instrument {
                Instrument::Synth(p) => synth_ui(ui, p),
                Instrument::Drums(p) => drums_ui(ui, p),
                Instrument::Piano(p) => piano_ui(ui, p),
            }
            ui.separator();
            mix_ui(ui, &mut t);
            ui.add_space(8.0);
            if ui
                .button(RichText::new("🗑 Delete track").color(Color32::from_rgb(240, 120, 100)))
                .clicked()
            {
                app.confirm_delete_track = Some(t.id);
            }
            ui.add_space(12.0);
            help(ui, t.instrument.is_drums());
        });

    if t != track
        && let Some(slot) = app.project.track_mut(t.id)
    {
        // Key lock changes go through the confirmation dialog, not here.
        let key_lock = slot.key_lock;
        *slot = Track { key_lock, ..t };
        app.touch();
    }
}

fn instrument_picker(ui: &mut Ui, t: &mut Track) {
    ui.horizontal(|ui| {
        ui.label("Sound");
        match &mut t.instrument {
            Instrument::Drums(p) => {
                egui::ComboBox::from_id_salt("kit")
                    .selected_text(p.kit.label())
                    .show_ui(ui, |ui| {
                        for kit in DrumKit::ALL {
                            if ui.selectable_label(p.kit == kit, kit.label()).clicked() {
                                *p = DrumParams {
                                    levels: p.levels,
                                    gain: p.gain,
                                    ..kit.params()
                                };
                            }
                        }
                    });
            }
            inst => {
                let current = inst.label();
                egui::ComboBox::from_id_salt("preset")
                    .width(140.0)
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(matches!(inst, Instrument::Piano(_)), "Piano")
                            .clicked()
                            && !matches!(inst, Instrument::Piano(_))
                        {
                            *inst = Instrument::Piano(PianoParams::default());
                        }
                        for preset in SynthPreset::ALL {
                            let on = matches!(inst, Instrument::Synth(p) if p.preset == preset);
                            if ui
                                .selectable_label(on, preset.label())
                                .on_hover_text(preset.description())
                                .clicked()
                            {
                                *inst = Instrument::Synth(preset.params());
                            }
                        }
                    })
                    .response
                    .on_hover_text("Choosing a sound resets its settings to that preset");
            }
        }
    });
}

fn key_lock_picker(app: &mut OrchestreApp, ui: &mut Ui, track: &Track) {
    ui.horizontal(|ui| {
        ui.label("Key");
        let text = track.key_lock.map_or("Off (all notes)".to_string(), |k| {
            format!("🔒 {}", k.label())
        });
        let mut choice: Option<Option<Key>> = None;
        egui::ComboBox::from_id_salt("key")
            .width(140.0)
            .selected_text(text)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(track.key_lock.is_none(), "Off (all notes)")
                    .clicked()
                {
                    choice = Some(None);
                }
                for k in Key::PRESETS {
                    if ui
                        .selectable_label(track.key_lock == Some(k), k.label())
                        .clicked()
                    {
                        choice = Some(Some(k));
                    }
                }
            })
            .response
            .on_hover_text(
                "Lock this track to a key: only notes that sound good together are shown.",
            );
        match choice {
            Some(None) if track.key_lock.is_some() => {
                if let Some(t) = app.project.track_mut(track.id) {
                    t.key_lock = None;
                }
                app.touch();
            }
            Some(Some(k)) if track.key_lock != Some(k) => {
                let conflicts = notes_outside_key(track, k).len();
                if conflicts == 0 {
                    if let Some(t) = app.project.track_mut(track.id) {
                        t.key_lock = Some(k);
                    }
                    app.touch();
                } else {
                    app.key_prompt = Some(KeyLockPrompt {
                        track: track.id,
                        key: k,
                        conflicts,
                    });
                }
            }
            _ => {}
        }
    });
}

fn note_tools(app: &mut OrchestreApp, ui: &mut Ui, t: &mut Track) {
    if !t.instrument.is_drums() {
        ui.horizontal(|ui| {
            ui.label("New notes");
            let lengths = [(PPQ / 4, "1/16"), (PPQ / 2, "1/8"), (PPQ, "1/4"), (PPQ * 2, "1/2"), (PPQ * 4, "Whole")];
            let current = lengths.iter().find(|l| l.0 == app.note_len).map_or("Custom", |l| l.1);
            egui::ComboBox::from_id_salt("notelen").width(80.0).selected_text(current).show_ui(ui, |ui| {
                for (len, label) in lengths {
                    ui.selectable_value(&mut app.note_len, len, label);
                }
            })
            .response
            .on_hover_text("Length of notes added by clicking. Dragging a note's right edge also changes it.");
        });
    }
    let sel: Vec<usize> = t
        .notes
        .iter()
        .enumerate()
        .filter(|(_, n)| app.selection.contains(&n.id))
        .map(|(i, _)| i)
        .collect();
    if !sel.is_empty() {
        let avg = sel.iter().map(|&i| t.notes[i].vel).sum::<f32>() / sel.len() as f32;
        let mut vel = avg;
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} selected", sel.len())).color(theme::SELECT));
            if ui
                .add(
                    Slider::new(&mut vel, 0.05..=1.0)
                        .show_value(false)
                        .text("Loudness"),
                )
                .changed()
            {
                for &i in &sel {
                    t.notes[i].vel = vel;
                }
            }
        });
    }
}

fn section(ui: &mut Ui, title: &str, open: bool, body: impl FnOnce(&mut Ui)) {
    egui::CollapsingHeader::new(RichText::new(title).strong())
        .default_open(open)
        .show(ui, body);
}

fn seconds(ui: &mut Ui, v: &mut f32, max: f32, label: &str) {
    ui.add(
        Slider::new(v, 0.001..=max)
            .logarithmic(true)
            .suffix(" s")
            .text(label),
    );
}

fn synth_ui(ui: &mut Ui, p: &mut SynthParams) {
    section(ui, "Sound", true, |ui| {
        ui.add(
            Slider::new(&mut p.cutoff, 60.0..=16000.0)
                .logarithmic(true)
                .show_value(false)
                .text("Brightness"),
        )
        .on_hover_text("Filter cutoff: darker ← → brighter");
        ui.add(
            Slider::new(&mut p.resonance, 0.0..=0.95)
                .show_value(false)
                .text("Resonance"),
        )
        .on_hover_text("Emphasis around the brightness point — gives a 'wah' character");
        seconds(ui, &mut p.amp.attack, 3.0, "Fade in");
        seconds(ui, &mut p.amp.release, 5.0, "Fade out");
        ui.add(
            Slider::new(&mut p.lfo_cutoff, 0.0..=3.0)
                .show_value(false)
                .text("Wobble"),
        )
        .on_hover_text("Rhythmic movement of the brightness");
        ui.add(
            Slider::new(&mut p.lfo_pitch, 0.0..=1.0)
                .show_value(false)
                .text("Vibrato"),
        );
        ui.add(
            Slider::new(&mut p.width, 0.0..=1.0)
                .show_value(false)
                .text("Stereo width"),
        );
        ui.add(
            Slider::new(&mut p.gain, 0.0..=1.2)
                .show_value(false)
                .text("Level"),
        );
        ui.horizontal(|ui| {
            ui.checkbox(&mut p.mono, "One note at a time")
                .on_hover_text("Mono: notes slide into each other — great for bass and leads");
            if p.mono {
                ui.add(
                    Slider::new(&mut p.glide, 0.0..=0.5)
                        .show_value(false)
                        .text("Glide"),
                );
            }
        });
    });
    section(ui, "Advanced", false, |ui| {
        ui.label(RichText::new("Oscillators").color(theme::TEXT_DIM));
        wave_combo(ui, "osc1", &mut p.osc1, "Wave 1");
        wave_combo(ui, "osc2", &mut p.osc2, "Wave 2");
        let mut semis = p.osc2_semitones as i32;
        if ui
            .add(Slider::new(&mut semis, -24..=24).text("Wave 2 pitch"))
            .changed()
        {
            p.osc2_semitones = semis as i8;
        }
        ui.add(
            Slider::new(&mut p.osc2_detune, -50.0..=50.0)
                .suffix(" ct")
                .text("Wave 2 detune"),
        );
        ui.add(Slider::new(&mut p.osc_mix, 0.0..=1.0).text("Mix 1 ↔ 2"));
        ui.add(Slider::new(&mut p.fm, 0.0..=5.0).text("FM (metallic)"));
        ui.add(Slider::new(&mut p.sub, 0.0..=1.0).text("Sub octave"));
        ui.add(Slider::new(&mut p.noise, 0.0..=1.0).text("Noise"));
        let mut uni = p.unison as i32;
        if ui
            .add(Slider::new(&mut uni, 1..=5).text("Unison voices"))
            .changed()
        {
            p.unison = uni as u8;
        }
        ui.add(
            Slider::new(&mut p.unison_spread, 0.0..=60.0)
                .suffix(" ct")
                .text("Unison detune"),
        );
        let mut oct = p.octave as i32;
        if ui
            .add(Slider::new(&mut oct, -3..=3).text("Octave"))
            .changed()
        {
            p.octave = oct as i8;
        }
        ui.label(RichText::new("Filter envelope").color(theme::TEXT_DIM));
        ui.add(
            Slider::new(&mut p.filter_env, 0.0..=6.0)
                .suffix(" oct")
                .text("Amount"),
        );
        seconds(ui, &mut p.filter_adsr.attack, 3.0, "Attack");
        seconds(ui, &mut p.filter_adsr.decay, 4.0, "Decay");
        ui.add(Slider::new(&mut p.filter_adsr.sustain, 0.0..=1.0).text("Sustain"));
        seconds(ui, &mut p.filter_adsr.release, 5.0, "Release");
        ui.label(RichText::new("Volume envelope").color(theme::TEXT_DIM));
        seconds(ui, &mut p.amp.attack, 3.0, "Attack");
        seconds(ui, &mut p.amp.decay, 4.0, "Decay");
        ui.add(Slider::new(&mut p.amp.sustain, 0.0..=1.0).text("Sustain"));
        seconds(ui, &mut p.amp.release, 5.0, "Release");
        ui.label(RichText::new("Wobble / vibrato").color(theme::TEXT_DIM));
        ui.add(
            Slider::new(&mut p.lfo_rate, 0.05..=20.0)
                .logarithmic(true)
                .suffix(" Hz")
                .text("Speed"),
        );
    });
}

fn wave_combo(ui: &mut Ui, id: &str, w: &mut Wave, label: &str) {
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt(id)
            .width(90.0)
            .selected_text(w.label())
            .show_ui(ui, |ui| {
                for wave in Wave::ALL {
                    ui.selectable_value(w, wave, wave.label());
                }
            });
        ui.label(label);
    });
}

fn drums_ui(ui: &mut Ui, p: &mut DrumParams) {
    section(ui, "Sound", true, |ui| {
        ui.add(
            Slider::new(&mut p.tune, -12.0..=12.0)
                .suffix(" st")
                .text("Tune"),
        );
        ui.add(
            Slider::new(&mut p.decay, 0.3..=3.0)
                .logarithmic(true)
                .show_value(false)
                .text("Length"),
        );
        ui.add(
            Slider::new(&mut p.punch, 0.0..=1.0)
                .show_value(false)
                .text("Kick punch"),
        );
        ui.add(
            Slider::new(&mut p.snappy, 0.0..=1.0)
                .show_value(false)
                .text("Snare snap"),
        );
        ui.add(
            Slider::new(&mut p.gain, 0.0..=1.2)
                .show_value(false)
                .text("Level"),
        );
    });
    section(ui, "Drum levels", false, |ui| {
        for (i, piece) in DrumPiece::ALL.iter().enumerate() {
            ui.add(
                Slider::new(&mut p.levels[i], 0.0..=1.5)
                    .show_value(false)
                    .text(piece.label()),
            );
        }
    });
}

fn piano_ui(ui: &mut Ui, p: &mut PianoParams) {
    section(ui, "Sound", true, |ui| {
        ui.add(
            Slider::new(&mut p.brightness, 0.0..=1.0)
                .show_value(false)
                .text("Brightness"),
        );
        ui.add(
            Slider::new(&mut p.decay, 0.3..=3.0)
                .logarithmic(true)
                .show_value(false)
                .text("Ring length"),
        );
        ui.add(
            Slider::new(&mut p.hardness, 0.0..=1.0)
                .show_value(false)
                .text("Hammer"),
        );
        ui.add(
            Slider::new(&mut p.detune, 0.0..=1.0)
                .show_value(false)
                .text("Honky-tonk"),
        );
        ui.add(
            Slider::new(&mut p.gain, 0.0..=1.2)
                .show_value(false)
                .text("Level"),
        );
        ui.checkbox(&mut p.pedal, "Sustain pedal")
            .on_hover_text("Let notes ring after they end");
    });
}

fn mix_ui(ui: &mut Ui, t: &mut Track) {
    section(ui, "Mix & effects", true, |ui| {
        ui.add(
            Slider::new(&mut t.volume, 0.0..=1.5)
                .show_value(false)
                .text("Volume"),
        );
        ui.add(
            Slider::new(&mut t.pan, -1.0..=1.0)
                .show_value(false)
                .text("Left ↔ Right"),
        );
        let fx: &mut FxParams = &mut t.fx;
        ui.add(
            Slider::new(&mut fx.reverb, 0.0..=1.0)
                .show_value(false)
                .text("Space (reverb)"),
        );
        ui.add(
            Slider::new(&mut fx.delay, 0.0..=1.0)
                .show_value(false)
                .text("Echo"),
        );
        ui.add(
            Slider::new(&mut fx.drive, 0.0..=1.0)
                .show_value(false)
                .text("Grit (drive)"),
        );
    });
}

fn help(ui: &mut Ui, drums: bool) {
    let lines: &[&str] = &[
        "Click: add a note",
        "Drag a note: move it",
        if drums {
            "Each row is one drum sound"
        } else {
            "Drag the right edge: change length"
        },
        "Right-click a note: delete",
        "Drag on empty space: select",
        "Ctrl/Cmd + C / V / D: copy, paste, duplicate",
        "Arrows: nudge selection · Alt: no snapping",
        if drums {
            "Home-row keys: play each drum live"
        } else {
            "Letter keys: play live · - / =: octave"
        },
    ];
    egui::CollapsingHeader::new(RichText::new("How to").color(theme::TEXT_DIM))
        .default_open(false)
        .show(ui, |ui| {
            for l in lines {
                ui.label(RichText::new(*l).size(11.5).color(theme::TEXT_DIM));
            }
        });
}
