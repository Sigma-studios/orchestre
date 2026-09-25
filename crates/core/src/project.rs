use serde::{Deserialize, Serialize};

use crate::instrument::{Instrument, InstrumentChoice};
use crate::theory::Key;
use crate::time::{Grid, PPQ, Tick, TimeSig};

pub type Id = u64;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub id: Id,
    pub start: Tick,
    pub len: Tick,
    /// MIDI pitch, or drum piece index on drum tracks.
    pub pitch: u8,
    /// 0..1.
    pub vel: f32,
}

impl Note {
    pub fn end(&self) -> Tick {
        self.start + self.len
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FxParams {
    /// Reverb send, 0..1.
    pub reverb: f32,
    /// Echo send, 0..1.
    pub delay: f32,
    /// Saturation, 0..1.
    pub drive: f32,
}

impl Default for FxParams {
    fn default() -> Self {
        FxParams {
            reverb: 0.15,
            delay: 0.0,
            drive: 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: Id,
    pub name: String,
    pub color: [u8; 3],
    pub instrument: Instrument,
    /// Linear gain, 0..1.5.
    pub volume: f32,
    /// -1 (left) .. 1 (right).
    pub pan: f32,
    pub mute: bool,
    pub solo: bool,
    /// The key this track's notes are locked to. Derived from the song key
    /// and `follow_key` (see [`Project::sync_keys`]); stored so that files
    /// stay readable by older versions.
    pub key_lock: Option<Key>,
    /// Whether this track follows the song key.
    #[serde(default = "yes")]
    pub follow_key: bool,
    pub fx: FxParams,
    pub notes: Vec<Note>,
}

fn yes() -> bool {
    true
}

impl Track {
    pub fn note(&self, id: Id) -> Option<&Note> {
        self.notes.iter().find(|n| n.id == id)
    }

    /// Valid pitch range for this track's notes.
    pub fn pitch_range(&self) -> (u8, u8) {
        if self.instrument.is_drums() {
            (0, crate::instrument::DrumPiece::ALL.len() as u8 - 1)
        } else {
            (21, 108)
        }
    }

    /// Key lock only makes sense for pitched instruments.
    pub fn effective_key(&self) -> Option<Key> {
        if self.instrument.is_drums() {
            None
        } else {
            self.key_lock
        }
    }
}

pub const TRACK_COLORS: [[u8; 3]; 8] = [
    [239, 108, 94],
    [245, 176, 65],
    [120, 200, 110],
    [80, 180, 230],
    [160, 130, 240],
    [236, 110, 180],
    [70, 200, 190],
    [200, 200, 90],
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub bpm: f32,
    pub time_sig: TimeSig,
    pub grid: Grid,
    pub length_bars: u32,
    pub tracks: Vec<Track>,
    pub next_id: Id,
    /// The song's key: melodic tracks that follow it only show its notes.
    #[serde(default)]
    pub key: Option<Key>,
}

impl Default for Project {
    fn default() -> Self {
        Project {
            bpm: 110.0,
            time_sig: TimeSig::default(),
            grid: Grid::Sixteenth,
            length_bars: 8,
            tracks: Vec::new(),
            next_id: 1,
            key: None,
        }
    }
}

impl Project {
    pub fn new_id(&mut self) -> Id {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn track(&self, id: Id) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    pub fn track_mut(&mut self, id: Id) -> Option<&mut Track> {
        self.tracks.iter_mut().find(|t| t.id == id)
    }

    pub fn add_track(&mut self, choice: InstrumentChoice) -> Id {
        let id = self.new_id();
        let color = TRACK_COLORS[self.tracks.len() % TRACK_COLORS.len()];
        let base = choice.label();
        let same = self
            .tracks
            .iter()
            .filter(|t| t.name.starts_with(base))
            .count();
        let name = if same == 0 {
            base.to_string()
        } else {
            format!("{base} {}", same + 1)
        };
        self.tracks.push(Track {
            id,
            name,
            color,
            instrument: choice.instrument(),
            volume: 0.8,
            pan: 0.0,
            mute: false,
            solo: false,
            key_lock: None,
            follow_key: true,
            fx: FxParams::default(),
            notes: Vec::new(),
        });
        self.sync_keys();
        id
    }

    /// Update every track's key lock from the song key.
    pub fn sync_keys(&mut self) {
        let key = self.key;
        for t in &mut self.tracks {
            t.key_lock = if t.follow_key && !t.instrument.is_drums() {
                key
            } else {
                None
            };
        }
    }

    /// Tracks affected by the song key.
    fn keyed_tracks(&mut self) -> impl Iterator<Item = &mut Track> {
        self.tracks
            .iter_mut()
            .filter(|t| t.follow_key && !t.instrument.is_drums())
    }

    /// Notes that don't fit `key`, across the tracks following the song key.
    pub fn notes_outside(&self, key: Key) -> usize {
        self.tracks
            .iter()
            .filter(|t| t.follow_key && !t.instrument.is_drums())
            .map(|t| crate::edit::notes_outside_key(t, key).len())
            .sum()
    }

    /// Change the song key. Notes that don't fit are handled as `change`
    /// says; with [`KeyChange::Transpose`] every note moves into the new key.
    pub fn set_key(&mut self, key: Option<Key>, change: crate::edit::KeyChange) {
        use crate::edit::{KeyChange, KeyConflict, apply_key_lock, transpose_track};
        let from = self.key;
        if let Some(to) = key {
            for t in self.keyed_tracks() {
                match (change, from) {
                    (KeyChange::Transpose, Some(from)) => transpose_track(t, from, to),
                    (KeyChange::Snap, _) => apply_key_lock(t, to, KeyConflict::Snap),
                    _ => apply_key_lock(t, to, KeyConflict::Delete),
                }
            }
        }
        self.key = key;
        self.sync_keys();
    }

    /// Make one track follow (or stop following) the song key.
    pub fn set_follow(&mut self, track: Id, follow: bool, conflict: crate::edit::KeyConflict) {
        let key = self.key;
        let Some(t) = self.track_mut(track) else {
            return;
        };
        t.follow_key = follow;
        if follow && let Some(k) = key {
            crate::edit::apply_key_lock(t, k, conflict);
        }
        self.sync_keys();
    }

    pub fn grid_ticks(&self) -> Tick {
        self.grid.ticks(self.time_sig)
    }

    pub fn length_ticks(&self) -> Tick {
        self.length_bars as Tick * self.time_sig.bar_ticks()
    }

    /// End of the last note across all tracks.
    pub fn content_end(&self) -> Tick {
        self.tracks
            .iter()
            .flat_map(|t| t.notes.iter().map(Note::end))
            .max()
            .unwrap_or(0)
    }

    /// Grow the song so that it always covers all notes plus one spare bar.
    pub fn fit_length(&mut self) {
        let bar = self.time_sig.bar_ticks();
        let needed = (self.content_end() + bar - 1) / bar;
        self.length_bars = self.length_bars.max(needed as u32).max(1);
    }

    pub fn seconds_per_tick(&self) -> f64 {
        60.0 / self.bpm as f64 / PPQ as f64
    }

    /// A small starter song so first-time users hear something immediately.
    pub fn demo() -> Project {
        use crate::instrument::SynthPreset;
        let mut p = Project::default();
        let q = PPQ;
        let e = PPQ / 2;
        let s = PPQ / 4;
        let bar = p.time_sig.bar_ticks();

        let drums = p.add_track(InstrumentChoice::Drums);
        let piano = p.add_track(InstrumentChoice::Piano);
        let bass = p.add_track(InstrumentChoice::Synth(SynthPreset::Bass));
        let lead = p.add_track(InstrumentChoice::Synth(SynthPreset::Pluck));

        let mut notes: Vec<(Id, Tick, Tick, u8, f32)> = Vec::new();
        for b in 0..8 {
            let t0 = b * bar;
            // Kick on 1 and 3, snare on 2 and 4, hats on 8ths.
            for k in [0, 2 * q] {
                notes.push((drums, t0 + k, s, 0, 1.0));
            }
            for k in [q, 3 * q] {
                notes.push((drums, t0 + k, s, 1, 0.9));
            }
            for i in 0..8 {
                notes.push((drums, t0 + i * e, s, 4, if i % 2 == 0 { 0.7 } else { 0.45 }));
            }
        }
        // i - VI - III - VII in A minor.
        let chords: [[u8; 3]; 4] = [[57, 60, 64], [53, 57, 60], [55, 60, 64], [55, 59, 62]];
        let roots: [u8; 4] = [45, 41, 36, 43];
        for b in 0..8 {
            let t0 = b * bar;
            let c = (b as usize) % 4;
            for &pitch in &chords[c] {
                notes.push((piano, t0, bar - s, pitch, 0.6));
            }
            for i in 0..4 {
                notes.push((bass, t0 + i * q, e + s, roots[c], 0.9));
            }
        }
        let melody: [(Tick, Tick, u8); 8] = [
            (0, e, 76),
            (e, e, 72),
            (q, e, 69),
            (q + e, e, 72),
            (2 * q, q, 74),
            (3 * q, e, 72),
            (3 * q + e, e, 71),
            (4 * q, q, 69),
        ];
        for rep in [4, 6] {
            for &(t, l, pitch) in &melody {
                notes.push((lead, rep * bar + t, l, pitch, 0.8));
            }
        }
        for (track, start, len, pitch, vel) in notes {
            let id = p.new_id();
            p.track_mut(track).unwrap().notes.push(Note {
                id,
                start,
                len,
                pitch,
                vel,
            });
        }
        p.key = Some(Key::new(9, crate::theory::Scale::Minor));
        p.sync_keys();
        p.track_mut(lead).unwrap().fx.delay = 0.35;
        p
    }
}
