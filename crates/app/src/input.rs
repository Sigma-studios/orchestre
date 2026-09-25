//! Keyboard shortcuts and the computer-keyboard piano.

use std::collections::HashSet;

use egui::{Event, Key};
use orchestre_core::edit::{self, Clip};
use orchestre_core::{DrumPiece, Id, snap_floor, snap_round};
use orchestre_dsp::Cmd;

use crate::app::OrchestreApp;
use crate::record::Source;

/// Keyboard piano layout by key *position* (named after US QWERTY):
/// home row = white keys, row above = black keys. Returns the semitone
/// above the octave's C.
fn position_semitone(key: Key) -> Option<i32> {
    Some(match key {
        Key::A => 0,
        Key::W => 1,
        Key::S => 2,
        Key::E => 3,
        Key::D => 4,
        Key::F => 5,
        Key::T => 6,
        Key::G => 7,
        Key::Y => 8,
        Key::H => 9,
        Key::U => 10,
        Key::J => 11,
        Key::K => 12,
        Key::O => 13,
        Key::L => 14,
        Key::P => 15,
        Key::Semicolon => 16,
        _ => return None,
    })
}

/// Fallback for key events without a physical position (rare; both native
/// and web normally report one): map typed letters, adding the AZERTY
/// letters found at those positions.
fn letter_semitone(key: Key) -> Option<i32> {
    match key {
        Key::Q => Some(0),
        Key::Z => Some(1),
        Key::M => Some(16),
        _ => position_semitone(key),
    }
}

fn keyboard_semitone(key: Key, physical_key: Option<Key>) -> Option<i32> {
    match physical_key {
        Some(p) => position_semitone(p),
        None => letter_semitone(key),
    }
}

/// White-key semitones of the home row, left to right. Drum tracks play
/// one drum per home-row key, in order.
pub const HOME_ROW: [i32; 10] = [0, 2, 4, 5, 7, 9, 11, 12, 14, 16];

/// Semitones of the row above (the piano's black keys), left to right.
/// Drum tracks put the percussion pieces there.
pub const BLACK_ROW: [i32; 7] = [1, 3, 6, 8, 10, 13, 15];

/// Keyboard semitone position that plays drum row `index`.
pub fn drum_semitone(index: usize) -> Option<i32> {
    if index < DrumPiece::KIT_LEN {
        HOME_ROW.get(index).copied()
    } else {
        BLACK_ROW.get(index - DrumPiece::KIT_LEN).copied()
    }
}

/// Drum row played by the key at `semi`, if any.
pub fn drum_at_semitone(semi: i32) -> Option<u8> {
    let index = match HOME_ROW.iter().position(|&s| s == semi) {
        Some(i) => i,
        None => DrumPiece::KIT_LEN + BLACK_ROW.iter().position(|&s| s == semi)?,
    };
    (index < DrumPiece::ALL.len()).then_some(index as u8)
}

/// Keyboard layout, only used to print the right letters on the note
/// editor's keys (playing always goes by key position).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum KbLayout {
    #[default]
    Qwerty,
    Azerty,
    Qwertz,
}

impl KbLayout {
    pub const ALL: [KbLayout; 3] = [KbLayout::Qwerty, KbLayout::Azerty, KbLayout::Qwertz];

