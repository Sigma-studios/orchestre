use egui::{Color32, RichText};
use orchestre_core::edit::{KeyChange, KeyConflict};

use crate::app::OrchestreApp;
use crate::theme;

pub fn show(app: &mut OrchestreApp, ctx: &egui::Context) {
    key_lock(app, ctx);
    delete_track(app, ctx);
    rename(app, ctx);
    replace_sound(app, ctx);
    quit(app, ctx);
}

/// Quitting with unsaved sounds: save them all, drop them, or stay.
fn quit(app: &mut OrchestreApp, ctx: &egui::Context) {
    if !app.sfx.quitting {
        return;
    }
    let names = app.sfx.unsaved_names();
    let mut choice = None;
    let resp = egui::Modal::new(egui::Id::new("quit")).show(ctx, |ui| {
        ui.set_max_width(420.0);
        let n = names.len();
        ui.heading(match n {
            1 => "Save changes to 1 sound before quitting?".to_string(),
            n => format!("Save changes to {n} sounds before quitting?"),
        });
        for name in names.iter().take(8) {
            ui.label(format!("• {name}"));
        }
        if n > 8 {
            ui.label(RichText::new(format!("and {} more", n - 8)).color(theme::TEXT_DIM));
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let save = egui::Button::new(RichText::new("Save all").color(Color32::WHITE))
                .fill(Color32::from_rgb(50, 110, 70));
            if ui.add(save).clicked() {
                choice = Some(Some(true));
            }
            if ui.button("Don't save").clicked() {
                choice = Some(Some(false));
            }
            if ui.button("Cancel").clicked() {
                choice = Some(None);
            }
        });
    });
    if resp.should_close() && choice.is_none() {
        choice = Some(None);
    }
    let Some(choice) = choice else { return };
    app.sfx.quitting = false;
    let quit = match choice {
        #[cfg(not(target_arch = "wasm32"))]
        Some(true) => crate::io::save_all_sounds(app),
        #[cfg(target_arch = "wasm32")]
        Some(true) => crate::io::save_sound(app),
        Some(false) => {
            // So the sound reopened next time is the saved one.
            app.sfx.discard_all();
            true
        }
        None => false,
    };
    if quit {
        app.sfx.quit_confirmed = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

/// The sound has unsaved changes and another one is being opened.
fn replace_sound(app: &mut OrchestreApp, ctx: &egui::Context) {
    let Some(next) = &app.sfx.replace else {
        return;
    };
    let (name, next_name) = (app.sfx.sound.name.clone(), next.sound.name.clone());
    let mut choice = None;
    let resp = egui::Modal::new(egui::Id::new("replacesound")).show(ctx, |ui| {
        ui.set_max_width(420.0);
        ui.heading(format!("Save changes to “{name}”?"));
        ui.label(format!(
            "You are opening “{next_name}”. Changes you haven't saved will be lost."
        ));
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let save = egui::Button::new(RichText::new("Save").color(Color32::WHITE))
                .fill(Color32::from_rgb(50, 110, 70));
            if ui.add(save).clicked() {
                choice = Some(Some(true));
            }
            if ui.button("Don't save").clicked() {
                choice = Some(Some(false));
            }
            if ui.button("Cancel").clicked() {
                choice = Some(None);
            }
        });
    });
    if resp.should_close() && choice.is_none() {
        choice = Some(None);
    }
    let Some(choice) = choice else { return };
    let next = app.sfx.replace.take();
    let go = match choice {
        Some(true) => crate::io::save_sound(app),
        Some(false) => true,
        None => false,
    };
    if go && let Some(r) = next {
        app.set_sound(r.sound, r.path);
    }
}

