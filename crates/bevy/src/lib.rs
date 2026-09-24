//! Play Orchestre songs (`.orch` files) in Bevy. The music is synthesized
//! live by the Orchestre engine, so a song costs a few kilobytes on disk
//! instead of megabytes of audio.
//!
//! ```no_run
//! use bevy::prelude::*;
//! use orchestre_bevy::{OrchestrePlugin, OrchestreSong};
//!
//! fn main() {
//!     App::new()
//!         .add_plugins((DefaultPlugins, OrchestrePlugin))
//!         .add_systems(Startup, |mut commands: Commands, assets: Res<AssetServer>| {
//!             let song: Handle<OrchestreSong> = assets.load("music/theme.orch");
//!             commands.spawn(AudioPlayer(song));
//!         })
//!         .run();
//! }
//! ```
//!
//! Songs loop seamlessly by default (the engine loops on the beat, with no
//! gap and no extra memory). Don't use `PlaybackSettings::LOOP` with them:
//! Bevy would buffer the whole song in memory and loop *after* the reverb
//! tail. To play a song once, load it with [`OrchestreSettings`]:
//!
//! ```no_run
//! # use bevy::prelude::*;
//! # use orchestre_bevy::{OrchestreSettings, OrchestreSong};
//! # fn system(assets: Res<AssetServer>) {
//! let jingle: Handle<OrchestreSong> =
//!     assets.load_with_settings("music/win.orch", |s: &mut OrchestreSettings| s.looping = false);
//! # }
//! ```

use std::sync::Arc;

use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, LoadContext};
use bevy::audio::{AddAudioSource, ChannelCount, Decodable, Sample, SampleRate, Source};
use bevy::prelude::*;
use orchestre_dsp::{Cmd, Engine, Song};
use serde::{Deserialize, Serialize};

pub use orchestre_core::Project;

/// Rodio resamples to the output device, so any common rate works.
const SAMPLE_RATE: u32 = 48_000;
const BLOCK_FRAMES: usize = 512;
/// Non-looping songs stop once their tail has been silent this long...
const SILENCE_TO_STOP: f32 = 0.25;
/// ...or at the latest this long after the last bar.
const MAX_TAIL: f32 = 8.0;

/// Registers `.orch` files as playable audio assets.
pub struct OrchestrePlugin;

impl Plugin for OrchestrePlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<OrchestreSong>()
            .register_asset_loader(OrchestreLoader);
    }
}

/// A song loaded from a `.orch` file. Play it with `AudioPlayer(handle)`.
#[derive(Asset, TypePath, Clone)]
pub struct OrchestreSong {
    song: Arc<Song>,
    /// Loop the whole song seamlessly (default), or play it once.
    pub looping: bool,
}

impl OrchestreSong {
    /// Build a song from a project in code (e.g. generated music).
    pub fn from_project(project: &Project, looping: bool) -> Self {
        OrchestreSong {
            song: Arc::new(Song::from_project(project)),
            looping,
        }
    }

    /// Length of one pass through the song, in seconds (excluding tails).
    pub fn duration_secs(&self) -> f64 {
        orchestre_core::ticks_to_seconds(self.song.length as f64, self.song.bpm as f64)
    }
}

impl Decodable for OrchestreSong {
    type Decoder = OrchestreDecoder;

    fn decoder(&self) -> Self::Decoder {
        OrchestreDecoder::new(&self.song, self.looping)
    }
}

/// Streams a song by running the Orchestre engine on demand.
pub struct OrchestreDecoder {
    engine: Engine,
    left: Vec<f32>,
    right: Vec<f32>,
    /// Next frame of the current block, and which channel comes next.
    frame: usize,
    channel: usize,
    looping: bool,
    /// Frames rendered since the song itself ended (non-looping only).
    tail: usize,
    silent: usize,
    done: bool,
}

impl OrchestreDecoder {
    fn new(song: &Song, looping: bool) -> Self {
        let mut engine = Engine::new(SAMPLE_RATE as f32);
        engine.handle(Cmd::SetSong(Box::new(song.clone())));
        let range = (song.length > 0).then_some((0, song.length));
        engine.handle(Cmd::SetLoop(if looping { range } else { None }));
        engine.handle(Cmd::Play);
        OrchestreDecoder {
            engine,
            left: vec![0.0; BLOCK_FRAMES],
            right: vec![0.0; BLOCK_FRAMES],
            frame: BLOCK_FRAMES,
            channel: 0,
            looping,
            tail: 0,
            silent: 0,
            done: false,
        }
    }

