//! Prints peak/RMS per instrument, for balancing preset loudness.
use orchestre_core::*;
use orchestre_dsp::{Song, render_song};

fn main() {
    let mut choices: Vec<(String, InstrumentChoice, u8)> = InstrumentChoice::all()
        .into_iter()
        .filter(|c| *c != InstrumentChoice::Drums)
        .map(|c| {
            (
                c.label().to_string(),
                c,
                if matches!(
                    c,
                    InstrumentChoice::Synth(SynthPreset::Bass | SynthPreset::SubBass)
                ) {
                    40
                } else {
                    60
                },
            )
        })
        .collect();
    for (i, d) in DrumPiece::ALL.iter().enumerate() {
        choices.push((d.label().to_string(), InstrumentChoice::Drums, i as u8));
    }
    for (name, c, pitch) in choices {
        let mut p = Project {
            length_bars: 1,
            ..Project::default()
        };
        let t = p.add_track(c);
        let id = p.new_id();
        let tr = p.track_mut(t).unwrap();
        tr.fx.reverb = 0.0;
        tr.notes.push(Note {
            id,
            start: 0,
            len: PPQ,
            pitch,
            vel: 0.8,
        });
        let out = render_song(&Song::from_project(&p), 44100, 6.0);
        let n = (44100 * 2 / 2).min(out.len());
        let peak = out.iter().fold(0f32, |m, s| m.max(s.abs()));
        let rms = (out[..n].iter().map(|s| s * s).sum::<f32>() / n as f32).sqrt();
        println!(
            "{name:>12}: peak {peak:.3} rms(0.5s) {rms:.3} len {:.2}s",
            out.len() as f32 / 88200.0
        );
    }
}
