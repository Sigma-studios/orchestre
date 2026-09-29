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

#[test]
fn count_in_clicks_then_plays() {
    let mut p = single_note(InstrumentChoice::Piano, 60);
    p.tracks[0].notes.clear();
    let mut e = Engine::new(SR as f32);
    e.handle(Cmd::SetSong(Box::new(Song::from_project(&p))));
    e.handle(Cmd::CountIn(4));
    let beat = (orchestre_core::ticks_to_seconds(PPQ as f64, p.bpm as f64) * SR as f64) as usize;
    let (mut l, mut r) = (vec![0.0; beat], vec![0.0; beat]);
    let mut counts = Vec::new();
    for i in 0..4 {
        e.process(&mut l, &mut r);
        assert!(peak(&l) > 0.1, "no click on count-in beat {i}");
        assert!(
            !e.is_playing() || i == 3,
            "started before the count-in ended"
        );
        counts.extend(e.drain_events().filter_map(|ev| match ev {
            Event::CountIn { beats_left } => Some(beats_left),
            _ => None,
        }));
    }
    e.process(&mut l[..256], &mut r[..256]);
    assert_eq!(counts, vec![4, 3, 2, 1]);
    assert!(e.is_playing());
    assert!(
        e.position() < PPQ as f64 / 4.0,
        "playback should start at the beginning"
    );
}

#[test]
fn metronome_clicks_only_when_enabled() {
    let mut p = single_note(InstrumentChoice::Piano, 60);
    p.tracks[0].notes.clear();
    let run = |on: bool| {
        let mut e = Engine::new(SR as f32);
        e.handle(Cmd::SetSong(Box::new(Song::from_project(&p))));
        e.handle(Cmd::SetMetronome(on));
        e.handle(Cmd::Play);
        let (mut l, mut r) = (vec![0.0; 44100], vec![0.0; 44100]);
        e.process(&mut l, &mut r);
        peak(&l)
    };
    assert!(run(true) > 0.1);
    assert!(run(false) < 1e-6);
}

/// Estimate the fundamental frequency by autocorrelation.
fn pitch_of(x: &[f32], sr: f32) -> f32 {
    let corr = |lag: usize| x.iter().zip(&x[lag..]).map(|(a, b)| a * b).sum::<f32>();
    let (lo, hi) = ((sr / 1500.0) as usize, (sr / 30.0) as usize);
    let best = (lo..hi)
        .max_by(|&a, &b| corr(a).total_cmp(&corr(b)))
        .unwrap();
    let (a, b, c) = (corr(best - 1), corr(best), corr(best + 1));
    sr / (best as f32 + 0.5 * (a - c) / (a - 2.0 * b + c))
}

#[test]
fn plucked_strings_are_in_tune() {
    use orchestre_core::{PluckKind, PluckParams};
    let sr = SR as f32;
    for kind in PluckKind::ALL {
        for pitch in [40u8, 57, 69, 81] {
            let mut e = crate::pluck::PluckEngine::new(
                PluckParams {
                    body: 0.0,
                    ..kind.params()
                },
                sr,
            );
            e.note_on(pitch, 0.9, f64::INFINITY);
            let mut out = Vec::new();
            let (mut l, mut r) = ([0.0f32; 32], [0.0f32; 32]);
            for _ in 0..600 {
                l.fill(0.0);
                r.fill(0.0);
                e.render(&mut l, &mut r);
                out.extend_from_slice(&l);
            }
            let f = pitch_of(&out[4096..4096 + 8192], sr);
            let want = crate::util::midi_to_hz(pitch as f32);
            let cents = 1200.0 * (f / want).log2();
            assert!(
                cents.abs() < 3.0,
                "{kind:?} note {pitch}: {f} Hz is {cents:+.1} cents off"
            );
        }
    }
}

#[test]
fn muted_strings_stop_quickly() {
    use orchestre_core::PluckKind;
    let mut e = crate::pluck::PluckEngine::new(PluckKind::Guitar.params(), SR as f32);
    e.note_on(60, 0.9, f64::INFINITY);
    let (mut l, mut r) = ([0.0f32; 32], [0.0f32; 32]);
    for _ in 0..100 {
        e.render(&mut l, &mut r);
    }
    e.live_off(60);
    // 0.3 s after the release it should be silent.
    for _ in 0..(SR as usize * 3 / 10 / 32) {
        l.fill(0.0);
        r.fill(0.0);
        e.render(&mut l, &mut r);
    }
    assert!(
        peak(&l) < 1e-3,
        "string still ringing after mute: {}",
        peak(&l)
    );
}

