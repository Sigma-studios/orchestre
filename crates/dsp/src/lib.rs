//! Realtime synthesis engine for Orchestre. Runs natively on the audio
//! thread and in the browser inside an AudioWorklet.

pub mod choir;
pub mod click;
pub mod drums;
pub mod engine;
pub mod env;
pub mod epiano;
pub mod filter;
pub mod fx;
pub mod glottis;
pub mod mallets;
pub mod organ;
pub mod osc;
pub mod piano;
pub mod pluck;
pub mod render;
pub mod sfx;
pub mod song;
pub mod synth;
pub mod util;

pub use engine::Engine;
pub use render::render_song;
pub use sfx::{SfxVoice, SoundPlayer, render_sound};
pub use song::{Cmd, Event, Song, SongNote, SongTrack, TrackParams};

#[cfg(test)]
mod tests;
