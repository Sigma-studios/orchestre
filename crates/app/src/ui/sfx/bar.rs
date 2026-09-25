use egui::{Color32, RichText, Ui};

use crate::app::OrchestreApp;
use crate::theme;

pub fn show(app: &mut OrchestreApp, ui: &mut Ui) {
    egui::Panel::top("sfx_bar")
        .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(egui::Margin::symmetric(12, 8)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("♫").strong().size(20.0).color(theme::ACCENT))
                    .on_hover_text("Orchestre");
                ui.add_space(4.0);
                file_menu(app, ui);
                ui.separator();
                super::mode_switch(app, ui);
                ui.separator();

                let looping = app.sfx.looping.is_some();
                let (label, fill) = if looping {
                    ("⏹ Stop", Color32::from_rgb(120, 60, 50))
                } else {
                    ("▶ Play", Color32::from_rgb(50, 110, 70))
                };
                let play = egui::Button::new(RichText::new(label).size(15.0))
                    .fill(fill)
                    .min_size(egui::vec2(84.0, 28.0));
                let empty = app.sfx.sound.layers.is_empty();
                let hover = if app.sfx.sound.looping == orchestre_core::sfx::Looping::Once {
                    "Play the sound (Space). Each play varies a little.\nKeys 1 to 9 play it from soft to hard."
                } else {
                    "This sound loops: Space starts and stops it.\nWhile it plays, keys 1 to 9 and the intensity slider change it live."
                };
                if ui
                    .add_enabled(!empty || looping, play)
                    .on_hover_text(hover)
                    .on_disabled_hover_text("Add a layer first")
                    .clicked()
                {
                    app.play_sound();
                }
                ui.label("Intensity");
                if ui
                    .add(egui::Slider::new(&mut app.sfx.intensity, 0.1..=2.0).show_value(false))
                    .on_hover_text(
                        "How hard the sound is played. The middle is as designed; \
                         lower is softer, darker and shorter, higher is louder and brighter.\n\
                         Games choose this on each play, and can change it while a sound plays.",
                    )
                    .changed()
                {
                    app.sound_live_changed();
                }
                ui.toggle_value(&mut app.sfx.lock_seed, "🔒 Same every time")
                    .on_hover_text(
                        "Play the same variation each time, to compare your edits.\n\
                         Off: every play varies, as it will in the game.",
                    );
                if app.sfx.lock_seed
                    && ui
                        .button("🎲")
                        .on_hover_text("Pick another variation and play it")
                        .clicked()
                {
                    app.sfx.reroll();
                    app.play_sound();
                }

                ui.separator();
                let name = app.sfx.sound.name.clone();
                let dirty = app.sfx.dirty();
                let label = if dirty { format!("{name} •") } else { name };
                let hover = match (&app.sfx.path, dirty) {
                    (Some(p), true) => format!("{}\nUnsaved changes (Ctrl/Cmd+S saves)", p.display()),
                    (Some(p), false) => p.display().to_string(),
                    (None, _) => "Not saved to a file yet".to_string(),
                };
                ui.add(egui::Label::new(RichText::new(label).color(theme::TEXT_DIM)).truncate())
                    .on_hover_text(hover);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let master = app.levels.get(&0).copied().unwrap_or(0.0);
                    crate::ui::transport::meter(ui, master, egui::vec2(60.0, 10.0));
                    if let Some(err) = app.audio.error() {
                        ui.label(RichText::new("⚠ No audio").color(Color32::from_rgb(240, 120, 100)))
                            .on_hover_text(err);
                    }
                    if ui
                        .add_enabled(app.sfx.history.can_redo(), egui::Button::new("↪"))
                        .on_hover_text("Redo (Ctrl/Cmd+Shift+Z)")
                        .clicked()
                    {
                        app.redo_sound();
                    }
                    if ui
                        .add_enabled(app.sfx.history.can_undo() || app.sfx.changed, egui::Button::new("↩"))
                        .on_hover_text("Undo (Ctrl/Cmd+Z)")
                        .clicked()
                    {
                        app.undo_sound();
                    }
                });
            });
        });
}

fn file_menu(app: &mut OrchestreApp, ui: &mut Ui) {
    ui.menu_button("File", |ui| {
        if ui.button("New sound").clicked() {
            crate::io::new_sound(app);
        }
        if ui.button("Open…").clicked() {
            crate::io::open_sound(app);
        }
        if ui.button("Save").clicked() {
            crate::io::save_sound(app);
        }
        if ui.button("Save as…").clicked() {
            crate::io::save_sound_as(app);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let unsaved = app.sfx.unsaved_names().len();
            let all = ui.add_enabled(unsaved > 0, egui::Button::new("Save all"));
            if all
                .on_hover_text(format!("Save the {unsaved} sounds with unsaved changes"))
                .clicked()
            {
                crate::io::save_all_sounds(app);
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            ui.separator();
            if ui
                .button("Choose sound folder…")
                .on_hover_text(
                    "Show every sound in a folder (e.g. your game's sfx folder) on the left",
                )
                .clicked()
            {
                crate::io::choose_sound_folder(app);
            }
        }
        ui.separator();
        if ui.button("Export WAV…").clicked() {
            crate::io::export_sound_wav(app);
        }
        if ui
            .button(format!("Export {} variations…", crate::io::VARIATIONS))
            .on_hover_text(
                "One WAV file per variation (name_01.wav, name_02.wav…), for engines that pick \
                 a random file on each play",
            )
            .clicked()
        {
            crate::io::export_sound_variations(app);
        }
        ui.separator();
        if ui.button("Settings…").clicked() {
            app.settings_open = true;
        }
    });
}
