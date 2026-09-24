//! Realtime synthesis engine for Orchestre. Runs natively on the audio
//! thread and in the browser inside an AudioWorklet.

pub mod click;
pub mod drums;
pub mod engine;
pub mod env;
pub mod filter;
pub mod fx;
pub mod osc;
pub mod piano;
pub mod render;
pub mod song;
pub mod synth;
pub mod util;

pub use engine::Engine;
pub use render::render_song;
pub use song::{Cmd, Event, Song, SongNote, SongTrack, TrackParams};

#[cfg(test)]
mod tests;
