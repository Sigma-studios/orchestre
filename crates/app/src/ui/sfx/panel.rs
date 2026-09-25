//! The middle of the sound editor: an overview of the whole sound, the
//! selected layer's settings (or the sound's) and its generator's knobs.

use egui::{Align2, FontId, Rect, RichText, Sense, Slider, Ui, pos2, vec2};
use orchestre_core::Wave;
use orchestre_core::sfx::{
    BubbleParams, Curve, EngineParams, FilterMode, Generator, GeneratorKind, Jitter, Layer,
    Looping, MAX_HITS, Material, ModalParams, NoiseParams, PulseParams, Shape, ThumpParams,
    ToneParams, VoiceParams, vowel_label,
};

use super::layers::{draw_grid, draw_playheads, draw_wave, navigate};
use crate::app::OrchestreApp;
use crate::theme;
use crate::ui::timeline::HEADER_W;

pub fn show(app: &mut OrchestreApp, ui: &mut Ui) {
    let full = ui.max_rect();
    let side = Rect::from_min_max(full.min, pos2(full.left() + HEADER_W, full.bottom()));
    let right = Rect::from_min_max(pos2(side.right(), full.top()), full.max);
    let over_h = (right.height() * 0.4).clamp(90.0, 220.0);
    let over = Rect::from_min_size(right.min, vec2(right.width(), over_h));
    let knobs = Rect::from_min_max(pos2(right.left(), over.bottom()), right.max);

    ui.painter().rect_filled(side, 0.0, theme::PANEL);
    let mut side_ui = ui.new_child(egui::UiBuilder::new().max_rect(side.shrink2(vec2(10.0, 0.0))));
    side_ui.spacing_mut().slider_width = 110.0;
    egui::ScrollArea::vertical()
        .id_salt("sfx_side")
        .auto_shrink(false)
        .show(&mut side_ui, |ui| {
            ui.add_space(8.0);
            if app.sfx.selected.is_some() && !app.sfx.sound.controls.is_empty() {
                controls_section(app, ui);
                ui.separator();
            }
            match app.sfx.selected {
                Some(id) => layer_side(app, ui, id),
                None => sound_side(app, ui),
            }
            ui.add_space(12.0);
        });

    overview(app, ui, over);

    ui.painter().rect_filled(knobs, 0.0, theme::BG);
    let mut kui = ui.new_child(egui::UiBuilder::new().max_rect(knobs.shrink2(vec2(16.0, 10.0))));
    kui.spacing_mut().slider_width = 150.0;
    egui::ScrollArea::vertical()
        .id_salt("sfx_knobs")
        .auto_shrink(false)
        .show(&mut kui, |ui| match app.sfx.selected {
            Some(id) => generator_knobs(app, ui, id),
            None => tips(ui),
        });
    crate::ui::editor::toast(app, ui, right);
}

/// Every layer's waveform over time, in its color. Click to play.
fn overview(app: &mut OrchestreApp, ui: &mut Ui, rect: Rect) {
    let resp = ui.interact(rect, ui.id().with("sfx_overview"), Sense::click());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, theme::GRID_BG);
    draw_grid(&p, rect, &app.sfx.view);
    let sound = app.sfx.sound.clone();
    let body = rect.shrink2(vec2(0.0, 14.0));
    for layer in &sound.layers {
        let audible = sound.audible(layer);
        let alpha = match app.sfx.selected {
            Some(id) if id == layer.id => 0.95,
            Some(_) => 0.3,
            None => 0.7,
        } * if audible { 1.0 } else { 0.3 };
        let env = app.sfx.waves.get(&sound, layer).to_vec();
        let color = theme::track_color(layer.color).gamma_multiply(alpha);
        draw_wave(
            &p,
            rect.left(),
            body,
            &env,
            &app.sfx.view,
            layer.start,
            color,
        );
    }
    draw_playheads(app, &p, rect);
    let info = if sound.layers.is_empty() {
        "No layers yet".to_string()
    } else {
        let n = sound.layers.len();
        let plays = match sound.looping {
            orchestre_core::sfx::Looping::Once => String::new(),
            orchestre_core::sfx::Looping::Sustain => " · holds until stopped".into(),
            orchestre_core::sfx::Looping::Repeat { every } => {
                format!(" · repeats every {}", super::format_secs(every))
            }
        };
        format!(
            "{} · {n} layer{}{plays}",
            super::format_secs(sound.length()),
            if n == 1 { "" } else { "s" }
        )
    };
    p.text(
        rect.left_top() + vec2(10.0, 8.0),
        Align2::LEFT_TOP,
        info,
        FontId::proportional(12.0),
        theme::TEXT_DIM,
    );
    navigate(ui, rect, &mut app.sfx.view);
    if resp.clicked() {
        app.play_sound();
    }
    resp.on_hover_text("Click to play (Space)");
}

fn section(ui: &mut Ui, title: &str, open: bool, body: impl FnOnce(&mut Ui)) {
    egui::CollapsingHeader::new(RichText::new(title).strong())
        .default_open(open)
        .show(ui, body);
}

fn hint(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(11.5).color(theme::TEXT_DIM));
}

fn amount(ui: &mut Ui, v: &mut f32, max: f32, label: &str) -> egui::Response {
    ui.add(Slider::new(v, 0.0..=max).show_value(false).text(label))
}

