//! Sound effects mode: the library on the left, layers at the bottom and
//! the selected layer's (or the sound's) settings in the middle.

mod bar;
mod curve;
mod layers;
mod library;
mod panel;

use egui::Ui;

use crate::app::OrchestreApp;
use crate::sfx::Mode;

pub fn show(app: &mut OrchestreApp, ui: &mut Ui) {
    bar::show(app, ui);
    library::show(app, ui);
    layers::show(app, ui);
    egui::CentralPanel::default()
        .frame(egui::Frame::central_panel(ui.style()).inner_margin(0.0))
        .show(ui, |ui| panel::show(app, ui));
}

/// Switch between making music and making sound effects.
pub fn mode_switch(app: &mut OrchestreApp, ui: &mut Ui) {
    let mut mode = app.mode;
    ui.selectable_value(&mut mode, Mode::Music, "🎵 Music")
        .on_hover_text("Write songs");
    ui.selectable_value(&mut mode, Mode::Sounds, "💥 Sound effects")
        .on_hover_text("Make sound effects for games: impacts, lasers, footsteps…");
    app.set_mode(mode);
}

/// Seconds as shown to people: "0.25 s", "1.5 s", "12 ms".
pub fn format_secs(s: f32) -> String {
    if s < 0.1 {
        format!("{:.0} ms", s * 1000.0)
    } else {
        format!("{s:.2} s")
    }
}
