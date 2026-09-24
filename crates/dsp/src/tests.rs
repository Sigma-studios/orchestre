use orchestre_core::{DrumPiece, InstrumentChoice, Note, PPQ, Project, SynthPreset};

use crate::{Cmd, Engine, Event, Song, render_song};

const SR: u32 = 44100;

fn peak(buf: &[f32]) -> f32 {
    buf.iter().fold(0.0f32, |m, s| m.max(s.abs()))
}

fn check_clean(buf: &[f32]) {
    assert!(buf.iter().all(|s| s.is_finite()), "non-finite sample");
    assert!(peak(buf) <= 1.0, "output exceeds full scale");
}

fn single_note(choice: InstrumentChoice, pitch: u8) -> Project {
    let mut p = Project {
        length_bars: 1,
        ..Project::default()
    };
    let t = p.add_track(choice);
    let id = p.new_id();
    p.track_mut(t).unwrap().notes.push(Note {
        id,
        start: 0,
        len: PPQ,
        pitch,
        vel: 0.9,
    });
    p.track_mut(t).unwrap().fx.reverb = 0.0;
    p
}

#[test]
fn every_instrument_makes_sound_and_stops() {
    let mut choices = InstrumentChoice::all();
    choices.retain(|c| *c != InstrumentChoice::Drums);
    for choice in choices {
        let song = Song::from_project(&single_note(choice, 60));
        let out = render_song(&song, SR, 8.0);
        check_clean(&out);
        assert!(peak(&out) > 0.05, "{choice:?} is silent");
        // Rendering stops once the tail dies out, well before the cap.
        let secs = out.len() as f32 / 2.0 / SR as f32;
        assert!(secs < 8.0, "{choice:?} never decays ({secs}s)");
        let tail = &out[out.len() - 2000..];
        assert!(peak(tail) < 1e-3, "{choice:?} tail not silent");
    }
}

#[test]
fn every_drum_piece_makes_sound() {
    for (i, piece) in DrumPiece::ALL.iter().enumerate() {
        let song = Song::from_project(&single_note(InstrumentChoice::Drums, i as u8));
        let out = render_song(&song, SR, 4.0);
        check_clean(&out);
        assert!(peak(&out) > 0.05, "{piece:?} is silent: {}", peak(&out));
    }
}

#[test]
fn demo_song_renders_deterministically() {
    let song = Song::from_project(&Project::demo());
    let a = render_song(&song, SR, 3.0);
    let b = render_song(&song, SR, 3.0);
    check_clean(&a);
    assert_eq!(a, b);
    // 8 bars of 4/4 at 110 bpm = 17.45s, plus some tail.
    let secs = a.len() as f32 / 2.0 / SR as f32;
    assert!((17.4..21.0).contains(&secs), "unexpected length {secs}");
}

#[test]
fn muted_tracks_are_silent() {
    let mut p = single_note(InstrumentChoice::Synth(SynthPreset::Lead), 60);
    p.tracks[0].mute = true;
    let out = render_song(&Song::from_project(&p), SR, 1.0);
    assert!(peak(&out) < 1e-6);
}

#[test]
fn live_notes_and_positions() {
    let mut p = single_note(InstrumentChoice::Piano, 60);
    p.tracks[0].notes.clear();
    let track = p.tracks[0].id;
    let mut e = Engine::new(SR as f32);
    e.handle(Cmd::SetSong(Box::new(Song::from_project(&p))));
    e.handle(Cmd::LiveNoteOn {
        track,
        pitch: 64,
        vel: 0.8,
    });
    let mut l = vec![0.0; 4096];
    let mut r = vec![0.0; 4096];
    e.process(&mut l, &mut r);
    assert!(peak(&l) > 0.01);

    e.handle(Cmd::SetLoop(Some((0, PPQ))));
    e.handle(Cmd::Play);
    for _ in 0..40 {
        e.process(&mut l, &mut r);
    }
    let positions: Vec<f64> = e
        .drain_events()
        .filter_map(|ev| match ev {
            Event::Position { tick, .. } => Some(tick),
            _ => None,
        })
        .collect();
    assert!(!positions.is_empty());
    assert!(
        positions.iter().all(|&t| (0.0..PPQ as f64).contains(&t)),
        "loop not respected"
    );
    assert!(e.is_playing());
}

#[test]
fn mono_synth_glides_without_stuck_notes() {
    let mut p = single_note(InstrumentChoice::Synth(SynthPreset::Bass), 40);
    let id = p.new_id();
    // Overlapping notes: legato in mono mode.
    p.tracks[0].notes.push(Note {
        id,
        start: PPQ / 2,
        len: PPQ,
        pitch: 43,
        vel: 0.9,
    });
    let out = render_song(&Song::from_project(&p), SR, 4.0);
    check_clean(&out);
    assert!(peak(&out[out.len() - 2000..]) < 1e-3);
}

fn play_from(p: &Project, tick: i64) -> Vec<f32> {
    let mut e = Engine::new(SR as f32);
    e.handle(Cmd::SetSong(Box::new(Song::from_project(p))));
    e.handle(Cmd::Seek(tick));
    e.handle(Cmd::Play);
    let (mut l, mut r) = (vec![0.0; 8192], vec![0.0; 8192]);
    e.process(&mut l, &mut r);
    l
}

#[test]
fn starting_mid_note_plays_the_rest() {
    let mut p = single_note(InstrumentChoice::Synth(SynthPreset::Pad), 60);
    p.tracks[0].notes[0].len = 4 * PPQ;
    assert!(
        peak(&play_from(&p, 2 * PPQ)) > 0.01,
        "note in progress was not chased"
    );
    // After the note has ended there is nothing to chase.
    assert!(peak(&play_from(&p, 4 * PPQ + 10)) < 1e-6);
}

#[test]
fn drum_hits_are_not_chased() {
    let mut p = single_note(InstrumentChoice::Drums, 0);
    p.tracks[0].notes[0].len = 2 * PPQ;
    assert!(peak(&play_from(&p, PPQ)) < 1e-6);
}

#[test]
fn loop_restart_plays_notes_on_the_first_beat() {
    // A kick on beat 1 of a one-bar loop must sound on every repeat.
    let p = single_note(InstrumentChoice::Drums, 0);
    let bar = p.time_sig.bar_ticks();
    let mut e = Engine::new(SR as f32);
    e.handle(Cmd::SetSong(Box::new(Song::from_project(&p))));
    e.handle(Cmd::SetLoop(Some((0, bar))));
    e.handle(Cmd::Play);
    let loop_len =
        (orchestre_core::ticks_to_seconds(bar as f64, p.bpm as f64) * SR as f64) as usize;
    let total = loop_len * 4;
    let (mut l, mut r) = (vec![0.0; total], vec![0.0; total]);
    e.process(&mut l, &mut r);
    for k in 0..4 {
        let start = k * loop_len;
        let hit = peak(&l[start..start + 2000]);
        assert!(hit > 0.05, "no kick at the start of loop {k} (peak {hit})");
    }
}