    pub fn label(self) -> &'static str {
        match self {
            KbLayout::Qwerty => "QWERTY",
            KbLayout::Azerty => "AZERTY",
            KbLayout::Qwertz => "QWERTZ",
        }
    }

    pub fn from_label(s: &str) -> Option<KbLayout> {
        Self::ALL.into_iter().find(|l| l.label() == s)
    }

    /// Printed letter of the key that plays `semi` semitones above C.
    pub fn key_label(self, semi: i32) -> Option<&'static str> {
        const QWERTY: [&str; 17] = [
            "A", "W", "S", "E", "D", "F", "T", "G", "Y", "H", "U", "J", "K", "O", "L", "P", ";",
        ];
        let base = *QWERTY.get(usize::try_from(semi).ok()?)?;
        Some(match (self, semi) {
            (KbLayout::Azerty, 0) => "Q",
            (KbLayout::Azerty, 1) => "Z",
            (KbLayout::Azerty, 16) => "M",
            (KbLayout::Qwertz, 8) => "Z",
            (KbLayout::Qwertz, 16) => "Ö",
            _ => base,
        })
    }

    /// Refine the guess from a key press: which letter the key at a given
    /// position typed.
    fn detect(self, key: Key, physical: Option<Key>) -> KbLayout {
        let Some(physical) = physical else {
            return self;
        };
        match (physical, key) {
            (Key::A, Key::Q)
            | (Key::Q, Key::A)
            | (Key::W, Key::Z)
            | (Key::Z, Key::W)
            | (Key::Semicolon, Key::M) => KbLayout::Azerty,
            (Key::Y, Key::Z) | (Key::Z, Key::Y) => KbLayout::Qwertz,
            (Key::A, Key::A) | (Key::Q, Key::Q) | (Key::W, Key::W) if self == KbLayout::Azerty => {
                KbLayout::Qwerty
            }
            (Key::Y, Key::Y) | (Key::Z, Key::Z) if self == KbLayout::Qwertz => KbLayout::Qwerty,
            _ => self,
        }
    }
}

/// The computer key playing `pitch` on a track (drum row index on drum
/// tracks), as printed on the user's keyboard.
pub fn key_label_for(app: &OrchestreApp, pitch: u8, drums: bool) -> Option<&'static str> {
    let semi = if drums {
        drum_semitone(pitch as usize)?
    } else {
        pitch as i32 - (app.octave as i32 + 1) * 12
    };
    app.kb_layout.key_label(semi)
}

pub fn handle(app: &mut OrchestreApp, ctx: &egui::Context) {
    if !ctx.input(|i| i.focused) {
        app.release_held_keys();
        return;
    }
    if ctx.egui_wants_keyboard_input()
        || app.key_prompt.is_some()
        || app.confirm_delete_track.is_some()
    {
        return;
    }
    let events = ctx.input(|i| i.events.clone());
    for ev in events {
        match ev {
            Event::Copy => copy(app, ctx, false),
            Event::Cut => copy(app, ctx, true),
            Event::Paste(text) => paste(app, &text),
            Event::Key {
                key,
                physical_key,
                pressed,
                repeat,
                modifiers,
            } => {
                if pressed && !app.kb_layout_manual {
                    app.kb_layout = app.kb_layout.detect(key, physical_key);
                }
                if modifiers.command {
                    if pressed {
                        command_key(app, ctx, key, modifiers.shift);
                    }
                    continue;
                }
                if let Some(semi) = keyboard_semitone(key, physical_key)
                    && !modifiers.alt
                {
                    if !repeat {
                        piano_key_event(app, key, semi, pressed);
                    }
                    continue;
                }
                if pressed {
                    plain_key(app, key, modifiers.shift, modifiers.alt);
                }
            }
            _ => {}
        }
    }
}

fn command_key(app: &mut OrchestreApp, ctx: &egui::Context, key: Key, shift: bool) {
    match key {
        Key::Z if shift => app.redo(),
        Key::Z => app.undo(),
        Key::Y => app.redo(),
        Key::A => select_all(app),
        Key::D => duplicate(app),
        Key::S => crate::io::save(app),
        Key::O => crate::io::open(app),
        // Some platforms send these as keys rather than clipboard events.
        Key::C => copy(app, ctx, false),
        Key::X => copy(app, ctx, true),
        _ => {}
    }
}

fn plain_key(app: &mut OrchestreApp, key: Key, shift: bool, alt: bool) {
    match key {
        Key::Space => app.toggle_play(),
        Key::R => app.toggle_record(),
        Key::Enter | Key::Home => app.seek(0.0),
        Key::Escape => {
            if app.selection.is_empty() {
                app.select_track(None);
            } else {
                app.selection.clear();
            }
        }
        Key::Delete | Key::Backspace => delete_selection(app),
        Key::ArrowLeft | Key::ArrowRight => {
            let step = if alt {
                app.project.grid_ticks() / 4
            } else {
                app.project.grid_ticks()
            }
            .max(1);
            let dt = if key == Key::ArrowLeft { -step } else { step };
            nudge(app, dt, 0);
        }
        Key::ArrowUp | Key::ArrowDown => {
            let drums = app
                .selected_track()
                .is_some_and(|t| t.instrument.is_drums());
            let locked = app
                .selected_track()
                .and_then(|t| t.effective_key())
                .is_some();
            let octave = if locked { 7 } else { 12 };
            let mut steps = if shift && !drums { octave } else { 1 };
            if key == Key::ArrowDown {
                steps = -steps;
            }
            // Drum rows go downwards as the index grows.
            if drums {
                steps = -steps;
            }
            nudge(app, 0, steps);
        }
        Key::Minus => app.octave = (app.octave - 1).max(0),
        Key::Equals | Key::Plus => app.octave = (app.octave + 1).min(8),
        _ => {}
    }
}