#[test]
fn engine_plays_sound_effects_over_the_song() {
    use orchestre_core::sfx::{PlayOpts, presets};
    let mut e = Engine::new(SR as f32);
    let sound = presets::find("Coin").unwrap().sound();
    let (mut l, mut r) = (vec![0.0f32; 512], vec![0.0f32; 512]);
    e.process(&mut l, &mut r);
    assert!(peak(&l) < 1e-6, "silent before");
    for seed in 0..40 {
        // More than the engine holds: the oldest are cut, nothing breaks.
        e.handle(Cmd::PlaySound {
            sound: Box::new(sound.clone()),
            opts: PlayOpts::seeded(seed),
        });
    }
    e.process(&mut l, &mut r);
    check_clean(&l);
    assert!(peak(&l) > 0.05, "plays");
    e.handle(Cmd::StopSounds);
    for _ in 0..4 {
        e.process(&mut l, &mut r);
    }
    // Only the reverb tail can remain.
    assert!(peak(&l) < 0.05, "stopped: {}", peak(&l));
}

#[test]
fn engine_releases_looping_sounds() {
    use orchestre_core::sfx::{PlayOpts, presets};
    let mut e = Engine::new(SR as f32);
    let (mut l, mut r) = (vec![0.0f32; 4410], vec![0.0f32; 4410]);
    e.handle(Cmd::PlaySound {
        sound: Box::new(presets::find("Engine").unwrap().sound()),
        opts: PlayOpts::default(),
    });
    for _ in 0..30 {
        e.process(&mut l, &mut r);
    }
    assert!(peak(&l) > 0.02, "still running after 3s");
    e.handle(Cmd::SoundLive(PlayOpts {
        intensity: 1.8,
        rate: 2.0,
        ..PlayOpts::default()
    }));
    e.handle(Cmd::ReleaseSounds);
    for _ in 0..30 {
        e.process(&mut l, &mut r);
    }
    assert!(peak(&l) < 1e-3, "released: {}", peak(&l));
}

#[test]
fn an_edited_loop_carries_on_rather_than_starting_over() {
    use orchestre_core::sfx::{PlayOpts, presets};
    let rms = |b: &[f32]| (b.iter().map(|x| x * x).sum::<f32>() / b.len() as f32).sqrt();
    let mut e = Engine::new(SR as f32);
    let wind = presets::find("Wind").unwrap().sound();
    e.handle(Cmd::PlaySound {
        sound: Box::new(wind.clone()),
        opts: PlayOpts::seeded(3),
    });
    // 10 ms blocks: a second in, well past the fade-in.
    let (mut l, mut r) = (vec![0.0f32; 441], vec![0.0f32; 441]);
    let mut before = 0.0;
    for _ in 0..100 {
        e.process(&mut l, &mut r);
        before = rms(&l);
    }
    assert!(before > 0.005, "playing: {before}");

    let louder = orchestre_core::sfx::Sound {
        volume: (wind.volume * 2.0).min(4.0),
        ..wind.clone()
    };
    e.handle(Cmd::SwapSound(Box::new(louder)));
    let mut levels = Vec::new();
    for _ in 0..30 {
        e.process(&mut l, &mut r);
        check_clean(&l);
        levels.push(rms(&l));
    }
    // A restart would come back in through its fade-in: a dip to near nothing.
    let lowest = levels.iter().copied().fold(f32::MAX, f32::min);
    assert!(lowest > before * 0.5, "dipped to {lowest} from {before}");
    // And the edit is heard: twice the volume, give or take the wind's gusts.
    let after = levels[20..].iter().sum::<f32>() / 10.0;
    assert!(after > before * 1.4, "{after} vs {before}");

    // One sound is left playing, not two: a release silences everything.
    e.handle(Cmd::ReleaseSounds);
    for _ in 0..600 {
        e.process(&mut l, &mut r);
    }
    assert!(peak(&l) < 1e-3, "released: {}", peak(&l));
}

#[test]
fn sync_osc_without_resets_matches_the_free_oscillator() {
    use crate::osc::{Osc, SyncOsc, default_width};
    use orchestre_core::Wave;
    for wave in [Wave::Saw, Wave::Square, Wave::Pulse] {
        let dt = 0.0173;
        let (mut free, mut synced) = (Osc::default(), SyncOsc::default());
        let w = default_width(wave);
        let a: Vec<f32> = (0..500).map(|_| free.next(wave, dt, 0.0)).collect();
        let b: Vec<f32> = (0..500).map(|_| synced.next(wave, dt, w, None)).collect();
        for i in 1..500 {
            assert!(
                (a[i] - b[i]).abs() < 1e-3,
                "{wave:?} differs at {i}: {} vs {}",
                a[i],
                b[i]
            );
        }
    }
}

