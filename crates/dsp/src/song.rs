//! The engine-side view of a project, and the messages exchanged with it.

use orchestre_core::sfx::{PlayOpts, Sound};
use orchestre_core::{FxParams, Id, Instrument, PreviewNote, Project, Tick};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SongNote {
    pub start: Tick,
    pub end: Tick,
    pub pitch: u8,
    pub vel: f32,
}

/// Everything about a track except its notes; can be updated live.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrackParams {
    pub id: Id,
    pub instrument: Instrument,
    pub fx: FxParams,
    pub volume: f32,
    pub pan: f32,
    /// False when muted, or when another track is soloed.
    pub audible: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SongTrack {
    pub params: TrackParams,
    /// Sorted by start.
    pub notes: Vec<SongNote>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Song {
    pub bpm: f32,
    /// Seconds of the echo effect (tempo-synced).
    pub delay_seconds: f32,
    pub length: Tick,
    /// Ticks per beat and per bar (for the metronome).
    pub beat: Tick,
    pub bar: Tick,
    pub tracks: Vec<SongTrack>,
}

impl Default for Song {
    fn default() -> Self {
        Song {
            bpm: 120.0,
            delay_seconds: 0.375,
            length: 0,
            beat: orchestre_core::PPQ,
            bar: 4 * orchestre_core::PPQ,
            tracks: Vec::new(),
        }
    }
}

impl Song {
    pub fn from_project(p: &Project) -> Song {
        let any_solo = p.tracks.iter().any(|t| t.solo);
        let tracks = p
            .tracks
            .iter()
            .map(|t| {
                let mut notes: Vec<SongNote> = t
                    .notes
                    .iter()
                    .map(|n| SongNote {
                        start: n.start,
                        end: n.end(),
                        pitch: n.pitch,
                        vel: n.vel,
                    })
                    .collect();
                notes.sort_by_key(|n| (n.start, n.pitch));
                SongTrack {
                    params: TrackParams {
                        id: t.id,
                        instrument: t.instrument,
                        fx: t.fx,
                        volume: t.volume,
                        pan: t.pan,
                        audible: !t.mute && (!any_solo || t.solo),
                    },
                    notes,
                }
            })
            .collect();
        Song {
            bpm: p.bpm,
            // Dotted eighth: the classic echo that sits well in most grooves.
            delay_seconds: 0.75 * 60.0 / p.bpm,
            length: p.length_ticks(),
            beat: p.time_sig.beat_ticks(),
            bar: p.time_sig.bar_ticks(),
            tracks,
        }
    }

    /// True if `other` differs from `self` only in track parameters
    /// (so the change can be sent without resending every note).
    pub fn same_notes(&self, other: &Song) -> bool {
        self.bpm == other.bpm
            && self.length == other.length
            && self.tracks.len() == other.tracks.len()
            && self
                .tracks
                .iter()
                .zip(&other.tracks)
                .all(|(a, b)| a.params.id == b.params.id && a.notes == b.notes)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Cmd {
    SetSong(Box<Song>),
    SetTrack(TrackParams),
    Play,
    Stop,
    Seek(Tick),
    /// Loop region; `None` stops at the end of the song.
    SetLoop(Option<(Tick, Tick)>),
    LiveNoteOn {
        track: Id,
        pitch: u8,
        vel: f32,
    },
    LiveNoteOff {
        track: Id,
        pitch: u8,
    },
    /// Click on every beat while playing.
    SetMetronome(bool),
    /// Click this many beats, then start playing from the current position.
    CountIn(u32),
    /// Play a short phrase on an instrument outside the song (menu previews).
    Preview {
        instrument: Instrument,
        notes: Vec<PreviewNote>,
    },
    StopPreview,
    /// Play a sound effect once, on top of whatever is playing.
    PlaySound {
        sound: Box<Sound>,
        opts: PlayOpts,
    },
    /// Fade out every sound effect that is playing.
    StopSounds,
    /// End looping sound effects: held layers fade out, repeats stop.
    ReleaseSounds,
    /// Change intensity, pitch and speed of every sound effect playing
    /// (the seed and volume are ignored).
    SoundLive(PlayOpts),
    /// An edited version of the sound: every loop still playing carries on
    /// with it, from where it has got to, crossfaded.
    SwapSound(Box<Sound>),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Event {
    Position {
        tick: f64,
        playing: bool,
    },
    /// Peak level of a track since the last report. Track id 0 = master.
    Level {
        track: Id,
        peak: f32,
    },
    /// A count-in click; `beats_left` includes this one.
    CountIn {
        beats_left: u32,
    },
}
