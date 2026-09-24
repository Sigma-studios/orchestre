use egui::{Color32, RichText};
use orchestre_core::edit::{KeyConflict, apply_key_lock};

use crate::app::OrchestreApp;
use crate::theme;

pub fn show(app: &mut OrchestreApp, ctx: &egui::Context) {
    key_lock(app, ctx);
    delete_track(app, ctx);
    rename(app, ctx);
}

fn key_lock(app: &mut OrchestreApp, ctx: &egui::Context) {
    let Some(prompt) = &app.key_prompt else {
        return;
    };
    let (track, key, n) = (prompt.track, prompt.key, prompt.conflicts);
    let mut choice: Option<Option<KeyConflict>> = None;
    let resp = egui::Modal::new(egui::Id::new("keylock")).show(ctx, |ui| {
        ui.set_max_width(380.0);
        ui.heading(format!("Lock to {}?", key.label()));
        ui.add_space(6.0);
        let s = if n == 1 { "" } else { "s" };
        ui.label(format!(
            "{n} note{s} on this track {} not part of {}.",
            if n == 1 { "is" } else { "are" },
            key.label()
        ));
        ui.label(
            RichText::new(format!(
                "Locking will delete {} — or you can move {} to the nearest note in the key.",
                if n == 1 { "it" } else { "them" },
                if n == 1 { "it" } else { "them" }
            ))
            .color(theme::TEXT_DIM),
        );
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let del = egui::Button::new(
                RichText::new(format!("Delete {n} note{s} & lock")).color(Color32::WHITE),
            )
            .fill(Color32::from_rgb(150, 60, 50));
            if ui.add(del).clicked() {
                choice = Some(Some(KeyConflict::Delete));
            }
            if ui.button("Move to nearest & lock").clicked() {
                choice = Some(Some(KeyConflict::Snap));
            }
            if ui.button("Cancel").clicked() {
                choice = Some(None);
            }
        });
    });
    if resp.should_close() && choice.is_none() {
        choice = Some(None);
    }
    if let Some(c) = choice {
        app.key_prompt = None;
        if let Some(c) = c {
            if let Some(t) = app.project.track_mut(track) {
                apply_key_lock(t, key, c);
            }
            let remaining: std::collections::HashSet<_> = app
                .project
                .track(track)
                .map(|t| t.notes.iter().map(|n| n.id).collect())
                .unwrap_or_default();
            app.selection.retain(|id| remaining.contains(id));
            app.touch();
            app.notify(format!("Locked to {} (Undo to revert)", key.label()));
        }
    }
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
