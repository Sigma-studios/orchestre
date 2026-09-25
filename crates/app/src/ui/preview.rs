//! Hear an instrument by resting the pointer on it in a menu.

use orchestre_core::InstrumentChoice;
use orchestre_dsp::Cmd;

use crate::app::OrchestreApp;

/// Pointer must rest this long on an entry before it plays (so sweeping
/// across a menu doesn't fire a burst of sounds).
const DELAY: f64 = 0.15;

#[derive(Default)]
pub struct PreviewState {
    hovered: Option<InstrumentChoice>,
    candidate: Option<InstrumentChoice>,
    since: f64,
    playing: Option<InstrumentChoice>,
}

impl PreviewState {
    /// Whether frames are needed to fire a pending preview.
    pub fn pending(&self) -> bool {
        self.candidate.is_some() && self.candidate != self.playing
    }
}

/// Call for each menu entry that can be previewed.
pub fn hover(app: &mut OrchestreApp, resp: &egui::Response, choice: InstrumentChoice) {
    if resp.hovered() {
        app.preview.hovered = Some(choice);
    }
}

/// Call once per frame, after all menus have been drawn.
pub fn end_frame(app: &mut OrchestreApp) {
    let now = app.now;
    let hovered = app.preview.hovered.take();
    if hovered != app.preview.candidate {
        app.preview.candidate = hovered;
        app.preview.since = now;
    }
    match app.preview.candidate {
        Some(c) if app.preview.playing != Some(c) && now - app.preview.since >= DELAY => {
            app.preview.playing = Some(c);
            app.send(Cmd::Preview {
                instrument: c.instrument(),
                notes: c.preview_notes(),
            });
        }
        None if app.preview.playing.is_some() => {
            app.preview.playing = None;
            app.send(Cmd::StopPreview);
        }
        _ => {}
    }
}
