//! App preferences (not part of a song), saved between sessions.

use orchestre_core::NoteNaming;
use serde::{Deserialize, Serialize};

use crate::app::OrchestreApp;
use crate::input::KbLayout;

const STORAGE_KEY: &str = "orchestre_settings";
/// Where the keyboard layout was stored before settings existed.
const LEGACY_LAYOUT_KEY: &str = "orchestre_kb_layout";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub note_naming: NoteNaming,
    /// Only used to print key labels; detected from typing unless chosen.
    pub kb_layout: KbLayout,
    pub kb_layout_manual: bool,
}

impl Settings {
    pub fn load(storage: Option<&dyn eframe::Storage>) -> Settings {
        let saved = storage.and_then(|s| s.get_string(STORAGE_KEY));
        if let Some(settings) = saved.and_then(|json| serde_json::from_str(&json).ok()) {
            return settings;
        }
        // First run: follow the system language, and keep an older saved layout.
        let mut settings = Settings::default();
        if system_is_french() {
            settings.note_naming = NoteNaming::French;
            settings.kb_layout = KbLayout::Azerty;
        }
        if let Some(pref) = storage.and_then(|s| s.get_string(LEGACY_LAYOUT_KEY)) {
            let (manual, name) = match pref.strip_prefix("auto:") {
                Some(name) => (false, name),
                None => (true, pref.as_str()),
            };
            if let Some(layout) = KbLayout::from_label(name) {
                settings.kb_layout = layout;
                settings.kb_layout_manual = manual;
            }
        }
        settings
    }

    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        if let Ok(json) = serde_json::to_string(self) {
            storage.set_string(STORAGE_KEY, json);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn system_is_french() -> bool {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .find(|v| !v.is_empty())
        .is_some_and(|v| v.starts_with("fr"))
}

#[cfg(target_arch = "wasm32")]
fn system_is_french() -> bool {
    web_sys::window()
        .and_then(|w| w.navigator().language())
        .is_some_and(|l| l.starts_with("fr"))
}

/// The settings window (File → Settings…).
pub fn window(app: &mut OrchestreApp, ctx: &egui::Context) {
    let mut open = app.settings_open;
    egui::Window::new("Settings")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.set_min_width(300.0);
            ui.label(egui::RichText::new("Note names").strong());
            for naming in NoteNaming::ALL {
                ui.radio_value(&mut app.settings.note_naming, naming, naming.label());
            }
            ui.label(
                egui::RichText::new("French names also number octaves from Do3 for middle C.")
                    .size(11.5)
                    .color(crate::theme::TEXT_DIM),
            );
            ui.add_space(10.0);
            ui.label(egui::RichText::new("Keyboard layout").strong());
            let detected = format!("Automatic (detected: {})", app.settings.kb_layout.label());
            if ui.radio(!app.settings.kb_layout_manual, detected).clicked() {
                app.settings.kb_layout_manual = false;
            }
            for layout in KbLayout::ALL {
                let on = app.settings.kb_layout_manual && app.settings.kb_layout == layout;
                if ui.radio(on, layout.label()).clicked() {
                    app.settings.kb_layout = layout;
                    app.settings.kb_layout_manual = true;
                }
            }
            ui.label(
                egui::RichText::new("Used to show which computer key plays each note.")
                    .size(11.5)
                    .color(crate::theme::TEXT_DIM),
            );
        });
    app.settings_open = open;
}
