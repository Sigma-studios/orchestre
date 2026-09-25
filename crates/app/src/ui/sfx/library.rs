//! Left panel: the sounds in the user's sound folder, as a tree. Presets
//! appear only when making a new sound, as starting points to copy.

use std::path::PathBuf;

use egui::{RichText, Ui};
use orchestre_core::sfx::Sound;
use orchestre_core::sfx::presets::{PRESETS, Preset, SfxCategory};

use crate::app::OrchestreApp;
use crate::sfx::{FileAction, Preview};
use crate::theme;

pub fn show(app: &mut OrchestreApp, ui: &mut Ui) {
    egui::Panel::left("sfx_library")
        .resizable(true)
        .default_size(230.0)
        .min_size(170.0)
        .frame(
            egui::Frame::new()
                .fill(theme::PANEL)
                .inner_margin(egui::Margin::symmetric(10, 8)),
        )
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| my_sounds(app, ui));
        });
    dialogs(app, ui.ctx());
}

fn title(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .size(11.0)
            .strong()
            .color(theme::TEXT_DIM),
    );
}

fn hint(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(11.5).color(theme::TEXT_DIM));
}

fn new_sound_button(app: &mut OrchestreApp, ui: &mut Ui) {
    if ui.button("+ New sound").clicked() {
        app.sfx.action = Some(FileAction::NewSound(None, String::new()));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn my_sounds(app: &mut OrchestreApp, ui: &mut Ui) {
    title(ui, "MY SOUNDS");
    let Some(lib) = &app.sfx.library else {
        hint(
            ui,
            "Each sound is a .orsfx file. Choose your game's sound folder (e.g. assets/sfx) \
             to see all its sounds here, subfolders included.",
        );
        if ui.button("Choose a folder…").clicked() {
            crate::io::choose_sound_folder(app);
        }
        ui.add_space(4.0);
        new_sound_button(app, ui);
        return;
    };
    // A copy, so the tree can be drawn while clicks change the app.
    let root = lib.root.clone();
    folder(app, ui, &root, true);
    if root.count() == 0 {
        hint(
            ui,
            "No sounds here yet. Click + next to the folder to make one.",
        );
    }
    ui.add_space(8.0);
    if ui.small_button("Change folder…").clicked() {
        crate::io::choose_sound_folder(app);
    }
}

/// A folder's row (with + to add a sound or folder in it), then what's
/// inside.
#[cfg(not(target_arch = "wasm32"))]
fn folder(app: &mut OrchestreApp, ui: &mut Ui, f: &crate::sfx::Folder, root: bool) {
    let id = ui.make_persistent_id(("sfx_folder", &f.path));
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, root)
        .show_header(ui, |ui| {
            let label = ui
                .label(format!("📁 {}", f.name()))
                .on_hover_text(f.path.display().to_string());
            label.context_menu(|ui| folder_menu(app, ui, &f.path));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button("+", |ui| folder_menu(app, ui, &f.path))
                    .response
                    .on_hover_text(format!("New sound or folder in {}", f.name()));
            });
        })
        .body(|ui| {
            for sub in &f.folders {
                folder(app, ui, sub, false);
            }
            for (name, path) in &f.sounds {
                sound_row(app, ui, name, path);
            }
        });
}

