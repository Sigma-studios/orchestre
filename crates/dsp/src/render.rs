use crate::engine::Engine;
use crate::song::{Cmd, Song};

/// Render the whole song offline to interleaved stereo samples, including
/// the tail of the last notes and effects (up to `max_tail` seconds).
pub fn render_song(song: &Song, sample_rate: u32, max_tail: f32) -> Vec<f32> {
    let sr = sample_rate as f32;
    let mut engine = Engine::new(sr);
    engine.handle(Cmd::SetSong(Box::new(song.clone())));
    engine.handle(Cmd::SetLoop(None));
    engine.handle(Cmd::Seek(0));
    engine.handle(Cmd::Play);

    let seconds = orchestre_core::ticks_to_seconds(song.length as f64, song.bpm as f64);
    let body = (seconds * sample_rate as f64).ceil() as usize;
    let chunk = 1024;
    let mut out = Vec::with_capacity((body + (max_tail * sr) as usize) * 2);
    let mut l = vec![0.0f32; chunk];
    let mut r = vec![0.0f32; chunk];
    let mut rendered = 0;
    let mut quiet = 0;
    while rendered < body + (max_tail * sr) as usize {
        engine.process(&mut l, &mut r);
        let _ = engine.drain_events().count();
        for i in 0..chunk {
            out.push(l[i]);
            out.push(r[i]);
        }
        rendered += chunk;
        if rendered >= body {
            let peak = l.iter().chain(&r).fold(0.0f32, |m, s| m.max(s.abs()));
            quiet = if peak < 1e-4 { quiet + chunk } else { 0 };
            if quiet > sample_rate as usize / 4 {
                break;
            }
        }
    }
    out
}