fn hz(ui: &mut Ui, v: &mut f32, lo: f32, hi: f32, label: &str) -> egui::Response {
    ui.add(
        Slider::new(v, lo..=hi)
            .logarithmic(true)
            .suffix(" Hz")
            .fixed_decimals(0)
            .text(label),
    )
}

fn secs(ui: &mut Ui, v: &mut f32, max: f32, label: &str) -> egui::Response {
    ui.add(
        Slider::new(v, 0.0..=max)
            .logarithmic(true)
            .smallest_positive(0.0005)
            .suffix(" s")
            .max_decimals(3)
            .text(label),
    )
}

fn semis(ui: &mut Ui, v: &mut f32, lo: f32, hi: f32, label: &str) -> egui::Response {
    ui.add(
        Slider::new(v, lo..=hi)
            .suffix(" st")
            .max_decimals(1)
            .text(label),
    )
}

fn sound_side(app: &mut OrchestreApp, ui: &mut Ui) {
    let mut name = app.sfx.sound.name.clone();
    ui.add(
        egui::TextEdit::singleline(&mut name)
            .desired_width(HEADER_W - 30.0)
            .font(egui::TextStyle::Heading),
    );
    if name != app.sfx.sound.name {
        app.sfx.sound.name = name;
        app.sfx.touch();
    }
    ui.add_space(4.0);
    controls_section(app, ui);
    let s = &mut app.sfx.sound;
    let before = (
        s.name.clone(),
        s.volume,
        s.space,
        s.variation,
        s.max_voices,
        s.looping,
    );
    section(ui, "Sound", true, |ui| {
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("sfx_looping")
                .width(150.0)
                .selected_text(s.looping.label())
                .show_ui(ui, |ui| {
                    for (l, tip) in [
                        (Looping::Once, "Plays once, then ends"),
                        (Looping::Sustain, "Layers hold after fading in, until the game stops the sound: wind, fire, engines"),
                        (Looping::Repeat { every: 0.5 }, "The whole sound starts again regularly, varying each time: heartbeats, alarms, machine guns"),
                    ] {
                        let on = std::mem::discriminant(&l) == std::mem::discriminant(&s.looping);
                        if ui.selectable_label(on, l.label()).on_hover_text(tip).clicked() && !on {
                            s.looping = l;
                        }
                    }
                });
            ui.label("Plays");
        });
        if let Looping::Repeat { every } = &mut s.looping {
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(every)
                        .range(0.02..=10.0)
                        .speed(0.005)
                        .max_decimals(3)
                        .suffix(" s"),
                );
                ui.label("between repeats");
            });
        }
        amount(ui, &mut s.volume, 1.5, "Volume");
        amount(ui, &mut s.space, 1.0, "Space (reverb)")
            .on_hover_text("Room around the sound: dry ← → a big hall");
        ui.add(
            Slider::new(&mut s.variation, 0.0..=2.0)
                .show_value(false)
                .text("Variation"),
        )
        .on_hover_text(
            "How different each play is. 0 plays the same sound every time; \
                 each layer sets what varies (select one to see).",
        );
        ui.horizontal(|ui| {
            let mut n = s.max_voices as i32;
            if ui.add(egui::DragValue::new(&mut n).range(1..=16)).changed() {
                s.max_voices = n as u8;
            }
            ui.label("copies at once");
        })
        .response
        .on_hover_text(
            "In a game, playing this sound more often than this at the same time \
             stops the oldest copy (so a burst of footsteps doesn't pile up).",
        );
    });
    let after = (
        s.name.clone(),
        s.volume,
        s.space,
        s.variation,
        s.max_voices,
        s.looping,
    );
    if before != after {
        app.sfx.touch();
    }
    ui.add_space(6.0);
    hint(ui, "Click a layer below to shape it.");
}

/// The sound's named controls: try them (live, even while it plays), and
/// define them. Shown first, whatever is selected.
fn controls_section(app: &mut OrchestreApp, ui: &mut Ui) {
    let has = !app.sfx.sound.controls.is_empty();
    // Remembered per sound, open at first when it has controls.
    let id = ("sfx_controls", app.sfx.sound.name.clone(), has);
    egui::CollapsingHeader::new(RichText::new("Controls").strong())
        .id_salt(id)
        .default_open(has)
        .show(ui, |ui| {
            hint(
                ui,
                "Settings a game can change on this sound, even while it plays: an engine's rpm, \
             a gun's rate of fire, a voice's pitch.",
            );
            let mut live = false;
            for c in app.sfx.sound.controls.clone() {
                let v = app.sfx.controls.entry(c.name.clone()).or_insert(c.base);
                let (lo, hi) = (c.min.min(c.max), c.max.max(c.min + 1e-3));
                let log = lo > 0.0 && hi / lo > 4.0;
                let r = ui.add(
                    Slider::new(v, lo..=hi)
                        .logarithmic(log)
                        .max_decimals(if hi - lo > 50.0 { 0 } else { 2 })
                        .suffix(if c.unit.is_empty() {
                            String::new()
                        } else {
                            format!(" {}", c.unit)
                        })
                        .text(&c.name),
                );
                live |= r.changed();
                if r.double_clicked() {
                    *v = c.base;
                    live = true;
                }
            }
            if live {
                app.sound_live_changed();
            }
            egui::CollapsingHeader::new(RichText::new("Edit controls").color(theme::TEXT_DIM))
                .default_open(false)
                .show(ui, |ui| edit_controls(app, ui));
        });
}