fn piano_key_event(app: &mut OrchestreApp, key: Key, semi: i32, pressed: bool) {
    let Some(track) = app.selected_track() else {
        return;
    };
    let id = track.id;
    if pressed {
        let pitch = if track.instrument.is_drums() {
            match drum_at_semitone(semi) {
                Some(i) => i,
                None => return,
            }
        } else {
            let p = ((app.octave as i32 + 1) * 12 + semi).clamp(0, 127) as u8;
            track.effective_key().map_or(p, |k| k.nearest(p))
        };
        if let Some((t, p)) = app.held_keys.insert(key, (id, pitch)) {
            app.send(Cmd::LiveNoteOff { track: t, pitch: p });
        }
        app.send(Cmd::LiveNoteOn {
            track: id,
            pitch,
            vel: 0.85,
        });
        app.record_note_on(Source::Key(key), pitch, 0.85);
    } else if let Some((t, p)) = app.held_keys.remove(&key) {
        app.send(Cmd::LiveNoteOff { track: t, pitch: p });
        app.record_note_off(Source::Key(key));
    }
}

pub fn select_all(app: &mut OrchestreApp) {
    if let Some(t) = app.selected_track() {
        app.selection = t.notes.iter().map(|n| n.id).collect();
    }
}

pub fn delete_selection(app: &mut OrchestreApp) {
    if app.selection.is_empty() {
        return;
    }
    let sel = std::mem::take(&mut app.selection);
    if let Some(t) = app.selected.and_then(|id| app.project.track_mut(id)) {
        edit::delete_notes(t, &sel);
        app.touch();
    }
}

fn nudge(app: &mut OrchestreApp, dt: i64, steps: i32) {
    if app.selection.is_empty() {
        return;
    }
    let sel = app.selection.clone();
    if let Some(t) = app.selected.and_then(|id| app.project.track_mut(id)) {
        edit::move_notes(t, &sel, dt, steps);
        let first = t
            .notes
            .iter()
            .find(|n| sel.contains(&n.id))
            .map(|n| (t.id, n.pitch));
        app.touch();
        if steps != 0
            && sel.len() == 1
            && let Some((track, pitch)) = first
        {
            app.audition(track, pitch);
        }
    }
}

pub fn duplicate(app: &mut OrchestreApp) {
    let step = app.project.time_sig.beat_ticks();
    let mut next = app.project.next_id;
    let sel = app.selection.clone();
    if let Some(t) = app.selected.and_then(|id| app.project.track_mut(id)) {
        let new = edit::duplicate(t, &sel, step, &mut next);
        if !new.is_empty() {
            app.project.next_id = next;
            app.selection = new;
            app.touch();
        }
    }
}

fn copy(app: &mut OrchestreApp, ctx: &egui::Context, cut: bool) {
    let Some(track) = app.selected_track() else {
        return;
    };
    if app.selection.is_empty() {
        return;
    }
    let clip = Clip::from_selection(track, &app.selection);
    // Also put the notes on the system clipboard, so pasting works even
    // across windows (and so the platform sends us a paste event).
    if let Ok(json) = serde_json::to_string(&clip) {
        ctx.copy_text(format!("orchestre-notes:{json}"));
    }
    let n = clip.notes.len();
    app.clipboard = clip;
    if cut {
        delete_selection(app);
        app.notify(format!("Cut {n} note{}", if n == 1 { "" } else { "s" }));
    } else {
        app.notify(format!("Copied {n} note{}", if n == 1 { "" } else { "s" }));
    }
}