/// What can be done in a folder: from its + button or a right-click.
#[cfg(not(target_arch = "wasm32"))]
fn folder_menu(app: &mut OrchestreApp, ui: &mut Ui, path: &std::path::Path) {
    if ui.button("New sound…").clicked() {
        app.sfx.action = Some(FileAction::NewSound(
            Some(path.to_path_buf()),
            String::new(),
        ));
    }
    if ui.button("New folder…").clicked() {
        app.sfx.action = Some(FileAction::NewFolder(path.to_path_buf(), String::new()));
    }
    ui.separator();
    if ui.button(reveal_label()).clicked() {
        crate::io::reveal(app, path);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn sound_row(app: &mut OrchestreApp, ui: &mut Ui, name: &str, path: &PathBuf) {
    let current = app.sfx.path.as_ref() == Some(path);
    // The open sound shows its name as it's being typed, and • when it
    // has unsaved changes.
    let name = match app.sfx.drafts.get(path) {
        _ if current => app.sfx.sound.name.clone(),
        Some(draft) => draft.sound.name.clone(),
        None => name.to_string(),
    };
    let label = if app.sfx.unsaved(path) {
        format!("{name} •")
    } else {
        name.clone()
    };
    let file = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let resp = ui.selectable_label(current, label).on_hover_text(file);
    if resp.hovered() {
        app.sfx.preview.hovered = Some(Preview::File(path.clone()));
    }
    if resp.clicked() && !current {
        crate::io::open_sound_file(app, path.clone());
    }
    resp.context_menu(|ui| {
        if ui.button("Duplicate").clicked() {
            crate::io::duplicate_sound(app, path.clone());
        }
        if ui.button("Rename…").clicked() {
            app.sfx.action = Some(FileAction::Rename(path.clone(), name.clone()));
        }
        if ui.button("Delete…").clicked() {
            app.sfx.action = Some(FileAction::Delete(path.clone()));
        }
        ui.separator();
        if ui.button(reveal_label()).clicked() {
            crate::io::reveal(app, path);
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn reveal_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "Show in Finder"
    } else if cfg!(target_os = "windows") {
        "Show in Explorer"
    } else {
        "Show in file browser"
    }
}

#[cfg(target_arch = "wasm32")]
fn my_sounds(app: &mut OrchestreApp, ui: &mut Ui) {
    title(ui, "MY SOUNDS");
    hint(
        ui,
        "Save and open .orsfx files with the File menu. The desktop app can also show a whole \
         folder of sounds here.",
    );
    ui.add_space(4.0);
    new_sound_button(app, ui);
}

fn dialogs(app: &mut OrchestreApp, ctx: &egui::Context) {
    let Some(action) = app.sfx.action.take() else {
        return;
    };
    app.sfx.action = match action {
        FileAction::NewSound(dir, text) => new_sound(app, ctx, dir, text),
        #[cfg(not(target_arch = "wasm32"))]
        FileAction::NewFolder(parent, mut name) => {
            match text_prompt(ctx, "New folder", "Create", &mut name) {
                Some(true) => {
                    crate::io::create_folder(app, parent, &name);
                    None
                }
                Some(false) => None,
                None => Some(FileAction::NewFolder(parent, name)),
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        FileAction::Rename(path, mut name) => {
            match text_prompt(ctx, "Rename sound", "Rename", &mut name) {
                Some(true) => {
                    crate::io::rename_sound(app, path, &name);
                    None
                }
                Some(false) => None,
                None => Some(FileAction::Rename(path, name)),
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        FileAction::Delete(path) => match confirm_delete(ctx, &path) {
            Some(true) => {
                crate::io::delete_sound(app, path);
                None
            }
            Some(false) => None,
            None => Some(FileAction::Delete(path)),
        },
        #[cfg(target_arch = "wasm32")]
        _ => None,
    };
}

/// Pick what a new sound starts from: nothing, or a copy of a preset. One
/// field both names a sound made from scratch and searches the presets.
/// Returns the action to keep while the dialog stays open.
fn new_sound(
    app: &mut OrchestreApp,
    ctx: &egui::Context,
    dir: Option<PathBuf>,
    mut text: String,
) -> Option<FileAction> {
    let mut chosen: Option<Option<Sound>> = None;
    let resp = egui::Modal::new(egui::Id::new("newsound")).show(ctx, |ui| {
        ui.set_width(380.0);
        ui.heading("New sound");
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(dir) = &dir {
            let place = app
                .sfx
                .library
                .as_ref()
                .and_then(|lib| {
                    let rel = dir.strip_prefix(lib.dir().parent()?).ok()?;
                    Some(rel.display().to_string())
                })
                .unwrap_or_else(|| dir.display().to_string());
            hint(ui, &format!("Saved in 📁 {place}"));
        }
        ui.add_space(8.0);
        let field = ui.add(
            egui::TextEdit::singleline(&mut text)
                .hint_text("🔍 Name it, or search the presets")
                .desired_width(f32::INFINITY),
        );
        field.request_focus();
        let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let name = text.trim();
        ui.add_space(6.0);
        let scratch = match name {
            "" => "Start from scratch".to_string(),
            n => format!("Start “{n}” from scratch"),
        };
        let clicked = ui
            .add(egui::Button::new(scratch).min_size(egui::vec2(ui.available_width(), 28.0)))
            .on_hover_text("An empty sound: add layers at the bottom (or press Enter)")
            .clicked();
        if clicked || enter {
            let mut sound = Sound::default();
            if !name.is_empty() {
                sound.name = name.to_string();
            }
            chosen = Some(Some(sound));
        }
        ui.add_space(8.0);
        let found: Vec<&Preset> = PRESETS.iter().filter(|p| matches(p, name)).collect();
        match (name.is_empty(), found.len()) {
            (true, _) => {
                title(ui, "OR START FROM A PRESET");
                hint(
                    ui,
                    "Rest on one to hear it. You get a copy to change as you like.",
                );
            }
            (false, 0) => hint(ui, "No preset matches. Try another word."),
            (false, 1) => title(ui, "OR START FROM THIS PRESET"),
            (false, n) => title(ui, &format!("OR START FROM ONE OF {n} PRESETS")),
        }
        egui::ScrollArea::vertical()
            .max_height(360.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                for cat in SfxCategory::ALL {
                    let presets: Vec<_> = found.iter().filter(|p| p.category == cat).collect();
                    if presets.is_empty() {
                        continue;
                    }
                    // While searching, every group with a match is open; the
                    // groups opened by hand are kept for when the search ends.
                    let searching = !name.is_empty();
                    egui::CollapsingHeader::new(cat.label())
                        .id_salt(("newsound", cat.label(), searching))
                        .default_open(searching)
                        .open(searching.then_some(true))
                        .show(ui, |ui| {
                            for p in presets {
                                let resp = ui
                                    .selectable_label(false, p.name)
                                    .on_hover_text(p.description);
                                if resp.hovered() {
                                    app.sfx.preview.hovered = Some(Preview::Preset(p.name));
                                }
                                if resp.clicked() {
                                    chosen = Some(Some(p.sound()));
                                }
                            }
                        });
                }
            });
        ui.add_space(8.0);
        if ui.button("Cancel").clicked() {
            chosen = Some(None);
        }
    });
    if resp.should_close() && chosen.is_none() {
        chosen = Some(None);
    }
    match chosen {
        None => Some(FileAction::NewSound(dir, text)),
        Some(None) => None,
        Some(Some(sound)) => {
            crate::io::create_sound(app, dir, sound);
            None
        }
    }
}

/// Whether a preset fits a search: every word appears in its name,
/// description or group ("metal door" finds "Heavy metal door").
fn matches(p: &Preset, query: &str) -> bool {
    let haystack = format!("{} {} {}", p.name, p.description, p.category.label()).to_lowercase();
    query
        .to_lowercase()
        .split_whitespace()
        .all(|word| haystack.contains(word))
}

/// A one-line text dialog: `Some(true)` when confirmed, `Some(false)` when
/// cancelled, `None` while still open.
#[cfg(not(target_arch = "wasm32"))]
fn text_prompt(ctx: &egui::Context, heading: &str, ok: &str, text: &mut String) -> Option<bool> {
    let mut done = None;
    egui::Modal::new(egui::Id::new(("sfx_prompt", heading))).show(ctx, |ui| {
        ui.heading(heading);
        let r = ui.text_edit_singleline(text);
        r.request_focus();
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            done = Some(true);
        }
        ui.horizontal(|ui| {
            if ui.button(ok).clicked() {
                done = Some(true);
            }
            if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                done = Some(false);
            }
        });
    });
    done
}

#[cfg(not(target_arch = "wasm32"))]
fn confirm_delete(ctx: &egui::Context, path: &std::path::Path) -> Option<bool> {
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let mut choice = None;
    let resp = egui::Modal::new(egui::Id::new("sfx_delete")).show(ctx, |ui| {
        ui.set_max_width(420.0);
        ui.heading(format!("Delete “{name}”?"));
        ui.label("The file will be deleted from disk. This can't be undone.");
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let del = egui::Button::new(RichText::new("Delete").color(egui::Color32::WHITE))
                .fill(egui::Color32::from_rgb(150, 60, 50));
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
    choice
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_needs_every_word() {
        let laser = PRESETS.iter().find(|p| p.name == "Laser").unwrap();
        assert!(matches(laser, ""));
        assert!(matches(laser, "LASER"));
        assert!(matches(laser, "  las  "));
        assert!(!matches(laser, "laser zzzz"));
        let found = PRESETS.iter().filter(|p| matches(p, "step")).count();
        assert!(found > 1, "finds several footsteps");
    }
}