fn edit_controls(app: &mut OrchestreApp, ui: &mut Ui) {
    let mut controls = app.sfx.sound.controls.clone();
    let mut remove = None;
    for (i, c) in controls.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut c.name)
                        .desired_width(110.0)
                        .hint_text("Name"),
                );
                egui::ComboBox::from_id_salt("target")
                    .width(80.0)
                    .selected_text(c.target.label())
                    .show_ui(ui, |ui| {
                        for t in orchestre_core::sfx::ControlTarget::ALL {
                            ui.selectable_value(&mut c.target, t, t.label());
                        }
                    });
                if ui
                    .small_button("🗑")
                    .on_hover_text("Remove this control")
                    .clicked()
                {
                    remove = Some(i);
                }
            });
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut c.unit)
                        .desired_width(40.0)
                        .hint_text("unit"),
                );
                ui.add(
                    egui::DragValue::new(&mut c.base)
                        .speed(0.5)
                        .prefix("as designed at "),
                );
            });
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut c.min).speed(0.5).prefix("from "));
                ui.add(egui::DragValue::new(&mut c.max).speed(0.5).prefix("to "));
            });
            ui.add_space(4.0);
        });
    }
    if let Some(i) = remove {
        controls.remove(i);
    }
    if ui.button("+ Add control").clicked() {
        controls.push(orchestre_core::sfx::Control::new(
            "Speed",
            orchestre_core::sfx::ControlTarget::Rate,
            "×",
            1.0,
            0.25,
            4.0,
        ));
    }
    hint(
        ui,
        "At its \"as designed\" value the sound plays as made; twice that value doubles its \
         pitch, speed or intensity. Games set controls by name.",
    );
    if controls != app.sfx.sound.controls {
        app.sfx.sound.controls = controls;
        app.sfx.touch();
    }
}

fn layer_side(app: &mut OrchestreApp, ui: &mut Ui, id: orchestre_core::Id) {
    let Some(orig) = app.sfx.sound.layer(id).cloned() else {
        app.sfx.selected = None;
        return;
    };
    let mut l = orig.clone();
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
        ui.painter()
            .circle_filled(r.center(), 6.0, theme::track_color(l.color));
        ui.add(
            egui::TextEdit::singleline(&mut l.name)
                .desired_width(HEADER_W - 90.0)
                .font(egui::TextStyle::Heading),
        );
        if ui
            .button("×")
            .on_hover_text("Back to the sound's settings (Esc)")
            .clicked()
        {
            app.sfx.selected = None;
        }
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("Made of");
        let current = l.generator.kind();
        egui::ComboBox::from_id_salt("sfx_kind")
            .width(120.0)
            .selected_text(current.label())
            .show_ui(ui, |ui| {
                for kind in GeneratorKind::ALL {
                    if ui
                        .selectable_label(kind == current, kind.label())
                        .on_hover_text(kind.description())
                        .clicked()
                        && kind != current
                    {
                        l.generator = kind.default_generator();
                    }
                }
            })
            .response
            .on_hover_text("What makes this layer's sound. Changing it resets its settings.");
    });
    ui.separator();
    section(ui, "Mix", true, |ui| {
        amount(ui, &mut l.gain, 1.5, "Volume");
        ui.add(
            Slider::new(&mut l.pan, -1.0..=1.0)
                .show_value(false)
                .text("Left ↔ Right"),
        );
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut l.start)
                    .range(0.0..=10.0)
                    .speed(0.002)
                    .max_decimals(3)
                    .suffix(" s"),
            );
            ui.label("starts at");
        })
        .response
        .on_hover_text("When this layer starts. You can also drag it along the timeline below.");
        amount(ui, &mut l.drive, 1.0, "Grit (drive)");
        semis(ui, &mut l.follow, -12.0, 12.0, "Pitch follows intensity").on_hover_text(
            "How much this layer's pitch rises when played harder, even while it plays: \
             an engine revving. 0 keeps the pitch.",
        );
    });
    section(ui, "Variation", true, |ui| {
        hint(
            ui,
            "Each play changes this layer at random, within these ranges.",
        );
        let j = &mut l.jitter;
        semis(ui, &mut j.pitch, 0.0, 12.0, "Pitch ±");
        ui.add(
            Slider::new(&mut j.volume, 0.0..=12.0)
                .suffix(" dB")
                .max_decimals(1)
                .text("Volume ±"),
        );
        ui.add(
            Slider::new(&mut j.timing, 0.0..=0.2)
                .suffix(" s")
                .max_decimals(3)
                .text("Timing ±"),
        )
        .on_hover_text("Moves the layer's start earlier or later");
        ui.add(
            Slider::new(&mut j.tone, 0.0..=1.0)
                .show_value(false)
                .text("Brightness ±"),
        );
        ui.add(
            Slider::new(&mut j.length, 0.0..=0.5)
                .show_value(false)
                .text("Length ±"),
        );
        ui.horizontal(|ui| {
            if ui.button("None").on_hover_text("Always the same").clicked() {
                *j = Jitter::NONE;
            }
            if ui.button("Subtle").clicked() {
                *j = Jitter::SUBTLE;
            }
            if ui.button("Strong").clicked() {
                *j = Jitter {
                    pitch: 2.0,
                    volume: 4.0,
                    timing: 0.01,
                    tone: 0.4,
                    length: 0.25,
                };
            }
        });
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui
            .button("⧉ Duplicate")
            .on_hover_text("Ctrl/Cmd+D")
            .clicked()
        {
            app.sfx.duplicate_layer(id);
        }
        if ui
            .button(RichText::new("🗑 Delete").color(egui::Color32::from_rgb(240, 120, 100)))
            .on_hover_text("Delete or Backspace")
            .clicked()
        {
            app.sfx.delete_layer(id);
        }
    });
    if l != orig
        && app.sfx.selected == Some(id)
        && let Some(slot) = app.sfx.sound.layer_mut(id)
    {
        *slot = l;
        app.sfx.touch();
    }
}

