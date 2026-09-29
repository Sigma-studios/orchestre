//! Hear an instrument by resting the pointer on it in a menu.

use orchestre_core::InstrumentChoice;
use orchestre_dsp::Cmd;

use crate::app::OrchestreApp;

/// Pointer must rest this long on an entry before it plays (so sweeping
/// across a menu doesn't fire a burst of sounds).
const DELAY: f64 = 0.15;

/// Something that can be heard from a menu.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Item {
    Choice(InstrumentChoice),
    /// Index in "My sounds".
    Mine(usize),
}

#[derive(Default)]
pub struct PreviewState {
    hovered: Option<Item>,
    candidate: Option<Item>,
    since: f64,
    playing: Option<Item>,
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
        app.preview.hovered = Some(Item::Choice(choice));
    }
}

/// Like [`hover`], for an entry of "My sounds".
pub fn hover_mine(app: &mut OrchestreApp, resp: &egui::Response, index: usize) {
    if resp.hovered() {
        app.preview.hovered = Some(Item::Mine(index));
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
            let cmd = match c {
                Item::Choice(c) => Some(Cmd::Preview {
                    instrument: c.instrument(),
                    notes: c.preview_notes(),
                    arp: c.default_arp(),
                }),
                Item::Mine(i) => app.my_sounds.get(i).map(|s| Cmd::Preview {
                    instrument: s.instrument.clone(),
                    notes: s.instrument.choice().preview_notes(),
                    arp: s.arp,
                }),
            };
            if let Some(cmd) = cmd {
                app.send(cmd);
            }
        }
        None if app.preview.playing.is_some() => {
            app.preview.playing = None;
            app.send(Cmd::StopPreview);
        }
        _ => {}
    }
}
