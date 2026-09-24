use orchestre_core::{Id, Instrument, PPQ, Tick};

use crate::click::Click;
use crate::drums::DrumEngine;
use crate::fx::{Delay, Reverb};
use crate::piano::PianoEngine;
use crate::song::{Cmd, Event, Song, TrackParams};
use crate::synth::SynthEngine;
use crate::util::{pan_gains, soft_clip};

/// Internal processing block. Sequencer events are quantized to this.
pub const BLOCK: usize = 32;
const MAX_EVENTS: usize = 256;

enum Inst {
    Synth(Box<SynthEngine>),
    Drums(Box<DrumEngine>),
    Piano(Box<PianoEngine>),
}

macro_rules! each {
    ($self:expr, $e:ident => $body:expr) => {
        match $self {
            Inst::Synth($e) => $body,
            Inst::Drums($e) => $body,
            Inst::Piano($e) => $body,
        }
    };
}

impl Inst {
    fn new(inst: &Instrument, sr: f32) -> Inst {
        match *inst {
            Instrument::Synth(p) => Inst::Synth(Box::new(SynthEngine::new(p, sr))),
            Instrument::Drums(p) => Inst::Drums(Box::new(DrumEngine::new(p, sr))),
            Instrument::Piano(p) => Inst::Piano(Box::new(PianoEngine::new(p, sr))),
        }
    }

    /// Update parameters in place; returns false if the instrument kind changed.
    fn update(&mut self, inst: &Instrument) -> bool {
        match (self, *inst) {
            (Inst::Synth(e), Instrument::Synth(p)) => e.set_params(p),
            (Inst::Drums(e), Instrument::Drums(p)) => e.set_params(p),
            (Inst::Piano(e), Instrument::Piano(p)) => e.set_params(p),
            _ => return false,
        }
        true
    }

    fn note_on(&mut self, pitch: u8, vel: f32, off_at: f64) {
        each!(self, e => e.note_on(pitch, vel, off_at))
    }
    fn release_due(&mut self, tick: f64) {
        each!(self, e => e.release_due(tick))
    }
    fn live_off(&mut self, pitch: u8) {
        each!(self, e => e.live_off(pitch))
    }
    fn all_off(&mut self) {
        each!(self, e => e.all_off())
    }
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) {
        each!(self, e => e.render(l, r))
    }
}

struct TrackState {
    id: Id,
    inst: Inst,
    /// Index of the next note to start.
    cursor: usize,
    drive: f32,
    peak: f32,
}

pub struct Engine {
    sr: f32,
    song: Box<Song>,
    /// Parallel to `song.tracks`.
    tracks: Vec<TrackState>,
    playing: bool,
    pos: f64,
    loop_range: Option<(Tick, Tick)>,
    reverb: Reverb,
    delay: Delay,
    events: Vec<Event>,
    report_every: usize,
    since_report: usize,
    master_peak: f32,
    /// Replaced songs, handed back so the owner can free them off the audio thread.
    garbage: Option<Box<Song>>,
    metronome: bool,
    count_in: Option<CountIn>,
    click: Click,
}

/// Clicks counted before playback starts, in ticks.
struct CountIn {
    pos: f64,
    len: f64,
    beats: u32,
}

impl Engine {
    pub fn new(sample_rate: f32) -> Engine {
        Engine {
            sr: sample_rate,
            song: Box::default(),
            tracks: Vec::with_capacity(64),
            playing: false,
            pos: 0.0,
            loop_range: None,
            reverb: Reverb::new(sample_rate),
            delay: Delay::new(sample_rate),
            events: Vec::with_capacity(MAX_EVENTS),
            report_every: (sample_rate / 30.0) as usize,
            since_report: 0,
            master_peak: 0.0,
            garbage: None,
            metronome: false,
            count_in: None,
            click: Click::new(sample_rate),
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn position(&self) -> f64 {
        self.pos
    }

    pub fn take_garbage(&mut self) -> Option<Box<Song>> {
        self.garbage.take()
    }

    /// Events produced since the last call.
    pub fn drain_events(&mut self) -> std::vec::Drain<'_, Event> {
        self.events.drain(..)
    }

    fn push_event(&mut self, e: Event) {
        if self.events.len() < MAX_EVENTS {
            self.events.push(e);
        }
    }

    pub fn handle(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::SetSong(song) => self.set_song(song),
            Cmd::SetTrack(p) => self.set_track(p),
            Cmd::Play => {
                self.playing = true;
                self.seek(self.pos);
            }
            Cmd::Stop => {
                self.playing = false;
                self.count_in = None;
                self.all_off();
                self.report_position();
            }
            Cmd::Seek(t) => {
                self.seek(t.max(0) as f64);
                self.report_position();
            }
            Cmd::SetLoop(r) => self.loop_range = r.filter(|(a, b)| b > a),
            Cmd::LiveNoteOn { track, pitch, vel } => {
                if let Some(t) = self.tracks.iter_mut().find(|t| t.id == track) {
                    t.inst.note_on(pitch, vel, f64::INFINITY);
                }
            }
            Cmd::SetMetronome(on) => self.metronome = on,
            Cmd::CountIn(beats) => {
                self.playing = false;
                self.all_off();
                self.count_in = (beats > 0).then(|| CountIn {
                    pos: 0.0,
                    len: beats as f64 * self.song.beat.max(1) as f64,
                    beats,
                });
                if self.count_in.is_none() {
                    self.handle(Cmd::Play);
                }
            }
            Cmd::LiveNoteOff { track, pitch } => {
                if let Some(t) = self.tracks.iter_mut().find(|t| t.id == track) {
                    t.inst.live_off(pitch);
                }
            }
        }
    }