fn generator_knobs(app: &mut OrchestreApp, ui: &mut Ui, id: orchestre_core::Id) {
    let Some(Layer { generator, .. }) = app.sfx.sound.layer(id).cloned() else {
        return;
    };
    let mut g = generator;
    ui.label(RichText::new(g.kind().description()).color(theme::TEXT_DIM));
    ui.add_space(4.0);
    ui.columns(2, |cols| {
        let (a, b) = cols.split_at_mut(1);
        let (left, right) = (&mut a[0], &mut b[0]);
        match &mut g {
            Generator::Noise(p) => noise_knobs(left, right, p),
            Generator::Tone(p) => tone_knobs(left, right, p),
            Generator::Modal(p) => modal_knobs(left, right, p),
            Generator::Thump(p) => thump_knobs(left, right, p),
            Generator::Voice(p) => voice_knobs(left, right, p),
            Generator::Pulses(p) => pulse_knobs(left, right, p),
            Generator::Bubbles(p) => bubble_knobs(left, right, p),
            Generator::Engine(p) => engine_knobs(left, right, p),
        }
    });
    if g != generator {
        app.sfx.set_generator(id, g);
    }
}

fn heading(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).strong());
}

fn shape_knobs(ui: &mut Ui, s: &mut Shape) {
    heading(ui, "Volume over time");
    secs(ui, &mut s.attack, 2.0, "Fade in");
    secs(ui, &mut s.hold, 3.0, "Hold");
    secs(ui, &mut s.decay, 6.0, "Fade out").on_hover_text("Time to fade to silence after the hold");
}

fn noise_knobs(ui: &mut Ui, other: &mut Ui, p: &mut NoiseParams) {
    heading(ui, "Noise");
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("sfx_filter")
            .width(170.0)
            .selected_text(p.filter.label())
            .show_ui(ui, |ui| {
                for f in FilterMode::ALL {
                    ui.selectable_value(&mut p.filter, f, f.label());
                }
            });
    });
    hz(ui, &mut p.cutoff_start, 30.0, 18000.0, "Tone at start")
        .on_hover_text("Filter frequency at the start of the layer");
    hz(ui, &mut p.cutoff_end, 30.0, 18000.0, "Tone at end").on_hover_text(
        "It slides there over the layer's length: up for a rising whoosh, down for a blast",
    );
    amount(ui, &mut p.resonance, 0.95, "Resonance")
        .on_hover_text("A whistling, focused tone at the filter frequency");
    ui.add_space(6.0);
    heading(ui, "Crackle");
    amount(ui, &mut p.crackle, 1.0, "Crackle")
        .on_hover_text("Steady noise ← → random grains: fire, gravel, debris");
    ui.add(
        Slider::new(&mut p.density, 5.0..=3000.0)
            .logarithmic(true)
            .fixed_decimals(0)
            .suffix(" /s")
            .text("Grains"),
    );
    shape_knobs(other, &mut p.shape);
    other.add_space(6.0);
    heading(other, "Drift");
    ui_drift(other, p);
}

fn ui_drift(ui: &mut Ui, p: &mut NoiseParams) {
    ui.add(
        Slider::new(&mut p.wobble, 0.0..=3.0)
            .max_decimals(1)
            .suffix(" oct")
            .text("Drift"),
    )
    .on_hover_text("The tone and level wander at random: gusts of wind, flickering fire");
    ui.add(
        Slider::new(&mut p.wobble_rate, 0.05..=40.0)
            .logarithmic(true)
            .max_decimals(2)
            .suffix(" /s")
            .text("Drift speed"),
    );
}