fn key_lock(app: &mut OrchestreApp, ctx: &egui::Context) {
    let Some(prompt) = &app.key_prompt else {
        return;
    };
    let (track, key, from, n) = (prompt.track, prompt.key, prompt.from, prompt.conflicts);
    let naming = app.naming();
    let s = if n == 1 { "" } else { "s" };
    let (it, is) = if n == 1 {
        ("it", "is")
    } else {
        ("them", "are")
    };
    // A song that already has a key can move to the new one as a whole.
    let can_transpose = track.is_none() && from.is_some();
    let mut choice: Option<Option<KeyChange>> = None;
    let resp = egui::Modal::new(egui::Id::new("keylock")).show(ctx, |ui| {
        ui.set_max_width(420.0);
        match (track, from) {
            (None, Some(from)) => ui.heading(format!("Change key from {} to {}?", from.label_in(naming), key.label_in(naming))),
            (None, None) => ui.heading(format!("Set the song key to {}?", key.label_in(naming))),
            (Some(_), _) => ui.heading(format!("Lock this track to {}?", key.label_in(naming))),
        };
        ui.add_space(6.0);
        let place = if track.is_some() { "on this track" } else { "in the song" };
        ui.label(format!("{n} note{s} {place} {is} not part of {}.", key.label_in(naming)));
        let hint = if can_transpose {
            "Transposing moves every note into the new key, so melodies keep their shape. Or you can delete or move only the notes that don't fit."
        } else {
            "You can delete them, or move them to the nearest note in the key."
        };
        ui.label(RichText::new(hint).color(theme::TEXT_DIM));
        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            if can_transpose {
                let t = egui::Button::new(RichText::new("Transpose all notes").color(Color32::WHITE))
                    .fill(Color32::from_rgb(50, 110, 70));
                if ui.add(t).clicked() {
                    choice = Some(Some(KeyChange::Transpose));
                }
            }
            let del = egui::Button::new(RichText::new(format!("Delete {n} note{s}")).color(Color32::WHITE))
                .fill(Color32::from_rgb(150, 60, 50));
            if ui.add(del).clicked() {
                choice = Some(Some(KeyChange::Delete));
            }
            if ui.button(format!("Move {it} to nearest")).clicked() {
                choice = Some(Some(KeyChange::Snap));
            }
            if ui.button("Cancel").clicked() {
                choice = Some(None);
            }
        });
    });
    if resp.should_close() && choice.is_none() {
        choice = Some(None);
    }
    let Some(Some(change)) = choice else {
        if choice.is_some() {
            app.key_prompt = None;
        }
        return;
    };
    app.key_prompt = None;
    match track {
        None => app.project.set_key(Some(key), change),
        Some(id) => {
            let conflict = if change == KeyChange::Snap {
                KeyConflict::Snap
            } else {
                KeyConflict::Delete
            };
            app.project.set_follow(id, true, conflict);
        }
    }
    let existing: std::collections::HashSet<_> = app
        .project
        .tracks
        .iter()
        .flat_map(|t| t.notes.iter().map(|n| n.id))
        .collect();
    app.selection.retain(|id| existing.contains(id));
    app.touch();
    app.notify(format!("Now in {} (Undo to revert)", key.label_in(naming)));
}

fn delete_track(app: &mut OrchestreApp, ctx: &egui::Context) {
    let Some(id) = app.confirm_delete_track else {
        return;
    };
    let Some(track) = app.project.track(id) else {
        app.confirm_delete_track = None;
        return;
    };
    let name = track.name.clone();
    let notes = track.notes.len();
    let mut choice = None;
    let resp = egui::Modal::new(egui::Id::new("deltrack")).show(ctx, |ui| {
        ui.heading(format!("Delete “{name}”?"));
        ui.label(format!(
            "The track and its {notes} notes will be removed. You can undo this."
        ));
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let del = egui::Button::new(RichText::new("Delete").color(Color32::WHITE))
                .fill(Color32::from_rgb(150, 60, 50));
            if ui.add(del).clicked() {
                choice = Some(true);
            }
            if ui.button("Cancel").clicked() {
                choice = Some(false);
            }
        });
    });
    if resp.should_close() && choice.is_none() {
        choice = Some(false);
    }
    if let Some(yes) = choice {
        app.confirm_delete_track = None;
        if yes {
            app.project.tracks.retain(|t| t.id != id);
            if app.selected == Some(id) {
                app.select_track(None);
            }
            app.touch();
        }
    }
}

fn rename(app: &mut OrchestreApp, ctx: &egui::Context) {
    let Some((id, mut name)) = app.renaming.take() else {
        return;
    };
    let mut done = None;
    egui::Modal::new(egui::Id::new("rename")).show(ctx, |ui| {
        ui.heading("Rename track");
        let r = ui.text_edit_singleline(&mut name);
        r.request_focus();
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            done = Some(true);
        }
        ui.horizontal(|ui| {
            if ui.button("OK").clicked() {
                done = Some(true);
            }
            if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                done = Some(false);
            }
        });
    });
    match done {
        Some(true) => {
            let name = name.trim().to_string();
            if !name.is_empty()
                && let Some(t) = app.project.track_mut(id)
            {
                t.name = name;
                app.touch();
            }
        }
        Some(false) => {}
        None => app.renaming = Some((id, name)),
    }
}