    fn render_block(&mut self) {
        let was_playing = self.engine.is_playing();
        self.engine.process(&mut self.left, &mut self.right);
        self.engine.drain_events().for_each(drop);
        self.frame = 0;
        if self.looping || (was_playing && self.engine.is_playing()) {
            return;
        }
        // Past the last bar: let reverb and releases ring out, then stop.
        self.tail += BLOCK_FRAMES;
        let peak = self
            .left
            .iter()
            .chain(&self.right)
            .fold(0.0f32, |m, s| m.max(s.abs()));
        self.silent = if peak < 1e-4 {
            self.silent + BLOCK_FRAMES
        } else {
            0
        };
        let sr = SAMPLE_RATE as f32;
        if self.silent as f32 >= SILENCE_TO_STOP * sr || self.tail as f32 >= MAX_TAIL * sr {
            self.done = true;
        }
    }
}

impl Iterator for OrchestreDecoder {
    type Item = Sample;

    fn next(&mut self) -> Option<Sample> {
        if self.frame >= BLOCK_FRAMES {
            if self.done {
                return None;
            }
            self.render_block();
        }
        let s = if self.channel == 0 {
            self.left[self.frame]
        } else {
            self.right[self.frame]
        };
        self.channel ^= 1;
        if self.channel == 0 {
            self.frame += 1;
        }
        Some(s as Sample)
    }
}

impl Source for OrchestreDecoder {
    fn current_span_len(&self) -> Option<usize> {
        // Format never changes; the length is unknown (or infinite).
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(2).unwrap()
    }

    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(SAMPLE_RATE).unwrap()
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

/// Settings for loading a `.orch` file (see [`AssetServer::load_with_settings`]).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrchestreSettings {
    pub looping: bool,
}

impl Default for OrchestreSettings {
    fn default() -> Self {
        OrchestreSettings { looping: true }
    }
}

#[derive(Debug)]
pub enum OrchestreLoadError {
    Io(std::io::Error),
    Format(String),
}

impl std::fmt::Display for OrchestreLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrchestreLoadError::Io(e) => write!(f, "could not read song: {e}"),
            OrchestreLoadError::Format(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for OrchestreLoadError {}

#[derive(Default, TypePath)]
pub struct OrchestreLoader;

impl AssetLoader for OrchestreLoader {
    type Asset = OrchestreSong;
    type Settings = OrchestreSettings;
    type Error = OrchestreLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &OrchestreSettings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<OrchestreSong, OrchestreLoadError> {
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .await
            .map_err(OrchestreLoadError::Io)?;
        let project = orchestre_core::file::parse_project_bytes(&bytes)
            .map_err(OrchestreLoadError::Format)?;
        Ok(OrchestreSong::from_project(&project, settings.looping))
    }

    fn extensions(&self) -> &[&str] {
        &[orchestre_core::file::EXTENSION]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn short_song(looping: bool) -> OrchestreSong {
        let mut p = Project::demo();
        p.length_bars = 1;
        let bar = p.time_sig.bar_ticks();
        for t in &mut p.tracks {
            t.notes.retain(|n| n.end() <= bar);
        }
        OrchestreSong::from_project(&p, looping)
    }

    #[test]
    fn plays_once_then_ends() {
        let song = short_song(false);
        let d = song.decoder();
        assert_eq!(d.channels().get(), 2);
        let samples: Vec<Sample> = d.collect();
        let secs = samples.len() as f64 / 2.0 / SAMPLE_RATE as f64;
        assert!(
            secs > song.duration_secs() && secs < song.duration_secs() + MAX_TAIL as f64 + 0.1,
            "{secs}s"
        );
        assert!(samples.iter().any(|s| s.abs() > 0.05));
        assert!(samples.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
    }

    #[test]
    fn looping_never_ends() {
        let song = short_song(true);
        // Three passes of the one-bar song, still going.
        let n = (song.duration_secs() * 3.0 * SAMPLE_RATE as f64 * 2.0) as usize;
        assert_eq!(song.decoder().take(n).count(), n);
    }
}