fn voice_knobs(ui: &mut Ui, other: &mut Ui, p: &mut VoiceParams) {
    let len = p.shape.length();
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("sfx_tract")
            .width(90.0)
            .selected_text(p.tract.label())
            .show_ui(ui, |ui| {
                for t in orchestre_core::sfx::Tract::ALL {
                    ui.selectable_value(&mut p.tract, t, t.label());
                }
            });
        ui.label("Throat");
    })
    .response
    .on_hover_text("Whose vocal tract shapes the voice: a person's vowels, or a dog's muzzle");
    let hz = |v: f32| format!("{v:.0} Hz");
    let pct = |v: f32| format!("{:.0}%", v * 100.0);
    heading(ui, "Pitch and vowel");
    super::curve::edit(
        ui,
        &mut p.pitch,
        &super::curve::Spec {
            label: "Pitch",
            lo: 40.0,
            hi: 3000.0,
            log: true,
            len,
            format: &hz,
        },
    );
    let mouth = |v: f32| {
        let word = if v < 0.33 {
            "closed"
        } else if v < 0.66 {
            "half open"
        } else {
            "open"
        };
        word.to_string()
    };
    super::curve::edit(
        ui,
        &mut p.openness,
        &super::curve::Spec {
            label: "Mouth: closed (oo, ee) ↑ open (ah)",
            lo: 0.0,
            hi: 1.0,
            log: false,
            len,
            format: &mouth,
        },
    );
    let tongue = |v: f32| {
        let word = if v < 0.33 {
            "back"
        } else if v < 0.66 {
            "middle"
        } else {
            "front"
        };
        word.to_string()
    };
    super::curve::edit(
        ui,
        &mut p.frontness,
        &super::curve::Spec {
            label: "Tongue: back (oo, oh) ↑ front (ee, eh)",
            lo: 0.0,
            hi: 1.0,
            log: false,
            len,
            format: &tongue,
        },
    );
    let (o, f) = (p.openness.at(0.5), p.frontness.at(0.5));
    ui.horizontal(|ui| {
        super::curve::vowel_chart(ui, &p.openness, &p.frontness);
        ui.label(
            RichText::new(format!("Middle of the layer: “{}”", vowel_label(o, f)))
                .size(11.5)
                .color(theme::TEXT_DIM),
        );
    });
    ui.add_space(4.0);
    section(ui, "Klatt: formants, hiss, nose", false, |ui| {
        klatt_knobs(ui, p, len)
    });

    heading(other, "Voice over time");
    let quality = |v: f32| {
        let word = if v < 0.3 {
            "pressed"
        } else if v < 0.65 {
            "normal"
        } else {
            "breathy"
        };
        word.to_string()
    };
    super::curve::edit(
        other,
        &mut p.quality,
        &super::curve::Spec {
            label: "Voice: pressed ↓ … breathy ↑",
            lo: 0.0,
            hi: 1.0,
            log: false,
            len,
            format: &quality,
        },
    );
    super::curve::edit(
        other,
        &mut p.loudness,
        &super::curve::Spec {
            label: "Loudness",
            lo: 0.0,
            hi: 1.0,
            log: false,
            len,
            format: &pct,
        },
    );
    other
        .add(
            Slider::new(&mut p.size, 0.25..=4.0)
                .logarithmic(true)
                .max_decimals(2)
                .text("Throat size"),
        )
        .on_hover_text("Small animal or child ← 1 is an adult → big creature");
    other.add_space(4.0);
    heading(other, "Rough edges");
    amount(other, &mut p.roughness, 1.0, "Roughness")
        .on_hover_text("Each vibration a little off in timing and strength: hoarse, unsteady");
    amount(other, &mut p.subharmonics, 1.0, "Growl (subharmonics)")
        .on_hover_text("Every other vibration weaker: a rattle an octave below");
    amount(other, &mut p.split, 1.0, "Split voice")
        .on_hover_text("A second pitch at once: the rasp of screams, crows and roars");
    other.add_enabled_ui(p.split > 0.0, |ui| {
        ui.add(
            Slider::new(&mut p.split_ratio, 0.25..=4.0)
                .logarithmic(true)
                .max_decimals(2)
                .text("Second pitch ×"),
        );
    });
    amount(other, &mut p.rasp, 1.0, "Rasp").on_hover_text(
        "Fast flutter that makes screams and alarms sound urgent (30–150 Hz), \
         or at 20–35 Hz with Trill, a rolled r or a lip trill",
    );
    other.add_enabled_ui(p.rasp > 0.0, |ui| {
        ui.add(
            Slider::new(&mut p.rasp_rate, 20.0..=150.0)
                .max_decimals(0)
                .suffix(" Hz")
                .text("Rasp rate"),
        );
        amount(ui, &mut p.trill, 1.0, "Trill").on_hover_text(
            "Smooth flutter ← → sharp closures: a tongue or lips tapping shut (grrr, brrr)",
        );
    });
    amount(other, &mut p.breath, 1.0, "Breath")
        .on_hover_text("Breath noise, pulsing with the voice");
    semis(other, &mut p.vibrato, 0.0, 6.0, "Wobble");
    other.add(
        Slider::new(&mut p.vibrato_rate, 0.5..=30.0)
            .logarithmic(true)
            .max_decimals(1)
            .suffix(" Hz")
            .text("Wobble speed"),
    );
    other.add_space(4.0);
    shape_knobs(other, &mut p.shape);
}