#[test]
fn ladder_stays_stable_at_full_resonance() {
    use crate::filter::Ladder;
    let mut f = Ladder::default();
    f.set(800.0, 1.0, SR as f32);
    let mut peak = 0.0f32;
    for i in 0..SR {
        let x = if i % 200 < 100 { 1.0 } else { -1.0 };
        let (a, b) = f.process(x);
        assert!(a.is_finite() && b.is_finite());
        peak = peak.max(a.abs());
    }
    assert!(peak < 8.0, "ladder blew up: {peak}");
}

#[test]
fn acid_slides_only_between_overlapping_notes() {
    let mut p = single_note(InstrumentChoice::Synth(SynthPreset::AcidBass), 40);
    let t = &mut p.tracks[0];
    let first = t.notes[0];
    // Back to back (no slide), then overlapping (slide).
    for (start, pitch) in [(PPQ, 43), (2 * PPQ - PPQ / 8, 47)] {
        t.notes.push(Note {
            id: first.id + start as u64,
            start,
            len: PPQ,
            pitch,
            vel: 1.0,
        });
    }
    let out = render_song(&Song::from_project(&p), SR, 2.0);
    check_clean(&out);
    assert!(peak(&out) > 0.05);
}

#[test]
fn arp_steps_up_through_held_notes() {
    use crate::arp::{Arp, ArpEvent};
    use orchestre_core::ArpParams;
    let p = ArpParams {
        on: true,
        octaves: 2,
        ..ArpParams::default()
    };
    let mut arp = Arp::default();
    for pitch in [64, 60, 67] {
        arp.note_on(pitch, 0.8, f64::INFINITY);
    }
    let step = PPQ as f64 / 4.0;
    let mut played = Vec::new();
    let mut offs = 0;
    // Two beats in blocks of 10 ticks.
    for b in 0..(2 * PPQ / 10) {
        let from = (b * 10) as f64;
        arp.process(&p, from, from + 10.0, step, &mut |e| match e {
            ArpEvent::On { pitch, .. } => played.push(pitch),
            ArpEvent::Off { .. } => offs += 1,
        });
    }
    assert_eq!(played, [60, 64, 67, 72, 76, 79, 60, 64]);
    assert!(offs >= 7);
    arp.all_off();
    arp.process(&p, 1000.0, 1010.0, step, &mut |_| {});
    assert!(arp.is_idle());
}

#[test]
fn volume_automation_silences_a_track() {
    use orchestre_core::{AutoLane, AutoPoint, AutoTarget};
    let mut p = single_note(InstrumentChoice::Synth(SynthPreset::Lead), 60);
    p.tracks[0].automation.push(AutoLane {
        target: AutoTarget::Volume,
        points: vec![AutoPoint {
            tick: 0,
            value: 0.0,
        }],
    });
    let out = render_song(&Song::from_project(&p), SR, 1.0);
    assert!(peak(&out) < 1e-6);
}

#[test]
fn pump_ducks_on_the_beat() {
    let mut p = single_note(InstrumentChoice::Synth(SynthPreset::Pad), 60);
    p.tracks[0].notes[0].len = 4 * PPQ;
    let rms = |p: &Project, from: f32, to: f32| {
        let out = render_song(&Song::from_project(p), SR, 0.0);
        let (a, b) = (
            (from * SR as f32) as usize * 2,
            (to * SR as f32) as usize * 2,
        );
        (out[a..b].iter().map(|s| s * s).sum::<f32>() / (b - a) as f32).sqrt()
    };
    // 110 bpm: beat 3 starts at 1.09s; just after it vs. late in the beat.
    let beat = 60.0 / 110.0;
    let plain = rms(&p, 2.0 * beat + 0.03, 2.0 * beat + 0.08);
    p.tracks[0].fx.pump = 1.0;
    let ducked = rms(&p, 2.0 * beat + 0.03, 2.0 * beat + 0.08);
    let late = rms(&p, 2.0 * beat + 0.45, 2.0 * beat + 0.5);
    assert!(ducked < plain * 0.5, "not ducked: {ducked} vs {plain}");
    assert!(late > ducked * 2.0, "did not recover: {late} vs {ducked}");
}