    fn set_song(&mut self, song: Box<Song>) {
        let mut old = std::mem::take(&mut self.tracks);
        for st in &song.tracks {
            let p = &st.params;
            let state = match old.iter().position(|t| t.id == p.id) {
                Some(i) => {
                    let mut t = old.swap_remove(i);
                    if !t.inst.update(&p.instrument) {
                        t.inst = Inst::new(&p.instrument, self.sr);
                    }
                    t
                }
                None => TrackState {
                    id: p.id,
                    inst: Inst::new(&p.instrument, self.sr),
                    cursor: 0,
                    drive: 0.0,
                    peak: 0.0,
                },
            };
            self.tracks.push(state);
        }
        self.delay.set_time(song.delay_seconds, self.sr);
        let prev = std::mem::replace(&mut self.song, song);
        self.garbage = Some(prev);
        let pos = self.pos;
        for (t, st) in self.tracks.iter_mut().zip(&self.song.tracks) {
            t.cursor = st.notes.partition_point(|n| (n.start as f64) < pos);
            t.drive = st.params.fx.drive;
        }
    }

    fn set_track(&mut self, p: TrackParams) {
        let Some(i) = self.song.tracks.iter().position(|t| t.params.id == p.id) else {
            return;
        };
        self.song.tracks[i].params = p;
        let t = &mut self.tracks[i];
        if !t.inst.update(&p.instrument) {
            t.inst = Inst::new(&p.instrument, self.sr);
        }
        t.drive = p.fx.drive;
    }

    fn all_off(&mut self) {
        for t in &mut self.tracks {
            t.inst.all_off();
        }
    }

    fn seek(&mut self, tick: f64) {
        self.pos = tick;
        self.all_off();
        let playing = self.playing;
        for (t, st) in self.tracks.iter_mut().zip(&self.song.tracks) {
            t.cursor = st.notes.partition_point(|n| (n.start as f64) < tick);
            // Chase notes already sounding at the new position, so starting
            // mid-note plays its remainder. Drum hits are one-shots: skip them.
            if playing && st.params.audible && !matches!(t.inst, Inst::Drums(_)) {
                for n in st.notes[..t.cursor].iter().filter(|n| n.end as f64 > tick) {
                    t.inst.note_on(n.pitch, n.vel, n.end as f64);
                }
            }
        }
    }

    fn report_position(&mut self) {
        self.push_event(Event::Position {
            tick: self.pos,
            playing: self.playing,
        });
    }

    fn ticks_per_sample(&self) -> f64 {
        self.song.bpm as f64 / 60.0 * PPQ as f64 / self.sr as f64
    }

    /// Start notes whose start falls in `[pos, end)`, and release those that ended.
    fn sequence(&mut self, end: f64) {
        for (t, st) in self.tracks.iter_mut().zip(&self.song.tracks) {
            while let Some(n) = st.notes.get(t.cursor) {
                if n.start as f64 >= end {
                    break;
                }
                if st.params.audible {
                    t.inst.note_on(n.pitch, n.vel, n.end as f64);
                }
                t.cursor += 1;
            }
            t.inst.release_due(end);
        }
    }

    /// Advance the transport by one block of `n` samples.
    /// Count-in clicks; starts playback when done.
    fn advance_count_in(&mut self, n: usize) {
        let dt = self.ticks_per_sample() * n as f64;
        let beat = self.song.beat.max(1) as f64;
        let Some(ci) = &mut self.count_in else { return };
        let prev = ci.pos;
        ci.pos += dt;
        let (b0, b1) = ((prev / beat).floor(), (ci.pos / beat).floor());
        let click = if prev == 0.0 {
            Some(0)
        } else if b1 > b0 && ci.pos < ci.len {
            Some(b1 as u32)
        } else {
            None
        };
        let (beats, done) = (ci.beats, ci.pos >= ci.len);
        if let Some(i) = click {
            self.click.trigger(i == 0);
            self.push_event(Event::CountIn {
                beats_left: beats - i,
            });
        }
        if done {
            self.count_in = None;
            self.handle(Cmd::Play);
            self.report_position();
        }
    }