/// The rest of Klatt's (1980) synthesizer, for sounds that are not vowels:
/// an r (F3 down), a hiss (s, sh, f), a hum (m, n), a whisper.
fn klatt_knobs(ui: &mut Ui, p: &mut VoiceParams, len: f32) {
    hint(
        ui,
        "For what a vowel can't do. An r is a low F3 (about 1600 Hz); s, sh and f are \
         frication through the parallel formants; m and n are the nose.",
    );
    let hz_fmt = |v: f32| format!("{v:.0} Hz");
    let pct = |v: f32| format!("{:.0}%", v * 100.0);
    let (freqs, widths) = orchestre_dsp::sfx::voice_formants(p, 0.5);

    heading(ui, "Formants by hand");
    for i in 0..5 {
        let mut set = p.klatt.formants[i].is_some();
        let label = format!("F{} ({:.0} Hz from the vowel)", i + 1, freqs[i]);
        if ui.checkbox(&mut set, label).changed() {
            p.klatt.formants[i] = set.then(|| Curve::flat(freqs[i].round()));
        }
        if let Some(curve) = &mut p.klatt.formants[i] {
            super::curve::edit(
                ui,
                curve,
                &super::curve::Spec {
                    label: &format!("F{}", i + 1),
                    lo: 50.0,
                    hi: 8000.0,
                    log: true,
                    len,
                    format: &hz_fmt,
                },
            );
        }
        let mut wide = p.klatt.bandwidths[i].is_some();
        let label = format!("B{} ({:.0} Hz from F{})", i + 1, widths[i], i + 1);
        if ui.checkbox(&mut wide, label).changed() {
            p.klatt.bandwidths[i] = wide.then(|| Curve::flat(widths[i].round()));
        }
        if let Some(curve) = &mut p.klatt.bandwidths[i] {
            super::curve::edit(
                ui,
                curve,
                &super::curve::Spec {
                    label: &format!("B{}: sharp ↓ … broad ↑", i + 1),
                    lo: 10.0,
                    hi: 2000.0,
                    log: true,
                    len,
                    format: &hz_fmt,
                },
            );
        }
    }

    heading(ui, "Voice, breath and hiss over time");
    let amounts: [(&mut Curve, &str, &str); 4] = [
        (
            &mut p.klatt.voicing,
            "Voicing (AV)",
            "How much is the vocal folds. 0 leaves breath and hiss: a whisper, an s",
        ),
        (
            &mut p.klatt.aspiration,
            "Aspiration (AH)",
            "Breath through the whole throat: an h, on top of Breath",
        ),
        (
            &mut p.klatt.frication,
            "Frication (AF)",
            "Hiss at a narrowing of the mouth, shaped by the parallel formants below",
        ),
        (
            &mut p.klatt.voice_bar,
            "Voice bar (AVS)",
            "A soft hum at the pitch, under a voiced hiss: v, z",
        ),
    ];
    for (curve, label, tip) in amounts {
        hint(ui, tip);
        super::curve::edit(
            ui,
            curve,
            &super::curve::Spec {
                label,
                lo: 0.0,
                hi: 1.0,
                log: false,
                len,
                format: &pct,
            },
        );
    }

    heading(ui, "Parallel formants (the hiss)");
    let on = p.klatt.frication_on();
    if !on {
        hint(ui, "Raise Frication to hear these.");
    }
    ui.add_enabled_ui(on, |ui| {
        let names = ["A1", "A2", "A3", "A4", "A5", "A6", "AB (bypass)"];
        for (v, name) in p.klatt.parallel.iter_mut().zip(names) {
            amount(ui, v, 1.0, name);
        }
        hint(ui, "s: A6 and AB · sh: A3 and A4 · f: AB alone");
        hz(ui, &mut p.klatt.f6, 2000.0, 12000.0, "F6");
        for (i, w) in p.klatt.parallel_widths.iter_mut().enumerate() {
            hz(ui, w, 20.0, 5000.0, &format!("B{}P", i + 1));
        }
    });

    heading(ui, "Nose");
    let mut nasal = p.klatt.nasal_zero.is_some();
    if ui
        .checkbox(&mut nasal, "Nasal (FNP, FNZ)")
        .on_hover_text("The nose open: an m or n with the mouth shut, a nasal vowel with it open")
        .changed()
    {
        p.klatt.nasal_zero = nasal.then(|| Curve::flat(1000.0));
    }
    if let Some(zero) = &mut p.klatt.nasal_zero {
        hz(
            ui,
            &mut p.klatt.nasal_pole,
            100.0,
            2000.0,
            "Nasal pole (FNP)",
        );
        super::curve::edit(
            ui,
            zero,
            &super::curve::Spec {
                label: "Nasal zero (FNZ)",
                lo: 100.0,
                hi: 4000.0,
                log: true,
                len,
                format: &hz_fmt,
            },
        );
    }
    if ui.button("Reset Klatt").clicked() {
        p.klatt = orchestre_core::sfx::KlattParams::default();
    }
}

fn pulse_knobs(ui: &mut Ui, other: &mut Ui, p: &mut PulseParams) {
    heading(ui, "Pulses");
    ui.add(
        Slider::new(&mut p.rate_start, 0.5..=500.0)
            .logarithmic(true)
            .max_decimals(1)
            .suffix(" /s")
            .text("Rate at start"),
    )
    .on_hover_text(
        "Pulses per second: slow ticks, a creak, an engine's putt-putt, a buzz. \
         For an engine: rpm = pulses per second × 120 / cylinders (four-stroke)",
    );
    ui.label(
        RichText::new(format!(
            "As an engine: {:.0} rpm (4 cylinders), {:.0} rpm (8)",
            p.rate_start * 30.0,
            p.rate_start * 15.0
        ))
        .size(11.5)
        .color(theme::TEXT_DIM),
    );
    ui.add(
        Slider::new(&mut p.rate_end, 0.5..=500.0)
            .logarithmic(true)
            .max_decimals(1)
            .suffix(" /s")
            .text("Rate at end"),
    );
    secs(ui, &mut p.glide, 3.0, "Slide time");
    amount(ui, &mut p.irregular, 1.0, "Irregular")
        .on_hover_text("Steady ← → random timing and strength");
    ui.add_space(6.0);
    heading(ui, "Each pulse");
    secs(ui, &mut p.burst, 0.1, "Burst length")
        .on_hover_text("Short clicks ← → longer, noisier puffs");
    hz(ui, &mut p.tone, 40.0, 12000.0, "Tone");
    amount(ui, &mut p.resonance, 1.0, "Ring").on_hover_text("Dull knock ← → ringing, squeaky");
    amount(ui, &mut p.grit, 1.0, "Grit")
        .on_hover_text("Clean clicks (friction, creaks) ← → bursts of noise");
    amount(ui, &mut p.body, 1.0, "Wooden body")
        .on_hover_text("Broad wooden resonances ring along: creaking doors, old furniture");
    shape_knobs(other, &mut p.shape);
}