/// Energy of `x` (mono) at `freq`, by the Goertzel algorithm.
fn energy_at(x: &[f32], freq: f32) -> f32 {
    let w = 2.0 * std::f32::consts::PI * freq / SR as f32;
    let c = 2.0 * w.cos();
    let (mut s1, mut s2) = (0.0f32, 0.0f32);
    for &v in x {
        let s0 = v + c * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    (s1 * s1 + s2 * s2 - c * s1 * s2) / x.len() as f32
}

#[test]
fn talk_box_vowels_shape_the_spectrum() {
    use crate::talkbox::TalkBoxEngine;
    use orchestre_core::{TalkBoxPreset, Vowel};
    for vocoder in [false, true] {
        // Energy near "ee"'s high second formant, relative to "oo"'s low one.
        let ratio = |vowel: Vowel| {
            let mut p = TalkBoxPreset::TalkBox.params();
            p.vocoder = vocoder;
            p.vowels = [vowel; 8];
            p.vibrato = 0.0;
            let mut e = TalkBoxEngine::new(p, SR as f32);
            // 110 Hz: harmonics every 110 Hz, so both bands have partials.
            e.note_on(45, 0.9, f64::INFINITY);
            let mut mono = Vec::new();
            for _ in 0..600 {
                let (mut l, mut r) = ([0.0f32; 32], [0.0f32; 32]);
                e.render(&mut l, &mut r);
                mono.extend_from_slice(&l);
            }
            let tail = &mono[mono.len() / 2..];
            let high: f32 = [2200.0, 2310.0, 2420.0]
                .iter()
                .map(|&f| energy_at(tail, f))
                .sum();
            let low: f32 = [770.0, 880.0, 990.0]
                .iter()
                .map(|&f| energy_at(tail, f))
                .sum();
            high / low
        };
        let (ee, oo) = (ratio(Vowel::Ee), ratio(Vowel::Oo));
        assert!(ee > oo * 4.0, "vocoder={vocoder}: ee {ee} vs oo {oo}");
    }
}

fn render_instrument(instrument: orchestre_core::Instrument, pitch: u8) -> Vec<f32> {
    let mut p = single_note(InstrumentChoice::Piano, pitch);
    p.tracks[0].instrument = instrument;
    render_song(&Song::from_project(&p), SR, 8.0)
}

#[test]
fn every_synth_preset_as_a_patch_sounds_and_stops() {
    use orchestre_core::Instrument;
    use orchestre_core::patch::Patch;
    for preset in SynthPreset::ALL {
        let patch = Patch::from_synth(&preset.params());
        let out = render_instrument(Instrument::Patch(Box::new(patch)), 48);
        check_clean(&out);
        assert!(peak(&out) > 0.02, "{preset:?} patch is silent");
        let secs = out.len() as f32 / 2.0 / SR as f32;
        assert!(secs < 8.0, "{preset:?} patch never stops ({secs}s)");
    }
}

#[test]
fn patch_without_envelope_still_ends_notes() {
    use orchestre_core::Instrument;
    use orchestre_core::patch::{Module, Patch};
    let mut patch = Patch {
        nodes: Vec::new(),
        cables: Vec::new(),
        macros: Vec::new(),
        mono: false,
        glide: 0.0,
        chorus: 0.0,
        gain: 0.8,
    };
    let osc = patch.add(Module::new("Oscillator").unwrap(), [0.0, 0.0]);
    let out = patch.add(Module::Output, [200.0, 0.0]);
    assert!(patch.connect(osc, 0, out, 0));
    let sound = render_instrument(Instrument::Patch(Box::new(patch)), 60);
    check_clean(&sound);
    assert!(peak(&sound) > 0.05);
    let secs = sound.len() as f32 / 2.0 / SR as f32;
    assert!(secs < 3.0, "note never ended ({secs}s)");
}

#[test]
fn editing_a_patch_live_keeps_it_playing() {
    use crate::patch::PatchEngine;
    use orchestre_core::patch::{Module, Patch};
    let mut patch = Patch::default();
    let mut e = PatchEngine::new(patch.clone(), SR as f32);
    e.note_on(60, 0.8, f64::INFINITY);
    let run = |e: &mut PatchEngine| {
        let mut p = 0.0f32;
        for _ in 0..200 {
            let (mut l, mut r) = ([0.0f32; 32], [0.0f32; 32]);
            e.render(&mut l, &mut r);
            p = p.max(peak(&l));
        }
        p
    };
    assert!(run(&mut e) > 0.02);
    // A setting change: the held note carries on.
    for n in &mut patch.nodes {
        if let Module::Filter { cutoff, .. } = &mut n.module {
            *cutoff = 500.0;
        }
    }
    e.set_params(&patch);
    assert!(run(&mut e) > 0.02, "note stopped after a settings change");
}