    fn advance(&mut self, n: usize) {
        if self.count_in.is_some() {
            self.advance_count_in(n);
            return;
        }
        if !self.playing {
            return;
        }
        let end = self.pos + self.ticks_per_sample() * n as f64;
        if self.metronome {
            let beat = self.song.beat.max(1) as f64;
            let next_beat = (self.pos / beat).ceil() * beat;
            if next_beat < end {
                let bar = self.song.bar.max(1);
                self.click.trigger((next_beat as Tick) % bar == 0);
            }
        }
        // Don't trigger notes that lie past the loop end.
        let seq_end = match self.loop_range {
            Some((_, b)) => end.min(b as f64),
            None => end,
        };
        self.sequence(seq_end);
        self.pos = end;
        match self.loop_range {
            Some((a, b)) if self.pos >= b as f64 => {
                // Jump to the exact loop start (so notes *on* it aren't
                // treated as already passed), then play the overshoot.
                let over = self.pos - b as f64;
                self.seek(a as f64);
                self.sequence(a as f64 + over);
                self.pos = a as f64 + over;
            }
            None if self.pos >= self.song.length as f64 => {
                self.playing = false;
                self.seek(0.0);
                self.report_position();
            }
            _ => {}
        }
    }

    /// Render stereo audio into planar buffers of equal length.
    pub fn process(&mut self, out_l: &mut [f32], out_r: &mut [f32]) {
        let frames = out_l.len().min(out_r.len());
        let mut done = 0;
        while done < frames {
            let n = (frames - done).min(BLOCK);
            self.process_block(&mut out_l[done..done + n], &mut out_r[done..done + n]);
            done += n;
        }
    }

    /// Render interleaved audio with `channels` channels (the first two get L/R).
    pub fn process_interleaved(&mut self, out: &mut [f32], channels: usize) {
        let channels = channels.max(1);
        let mut l = [0.0f32; BLOCK];
        let mut r = [0.0f32; BLOCK];
        for chunk in out.chunks_mut(BLOCK * channels) {
            let n = chunk.len() / channels;
            self.process_block(&mut l[..n], &mut r[..n]);
            for (i, frame) in chunk.chunks_mut(channels).enumerate() {
                if channels == 1 {
                    frame[0] = (l[i] + r[i]) * 0.5;
                } else {
                    frame[0] = l[i];
                    frame[1] = r[i];
                    for s in &mut frame[2..] {
                        *s = 0.0;
                    }
                }
            }
        }
    }

    fn process_block(&mut self, out_l: &mut [f32], out_r: &mut [f32]) {
        let n = out_l.len();
        self.advance(n);

        let mut l = [0.0f32; BLOCK];
        let mut r = [0.0f32; BLOCK];
        let mut rev = [0.0f32; BLOCK];
        let mut del_l = [0.0f32; BLOCK];
        let mut del_r = [0.0f32; BLOCK];
        out_l.fill(0.0);
        out_r.fill(0.0);

        for (t, st) in self.tracks.iter_mut().zip(&self.song.tracks) {
            let p = &st.params;
            l[..n].fill(0.0);
            r[..n].fill(0.0);
            t.inst.render(&mut l[..n], &mut r[..n]);
            let (pl, pr) = pan_gains(p.pan);
            let vol = p.volume * std::f32::consts::SQRT_2;
            let drive = 1.0 + t.drive * 6.0;
            let makeup = 1.0 / (1.0 + t.drive * 1.5);
            let mut peak = 0.0f32;
            for i in 0..n {
                let (mut a, mut b) = (l[i], r[i]);
                if t.drive > 0.0 {
                    a = soft_clip(a * drive) * makeup;
                    b = soft_clip(b * drive) * makeup;
                }
                a *= vol * pl;
                b *= vol * pr;
                peak = peak.max(a.abs()).max(b.abs());
                out_l[i] += a;
                out_r[i] += b;
                rev[i] += (a + b) * p.fx.reverb;
                del_l[i] += a * p.fx.delay;
                del_r[i] += b * p.fx.delay;
            }
            t.peak = t.peak.max(peak);
        }

        for i in 0..n {
            let (dl, dr) = self.delay.process(del_l[i], del_r[i]);
            // Echoes also feed the reverb a little so they sit in the same space.
            let (rl, rr) = self.reverb.process(rev[i] + (dl + dr) * 0.15);
            let click = self.click.sample();
            let a = soft_clip((out_l[i] + dl + rl * 2.5) * 0.9 + click);
            let b = soft_clip((out_r[i] + dr + rr * 2.5) * 0.9 + click);
            self.master_peak = self.master_peak.max(a.abs()).max(b.abs());
            out_l[i] = a;
            out_r[i] = b;
        }

        self.since_report += n;
        if self.since_report >= self.report_every {
            self.since_report = 0;
            self.report_position();
            for i in 0..self.tracks.len() {
                let (id, peak) = (self.tracks[i].id, self.tracks[i].peak);
                self.tracks[i].peak = 0.0;
                self.push_event(Event::Level { track: id, peak });
            }
            let master = self.master_peak;
            self.master_peak = 0.0;
            self.push_event(Event::Level {
                track: 0,
                peak: master,
            });
        }
    }
}