fn tone_knobs(ui: &mut Ui, other: &mut Ui, p: &mut ToneParams) {
    heading(ui, "Pitch");
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("sfx_wave")
            .width(100.0)
            .selected_text(p.wave.label())
            .show_ui(ui, |ui| {
                for w in Wave::ALL {
                    ui.selectable_value(&mut p.wave, w, w.label());
                }
            });
        ui.label("Wave");
    });
    hz(ui, &mut p.pitch_start, 20.0, 10000.0, "Start pitch");
    hz(ui, &mut p.pitch_end, 20.0, 10000.0, "End pitch");
    secs(ui, &mut p.glide, 3.0, "Slide time")
        .on_hover_text("Time to slide from start to end pitch");
    semis(ui, &mut p.jump, -24.0, 24.0, "Jump")
        .on_hover_text("A sudden step in pitch, like a coin's second note");
    secs(ui, &mut p.jump_time, 2.0, "Jump after");
    ui.add_space(6.0);
    heading(ui, "Character");
    semis(ui, &mut p.vibrato, 0.0, 12.0, "Wobble");
    ui.add(
        Slider::new(&mut p.vibrato_rate, 0.5..=80.0)
            .logarithmic(true)
            .suffix(" Hz")
            .max_decimals(1)
            .text("Wobble speed"),
    );
    ui.add(
        Slider::new(&mut p.fm, 0.0..=8.0)
            .max_decimals(1)
            .text("Metal (FM)"),
    )
    .on_hover_text("Frequency modulation: buzzy, metallic, electric");
    ui.add(
        Slider::new(&mut p.fm_ratio, 0.25..=8.0)
            .logarithmic(true)
            .max_decimals(2)
            .text("Metal ratio"),
    );
    hz(ui, &mut p.cutoff, 100.0, 18000.0, "Brightness");
    amount(ui, &mut p.crush, 1.0, "Lo-fi").on_hover_text("Fewer bits: a retro console sound");
    shape_knobs(other, &mut p.shape);
}

fn modal_knobs(ui: &mut Ui, other: &mut Ui, p: &mut ModalParams) {
    heading(ui, "Object");
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("sfx_material")
            .width(100.0)
            .selected_text(p.material.label())
            .show_ui(ui, |ui| {
                for m in Material::ALL {
                    if ui.selectable_label(p.material == m, m.label()).clicked() && p.material != m
                    {
                        // Each material rings as long as its internal losses
                        // allow at this pitch.
                        p.material = m;
                        p.decay = m.ring_time(p.pitch);
                    }
                }
            });
        ui.label("Material");
    });
    hz(ui, &mut p.pitch, 40.0, 8000.0, "Pitch").on_hover_text("Small objects ring higher");
    secs(ui, &mut p.decay, 6.0, "Ring length");
    amount(ui, &mut p.damping, 1.0, "Damping").on_hover_text(
        "How much faster the high ringing dies: 0 = all ring as long, 1 = ring time falls with frequency",
    );
    if ui
        .small_button(format!("Ring like {}", p.material.label().to_lowercase()))
        .on_hover_text("Set the ring length from the material's internal losses at this pitch")
        .clicked()
    {
        p.decay = p.material.ring_time(p.pitch);
    }
    amount(ui, &mut p.hardness, 1.0, "Hardness")
        .on_hover_text("Soft mallet ← → hard hit: brighter, with more click");
    let mut modes = p.modes as i32;
    if ui
        .add(Slider::new(&mut modes, 1..=orchestre_core::sfx::MAX_MODES as i32).text("Resonances"))
        .on_hover_text(
            "A few ring like a bell or bar; dozens sound like a blade, a plate or junk metal",
        )
        .changed()
    {
        p.modes = modes as u8;
    }
    amount(ui, &mut p.randomize, 1.0, "Irregular shape").on_hover_text(
        "A bell or bar ← → a shard, a link, a lump of junk: resonances land anywhere",
    );
    heading(other, "Shards");
    let mut hits = p.hits as i32;
    if other
        .add(Slider::new(&mut hits, 0..=MAX_HITS as i32).text("Extra hits"))
        .on_hover_text("Smaller strikes after the first: shards, rattles, debris")
        .changed()
    {
        p.hits = hits as u8;
    }
    other.add_enabled_ui(p.hits > 0, |ui| {
        secs(ui, &mut p.spread, 3.0, "Spread over");
        semis(ui, &mut p.scatter, 0.0, 24.0, "Pitch scatter ±");
    });
}

