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
    /// DJ filter: below 0 cuts highs (low-pass), above 0 cuts lows
    /// (high-pass); 0 is off.
    #[serde(default)]
    pub filter: f32,
    /// Phaser: a sweeping, swooshing notch, 0..1.
    #[serde(default)]
    pub phaser: f32,
    /// Ducks the track on every beat, like sidechain compression from a
    /// four-on-the-floor kick, 0..1.
    #[serde(default)]
    pub pump: f32,
}

impl Default for FxParams {
    fn default() -> Self {
        FxParams {
            reverb: 0.15,
            delay: 0.0,
            drive: 0.0,
            filter: 0.0,
            phaser: 0.0,
            pump: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ArpPattern {
    #[default]
    Up,
    Down,
    UpDown,
    /// In the order the notes were played.
    Played,
    Random,
}

impl ArpPattern {
    pub const ALL: [ArpPattern; 5] = [
        ArpPattern::Up,
        ArpPattern::Down,
        ArpPattern::UpDown,
        ArpPattern::Played,
        ArpPattern::Random,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ArpPattern::Up => "Up",
            ArpPattern::Down => "Down",
            ArpPattern::UpDown => "Up & down",
            ArpPattern::Played => "As played",
            ArpPattern::Random => "Random",
        }
    }
}

/// Arpeggiator note values, in beats per step.
pub const ARP_RATES: [(f32, &str); 5] = [
    (0.125, "1/32"),
    (0.25, "1/16"),
    (1.0 / 3.0, "1/8 triplet"),
    (0.5, "1/8"),
    (1.0, "1/4"),
];

/// Plays held chords as a rhythmic pattern of single notes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArpParams {
    pub on: bool,
    /// Beats per step.
    pub rate: f32,
    pub pattern: ArpPattern,
    /// Octaves the pattern climbs through, 1..=4.
    pub octaves: u8,
    /// Note length as a fraction of a step, 0..1.
    pub gate: f32,
}

impl Default for ArpParams {
    fn default() -> Self {
        ArpParams {
            on: false,
            rate: 0.25,
            pattern: ArpPattern::Up,
            octaves: 1,
            gate: 0.6,
        }
    }
}

/// What an automation lane controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AutoTarget {
    Volume,
    Pan,
    Filter,
    /// The instrument's own filter (classic synth only).
    Brightness,
    Reverb,
    Echo,
    Drive,
    Phaser,
    Pump,
}

impl AutoTarget {
    pub const ALL: [AutoTarget; 9] = [
        AutoTarget::Filter,
        AutoTarget::Brightness,
        AutoTarget::Volume,
        AutoTarget::Pan,
        AutoTarget::Reverb,
        AutoTarget::Echo,
        AutoTarget::Drive,
        AutoTarget::Phaser,
        AutoTarget::Pump,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AutoTarget::Volume => "Volume",
            AutoTarget::Pan => "Left ↔ Right",
            AutoTarget::Filter => "Filter sweep",
            AutoTarget::Brightness => "Synth brightness",
            AutoTarget::Reverb => "Space (reverb)",
            AutoTarget::Echo => "Echo",
            AutoTarget::Drive => "Grit (drive)",
            AutoTarget::Phaser => "Phaser",
            AutoTarget::Pump => "Pump",
        }
    }

    pub fn range(self) -> (f32, f32) {
        match self {
            AutoTarget::Volume => (0.0, 1.5),
            AutoTarget::Pan | AutoTarget::Filter | AutoTarget::Brightness => (-1.0, 1.0),
            _ => (0.0, 1.0),
        }
    }

    /// The track's value when not automated.
    pub fn current(self, t: &Track) -> f32 {
        match self {
            AutoTarget::Volume => t.volume,
            AutoTarget::Pan => t.pan,
            AutoTarget::Filter => t.fx.filter,
            AutoTarget::Brightness => 0.0,
            AutoTarget::Reverb => t.fx.reverb,
            AutoTarget::Echo => t.fx.delay,
            AutoTarget::Drive => t.fx.drive,
            AutoTarget::Phaser => t.fx.phaser,
            AutoTarget::Pump => t.fx.pump,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutoPoint {
    pub tick: Tick,
    pub value: f32,
}

/// A parameter drawn over time: straight lines between points, flat
/// before the first and after the last.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutoLane {
    pub target: AutoTarget,
    /// Sorted by tick.
    pub points: Vec<AutoPoint>,
}

impl AutoLane {
    pub fn value_at(&self, tick: f64) -> Option<f32> {
        let pts = &self.points;
        let first = pts.first()?;
        let i = pts.partition_point(|p| (p.tick as f64) <= tick);
        if i == 0 {
            return Some(first.value);
        }
        let a = pts[i - 1];
        let Some(b) = pts.get(i) else {
            return Some(a.value);
        };
        let span = (b.tick - a.tick) as f64;
        let t = if span > 0.0 {
            ((tick - a.tick as f64) / span) as f32
        } else {
            1.0
        };
        Some(a.value + (b.value - a.value) * t)
    }

    pub fn sort(&mut self) {
        self.points.sort_by_key(|p| p.tick);
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
    #[serde(default)]
    pub arp: ArpParams,
    #[serde(default)]
    pub automation: Vec<AutoLane>,
}

fn yes() -> bool {
    true
}

impl Track {
    pub fn lane(&self, target: AutoTarget) -> Option<&AutoLane> {
        self.automation.iter().find(|l| l.target == target)
    }

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
            arp: choice.default_arp(),
            automation: Vec::new(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automation_interpolates_between_points() {
        let lane = AutoLane {
            target: AutoTarget::Filter,
            points: vec![
                AutoPoint {
                    tick: 100,
                    value: -1.0,
                },
                AutoPoint {
                    tick: 200,
                    value: 1.0,
                },
            ],
        };
        assert_eq!(lane.value_at(0.0), Some(-1.0));
        assert_eq!(lane.value_at(150.0), Some(0.0));
        assert_eq!(lane.value_at(1000.0), Some(1.0));
        let empty = AutoLane {
            target: AutoTarget::Filter,
            points: Vec::new(),
        };
        assert_eq!(empty.value_at(0.0), None);
    }

    #[test]
    fn old_tracks_load_without_arp_or_automation() {
        let mut p = Project::default();
        p.add_track(InstrumentChoice::Piano);
        let mut v = serde_json::to_value(&p).unwrap();
        let t = v["tracks"][0].as_object_mut().unwrap();
        t.remove("arp");
        t.remove("automation");
        for k in ["filter", "phaser", "pump"] {
            t.get_mut("fx").unwrap().as_object_mut().unwrap().remove(k);
        }
        let back: Project = serde_json::from_value(v).unwrap();
        assert_eq!(back, p);
    }
}