fn paste(app: &mut OrchestreApp, text: &str) {
    if let Some(json) = text.strip_prefix("orchestre-notes:")
        && let Ok(clip) = serde_json::from_str::<Clip>(json)
    {
        app.clipboard = clip;
    }
    if app.clipboard.is_empty() {
        return;
    }
    let step = app.project.grid_ticks();
    // Paste under the mouse if it is over the editor, else at the playhead.
    let at = match app.hover_tick {
        Some(t) => snap_floor(t, step),
        None => snap_round(app.position as i64, step),
    }
    .max(0);
    let mut next = app.project.next_id;
    let clip = app.clipboard.clone();
    let Some(t) = app.selected.and_then(|id| app.project.track_mut(id)) else {
        app.notify("Select a track to paste into");
        return;
    };
    let new: HashSet<Id> = edit::paste(t, &clip, at, &mut next);
    if new.is_empty() {
        app.notify(
            "Drum notes can only be pasted into drum tracks (and melodies into melodic tracks)",
        );
        return;
    }
    app.project.next_id = next;
    app.selection = new;
    app.touch();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_detection_and_labels() {
        let l = KbLayout::Qwerty.detect(Key::Q, Some(Key::A));
        assert_eq!(l, KbLayout::Azerty);
        assert_eq!(l.key_label(0), Some("Q"));
        assert_eq!(l.key_label(2), Some("S"));
        assert_eq!(l.key_label(17), None);
        assert_eq!(l.detect(Key::A, Some(Key::A)), KbLayout::Qwerty);
        assert_eq!(
            KbLayout::Qwerty.detect(Key::Z, Some(Key::Y)),
            KbLayout::Qwertz
        );
        assert_eq!(
            KbLayout::Azerty.detect(Key::S, Some(Key::S)),
            KbLayout::Azerty
        );
        for l in KbLayout::ALL {
            assert_eq!(KbLayout::from_label(l.label()), Some(l));
        }
    }

    #[test]
    fn every_drum_has_its_own_key() {
        let mut semis: Vec<i32> = (0..DrumPiece::ALL.len())
            .map(|i| drum_semitone(i).unwrap())
            .collect();
        for (i, &s) in semis.iter().enumerate() {
            assert_eq!(drum_at_semitone(s), Some(i as u8));
        }
        semis.sort();
        semis.dedup();
        assert_eq!(semis.len(), DrumPiece::ALL.len(), "two drums share a key");
        // Percussion sits on the black-key row: W E T Y U O P on QWERTY.
        let letters: Vec<_> = (DrumPiece::KIT_LEN..DrumPiece::ALL.len())
            .map(|i| {
                KbLayout::Qwerty
                    .key_label(drum_semitone(i).unwrap())
                    .unwrap()
            })
            .collect();
        assert_eq!(letters, ["W", "E", "T", "Y", "U", "O", "P"]);
    }

    #[test]
    fn physical_positions_ignore_letter_aliases() {
        // AZERTY "A" sits at the QWERTY Q position: not a piano key.
        assert_eq!(keyboard_semitone(Key::A, Some(Key::Q)), None);
        // AZERTY "Q" sits at the QWERTY A position: C.
        assert_eq!(keyboard_semitone(Key::Q, Some(Key::A)), Some(0));
        assert_eq!(keyboard_semitone(Key::M, Some(Key::Semicolon)), Some(16));
    }

    #[test]
    fn letters_cover_qwerty_and_azerty_home_rows() {
        let qwerty = [
            Key::A,
            Key::S,
            Key::D,
            Key::F,
            Key::G,
            Key::H,
            Key::J,
            Key::K,
            Key::L,
            Key::Semicolon,
        ];
        let azerty = [
            Key::Q,
            Key::S,
            Key::D,
            Key::F,
            Key::G,
            Key::H,
            Key::J,
            Key::K,
            Key::L,
            Key::M,
        ];
        for row in [qwerty, azerty] {
            let semis: Vec<i32> = row
                .iter()
                .map(|&k| keyboard_semitone(k, None).unwrap())
                .collect();
            assert_eq!(semis, HOME_ROW);
        }
    }
}