fn thump_knobs(ui: &mut Ui, other: &mut Ui, p: &mut ThumpParams) {
    heading(ui, "Body");
    hz(ui, &mut p.pitch_start, 30.0, 2000.0, "Start pitch");
    hz(ui, &mut p.pitch_end, 20.0, 800.0, "End pitch");
    secs(ui, &mut p.drop, 0.5, "Drop time").on_hover_text("How fast the pitch falls");
    amount(ui, &mut p.click, 1.0, "Click").on_hover_text("A sharp snap at the very start");
    amount(ui, &mut p.noise, 1.0, "Noise").on_hover_text("Pure boom ← → dull, messy thud");
    hz(ui, &mut p.cutoff, 100.0, 12000.0, "Noise tone");
    amount(ui, &mut p.blast, 1.0, "Blast wave")
        .on_hover_text("A sharp overpressure then a suction at the start: gunshots, explosions");
    shape_knobs(other, &mut p.shape);
}

fn engine_knobs(ui: &mut Ui, other: &mut Ui, p: &mut EngineParams) {
    heading(ui, "Engine");
    ui.add(
        Slider::new(&mut p.rpm, 300.0..=9000.0)
            .logarithmic(true)
            .max_decimals(0)
            .suffix(" rpm")
            .text("Speed"),
    )
    .on_hover_text("Crankshaft speed as designed (a control can change it live)");
    ui.horizontal(|ui| {
        let mut c = p.cylinders as i32;
        if ui.add(egui::DragValue::new(&mut c).range(1..=12)).changed() {
            p.cylinders = c as u8;
        }
        ui.label("cylinders");
        ui.checkbox(&mut p.two_stroke, "Two-stroke")
            .on_hover_text("Fires every turn of the crank instead of every other: buzzier");
    });
    let cycle = p.rpm / 60.0 / if p.two_stroke { 1.0 } else { 2.0 };
    ui.label(
        RichText::new(format!(
            "Fires {:.0} times a second",
            cycle * p.cylinders.max(1) as f32
        ))
        .size(11.5)
        .color(theme::TEXT_DIM),
    );
    amount(ui, &mut p.uneven, 1.0, "Uneven firing")
        .on_hover_text("Even ← → lopey, like a V-twin or a cross-plane V8");
    ui.add(
        Slider::new(&mut p.exhaust, 0.2..=6.0)
            .logarithmic(true)
            .max_decimals(1)
            .suffix(" m")
            .text("Exhaust pipe"),
    )
    .on_hover_text("Its length sets the resonance that colours the engine's note");
    amount(ui, &mut p.load, 1.0, "Load")
        .on_hover_text("Coasting ← → full throttle: louder, brighter, rougher");
    amount(ui, &mut p.roughness, 1.0, "Roughness")
        .on_hover_text("Firing-to-firing variation, and the odd misfire");
    amount(ui, &mut p.mechanics, 1.0, "Mechanics")
        .on_hover_text("Intake hiss and valve ticks, rising with rpm");
    shape_knobs(other, &mut p.shape);
}

fn bubble_knobs(ui: &mut Ui, other: &mut Ui, p: &mut BubbleParams) {
    heading(ui, "Bubbles");
    ui.add(
        Slider::new(&mut p.rate, 1.0..=5000.0)
            .logarithmic(true)
            .max_decimals(0)
            .suffix(" /s")
            .text("Bubbles"),
    )
    .on_hover_text("Up to ~1000/s: drips and light rain; 10000/s: a downpour (van den Doel)");
    ui.add(
        Slider::new(&mut p.size_min, 0.2..=20.0)
            .logarithmic(true)
            .max_decimals(1)
            .suffix(" mm")
            .text("Smallest"),
    );
    ui.add(
        Slider::new(&mut p.size_max, 0.2..=20.0)
            .logarithmic(true)
            .max_decimals(1)
            .suffix(" mm")
            .text("Largest"),
    )
    .on_hover_text("A bubble rings at about 3000 / radius (mm) Hz: 1 mm ≈ 3 kHz, 5 mm ≈ 600 Hz");
    ui.add(
        Slider::new(&mut p.small, 0.0..=8.0)
            .max_decimals(1)
            .text("Mostly small"),
    )
    .on_hover_text("How much more common small bubbles are (~5: rain)");
    ui.add(
        Slider::new(&mut p.rise, 0.0..=10.0)
            .logarithmic(true)
            .smallest_positive(0.01)
            .max_decimals(2)
            .text("Pitch rise"),
    )
    .on_hover_text("Drops in water ~0.1, pouring ~1, blowing through a straw ~10");
    shape_knobs(other, &mut p.shape);
}

fn tips(ui: &mut Ui) {
    ui.label(RichText::new("How sounds work").strong());
    ui.add_space(4.0);
    for line in [
        "A sound is made of layers that play together, each starting at its own time: \
         an explosion can be a boom, a blast of noise and a crackle of debris.",
        "Click a layer below to shape it; drag it to move it in time.",
        "Space plays the sound; keys 1 to 9 play it from soft to hard, like a game would.",
        "Every play varies a little, so repeated sounds don't feel copy-pasted. \
         Turn on “Same every time” to compare your edits.",
        "Save sounds as .orsfx files next to your game's assets: orchestre-bevy plays them \
         directly, and File → Export makes WAV files for other engines.",
    ] {
        ui.label(RichText::new(format!("• {line}")).color(theme::TEXT_DIM));
    }
}
