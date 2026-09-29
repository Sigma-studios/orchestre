use orchestre_core::{ArpParams, AutoTarget, FxParams, Id, Instrument, PPQ, PreviewNote, Tick};

use crate::arp::{Arp, ArpEvent};
use crate::choir::ChoirEngine;
use crate::click::Click;
use crate::drums::DrumEngine;
use crate::epiano::EPianoEngine;
use crate::filter::Svf;
use crate::fx::{Delay, Phaser, Reverb};
use crate::mallets::MalletEngine;
use crate::organ::OrganEngine;
use crate::patch::PatchEngine;
use crate::piano::PianoEngine;
use crate::pluck::PluckEngine;
use crate::sfx::{MAX_SOUNDS, SfxVoice};
use crate::song::{Cmd, Event, Song, TrackParams};
use crate::synth::SynthEngine;
use crate::talkbox::TalkBoxEngine;
use crate::util::{pan_gains, soft_clip};

/// Internal processing block. Sequencer events are quantized to this.
pub const BLOCK: usize = 32;
const MAX_EVENTS: usize = 256;

enum Inst {
    Synth(Box<SynthEngine>),
    Drums(Box<DrumEngine>),
    Piano(Box<PianoEngine>),
    EPiano(Box<EPianoEngine>),
    Organ(Box<OrganEngine>),
    Mallets(Box<MalletEngine>),
    Pluck(Box<PluckEngine>),
    Choir(Box<ChoirEngine>),
    TalkBox(Box<TalkBoxEngine>),
    Patch(Box<PatchEngine>),
}

macro_rules! each {
    ($self:expr, $e:ident => $body:expr) => {
        match $self {
            Inst::Synth($e) => $body,
            Inst::Drums($e) => $body,
            Inst::Piano($e) => $body,
            Inst::EPiano($e) => $body,
            Inst::Organ($e) => $body,
            Inst::Mallets($e) => $body,
            Inst::Pluck($e) => $body,
            Inst::Choir($e) => $body,
            Inst::TalkBox($e) => $body,
            Inst::Patch($e) => $body,
        }
    };
}

impl Inst {
    fn new(inst: &Instrument, sr: f32) -> Inst {
        match inst {
            Instrument::Synth(p) => Inst::Synth(Box::new(SynthEngine::new(*p, sr))),
            Instrument::Drums(p) => Inst::Drums(Box::new(DrumEngine::new(*p, sr))),
            Instrument::Piano(p) => Inst::Piano(Box::new(PianoEngine::new(*p, sr))),
            Instrument::EPiano(p) => Inst::EPiano(Box::new(EPianoEngine::new(*p, sr))),
            Instrument::Organ(p) => Inst::Organ(Box::new(OrganEngine::new(*p, sr))),
            Instrument::Mallets(p) => Inst::Mallets(Box::new(MalletEngine::new(*p, sr))),
            Instrument::Pluck(p) => Inst::Pluck(Box::new(PluckEngine::new(*p, sr))),
            Instrument::Choir(p) => Inst::Choir(Box::new(ChoirEngine::new(*p, sr))),
            Instrument::TalkBox(p) => Inst::TalkBox(Box::new(TalkBoxEngine::new(*p, sr))),
            Instrument::Patch(p) => Inst::Patch(Box::new(PatchEngine::new((**p).clone(), sr))),
        }
    }

    /// Update parameters in place; returns false if the instrument kind changed.
    fn update(&mut self, inst: &Instrument) -> bool {
        match (self, inst) {
            (Inst::Synth(e), Instrument::Synth(p)) => e.set_params(*p),
            (Inst::Drums(e), Instrument::Drums(p)) => e.set_params(*p),
            (Inst::Piano(e), Instrument::Piano(p)) => e.set_params(*p),
            (Inst::EPiano(e), Instrument::EPiano(p)) => e.set_params(*p),
            (Inst::Organ(e), Instrument::Organ(p)) => e.set_params(*p),
            (Inst::Mallets(e), Instrument::Mallets(p)) => e.set_params(*p),
            (Inst::Pluck(e), Instrument::Pluck(p)) => e.set_params(*p),
            (Inst::Choir(e), Instrument::Choir(p)) => e.set_params(*p),
            (Inst::TalkBox(e), Instrument::TalkBox(p)) => e.set_params(*p),
            (Inst::Patch(e), Instrument::Patch(p)) => e.set_params(p),
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
    fn set_tempo(&mut self, bpm: f32) {
        match self {
            Inst::Synth(e) => e.set_tempo(bpm),
            Inst::Patch(e) => e.set_tempo(bpm),
            _ => {}
        }
    }
    fn set_brightness(&mut self, octaves: f32) {
        if let Inst::Synth(e) = self {
            e.set_brightness(octaves);
        }
    }
    fn is_drums(&self) -> bool {
        matches!(self, Inst::Drums(_))
    }

    /// Advance an arpeggiator over the ticks `[from, to)`, playing its notes.
    fn run_arp(&mut self, arp: &mut Arp, p: &ArpParams, from: f64, to: f64) {
        let step = p.rate as f64 * PPQ as f64;
        arp.process(p, from, to, step, &mut |e| match e {
            ArpEvent::On { pitch, vel } => self.note_on(pitch, vel, f64::INFINITY),
            ArpEvent::Off { pitch } => self.live_off(pitch),
        });
    }
}

/// An instrument playing a short phrase outside the song (menu previews).
struct Preview {
    inst: Inst,
    arp: Arp,
    arp_params: ArpParams,
    notes: Vec<PreviewNote>,
    /// Per note: 0 = waiting, 1 = sounding, 2 = done.
    state: Vec<u8>,
    clock: f32,
    end: f32,
}

struct TrackState {
    id: Id,
    inst: Inst,
    /// Index of the next note to start.
    cursor: usize,
    peak: f32,
    arp: Arp,
    /// Whether notes go through the arpeggiator.
    arp_on: bool,
    /// DJ filter, one per channel.
    dj: [Svf; 2],
    phaser: Phaser,
}

impl TrackState {
    fn new(p: &TrackParams, sr: f32) -> TrackState {
        TrackState {
            id: p.id,
            inst: Inst::new(&p.instrument, sr),
            cursor: 0,
            peak: 0.0,
            arp: Arp::default(),
            arp_on: false,
            dj: [Svf::default(); 2],
            phaser: Phaser::new(sr),
        }
    }

    fn note_on(&mut self, pitch: u8, vel: f32, off_at: f64) {
        if self.arp_on {
            self.arp.note_on(pitch, vel, off_at);
        } else {
            self.inst.note_on(pitch, vel, off_at);
        }
    }

    fn release_due(&mut self, tick: f64) {
        self.arp.release_due(tick);
        self.inst.release_due(tick);
    }

    fn live_off(&mut self, pitch: u8) {
        self.arp.live_off(pitch);
        self.inst.live_off(pitch);
    }

    fn all_off(&mut self) {
        self.arp.all_off();
        self.inst.all_off();
    }

    /// Follow the track's settings; returns true if the instrument was rebuilt.
    fn apply(&mut self, p: &TrackParams, sr: f32) -> bool {
        let rebuilt = !self.inst.update(&p.instrument);
        if rebuilt {
            self.inst = Inst::new(&p.instrument, sr);
        }
        let arp_on = p.arp.on && !self.inst.is_drums();
        if arp_on != self.arp_on {
            self.all_off();
            self.arp_on = arp_on;
        }
        rebuilt
    }
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
    preview: Option<Preview>,
    /// Sound effects playing, oldest first.
    sounds: Vec<SfxVoice>,
    /// Ticks at the tempo, running even when stopped (live arpeggios).
    free_clock: f64,
}

/// Clicks counted before playback starts, in ticks.
struct CountIn {
    pos: f64,
    len: f64,
    beats: u32,
}

impl Engine {
    pub fn new(sample_rate: f32) -> Engine {
        // Voice tables take a few milliseconds: build them now, not mid-sound.
        crate::glottis::tables();
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
            preview: None,
            sounds: Vec::with_capacity(MAX_SOUNDS),
            free_clock: 0.0,
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
                    t.note_on(pitch, vel, f64::INFINITY);
                }
            }
            Cmd::SetMetronome(on) => self.metronome = on,
            Cmd::Preview {
                instrument,
                notes,
                arp,
            } => self.start_preview(instrument, notes, arp),
            Cmd::StopPreview => {
                if let Some(pv) = &mut self.preview {
                    pv.arp.all_off();
                    pv.inst.all_off();
                    pv.state.iter_mut().for_each(|s| *s = 2);
                    pv.end = pv.clock + 2.0;
                }
            }
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
            Cmd::PlaySound { sound, opts } => {
                if self.sounds.len() >= MAX_SOUNDS {
                    self.sounds.remove(0);
                }
                let voice = SfxVoice::new(&sound, &opts, self.sr);
                self.sounds.push(voice);
            }
            Cmd::StopSounds => self.sounds.iter_mut().for_each(SfxVoice::stop),
            Cmd::ReleaseSounds => self.sounds.iter_mut().for_each(SfxVoice::release),
            Cmd::SoundLive(o) => self.sounds.iter_mut().for_each(|s| s.set_live(&o)),
            Cmd::SwapSound(sound) => {
                let mut fresh = Vec::new();
                for voice in self.sounds.iter_mut().filter(|v| v.swappable()) {
                    fresh.push(voice.swapped(&sound));
                    voice.cross_fade_out();
                }
                for voice in fresh {
                    if self.sounds.len() >= MAX_SOUNDS {
                        self.sounds.remove(0);
                    }
                    self.sounds.push(voice);
                }
            }
            Cmd::LiveNoteOff { track, pitch } => {
                if let Some(t) = self.tracks.iter_mut().find(|t| t.id == track) {
                    t.live_off(pitch);
                }
            }
        }
    }

    fn set_song(&mut self, song: Box<Song>) {
        let mut old = std::mem::take(&mut self.tracks);
        for st in &song.tracks {
            let p = &st.params;
            let mut state = match old.iter().position(|t| t.id == p.id) {
                Some(i) => old.swap_remove(i),
                None => TrackState::new(p, self.sr),
            };
            state.apply(p, self.sr);
            self.tracks.push(state);
        }
        self.delay.set_time(song.delay_seconds, self.sr);
        for t in &mut self.tracks {
            t.inst.set_tempo(song.bpm);
        }
        let prev = std::mem::replace(&mut self.song, song);
        self.garbage = Some(prev);
        let pos = self.pos;
        for (t, st) in self.tracks.iter_mut().zip(&self.song.tracks) {
            t.cursor = st.notes.partition_point(|n| (n.start as f64) < pos);
        }
    }

    fn start_preview(&mut self, instrument: Instrument, notes: Vec<PreviewNote>, arp: ArpParams) {
        let end = notes.iter().map(|n| n.start + n.len).fold(0.0, f32::max) + 3.0;
        // Reuse the previous preview's instrument if it's the same kind.
        let inst = match self.preview.take().map(|pv| pv.inst) {
            Some(mut inst) => {
                if inst.update(&instrument) {
                    inst.all_off();
                    inst
                } else {
                    Inst::new(&instrument, self.sr)
                }
            }
            None => Inst::new(&instrument, self.sr),
        };
        let state = vec![0; notes.len()];
        self.preview = Some(Preview {
            inst,
            arp: Arp::default(),
            arp_params: arp,
            notes,
            state,
            clock: 0.0,
            end,
        });
    }

    /// Advance and render the preview phrase, mixed into the given buffers.
    fn render_preview(&mut self, l: &mut [f32], r: &mut [f32], rev: &mut [f32]) {
        let Some(pv) = &mut self.preview else { return };
        let arp = pv.arp_params.on && !pv.inst.is_drums();
        for (n, st) in pv.notes.iter().zip(pv.state.iter_mut()) {
            if *st == 0 && n.start <= pv.clock {
                if arp {
                    pv.arp.note_on(n.pitch, n.vel, f64::INFINITY);
                } else {
                    pv.inst.note_on(n.pitch, n.vel, f64::INFINITY);
                }
                *st = 1;
            }
            if *st == 1 && n.start + n.len <= pv.clock {
                pv.arp.live_off(n.pitch);
                pv.inst.live_off(n.pitch);
                *st = 2;
            }
        }
        let len = l.len();
        if arp || !pv.arp.is_idle() {
            // Previews run at 120 bpm: two beats a second.
            let ticks = |secs: f32| secs as f64 * 2.0 * PPQ as f64;
            let (from, to) = (ticks(pv.clock), ticks(pv.clock + len as f32 / self.sr));
            pv.inst.run_arp(&mut pv.arp, &pv.arp_params, from, to);
        }
        let mut pl = [0.0f32; BLOCK];
        let mut pr = [0.0f32; BLOCK];
        pv.inst.render(&mut pl[..len], &mut pr[..len]);
        for i in 0..len {
            l[i] += pl[i] * 0.9;
            r[i] += pr[i] * 0.9;
            rev[i] += (pl[i] + pr[i]) * 0.12;
        }
        pv.clock += len as f32 / self.sr;
        if pv.clock > pv.end {
            self.preview = None;
        }
    }

    fn set_track(&mut self, p: TrackParams) {
        let Some(i) = self.song.tracks.iter().position(|t| t.params.id == p.id) else {
            return;
        };
        let t = &mut self.tracks[i];
        if t.apply(&p, self.sr) {
            t.inst.set_tempo(self.song.bpm);
        }
        self.song.tracks[i].params = p;
    }

    fn all_off(&mut self) {
        for t in &mut self.tracks {
            t.all_off();
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
            if playing && st.params.audible && !t.inst.is_drums() {
                for n in st.notes[..t.cursor].iter().filter(|n| n.end as f64 > tick) {
                    t.note_on(n.pitch, n.vel, n.end as f64);
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
                    // Acid-style slides: only notes that overlap glide, so
                    // end the ones that stop where this one starts first.
                    if let Inst::Synth(e) = &mut t.inst
                        && e.params.slide_only
                    {
                        e.release_due(n.start as f64);
                    }
                    t.note_on(n.pitch, n.vel, n.end as f64);
                }
                t.cursor += 1;
            }
            t.release_due(end);
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
        let dt = self.ticks_per_sample() * n as f64;
        let before = self.pos;
        self.advance(n);
        // The ticks this block covers, for arpeggiators and the pump: song
        // time while playing, a free-running clock otherwise.
        let (from, to) = if self.playing {
            if self.pos >= before {
                (before, self.pos)
            } else {
                (self.pos - dt, self.pos)
            }
        } else {
            (self.free_clock, self.free_clock + dt)
        };
        self.free_clock += dt;
        let pos = self.pos;
        let beat = self.song.beat.max(1) as f64;
        let playing = self.playing;
        let sr = self.sr;

        let mut l = [0.0f32; BLOCK];
        let mut r = [0.0f32; BLOCK];
        let mut rev = [0.0f32; BLOCK];
        let mut del_l = [0.0f32; BLOCK];
        let mut del_r = [0.0f32; BLOCK];
        out_l.fill(0.0);
        out_r.fill(0.0);

        for (t, st) in self.tracks.iter_mut().zip(&self.song.tracks) {
            let p = &st.params;
            let (mut fx, mut volume, mut pan, mut bright) = (p.fx, p.volume, p.pan, 0.0);
            for lane in &p.automation {
                if let Some(v) = lane.value_at(pos) {
                    automate(lane.target, v, &mut fx, &mut volume, &mut pan, &mut bright);
                }
            }
            t.inst.set_brightness(bright * 4.0);
            if t.arp_on || !t.arp.is_idle() {
                t.inst.run_arp(&mut t.arp, &p.arp, from, to);
            }
            l[..n].fill(0.0);
            r[..n].fill(0.0);
            t.inst.render(&mut l[..n], &mut r[..n]);
            let (l, r) = (&mut l[..n], &mut r[..n]);

            if fx.drive > 0.0 {
                let drive = 1.0 + fx.drive * 6.0;
                let makeup = 1.0 / (1.0 + fx.drive * 1.5);
                for x in l.iter_mut().chain(r.iter_mut()) {
                    *x = soft_clip(*x * drive) * makeup;
                }
            }
            dj_filter(&mut t.dj, fx.filter, sr, l, r);
            if fx.phaser > 0.0 {
                t.phaser.process(l, r, fx.phaser);
            }
            if fx.pump > 0.0 && playing {
                for i in 0..n {
                    let tick = from + (to - from) * i as f64 / n as f64;
                    let g = 1.0 - fx.pump * duck((tick / beat).fract() as f32);
                    l[i] *= g;
                    r[i] *= g;
                }
            }

            let (pl, pr) = pan_gains(pan);
            let vol = volume * std::f32::consts::SQRT_2;
            let mut peak = 0.0f32;
            for i in 0..n {
                let a = l[i] * vol * pl;
                let b = r[i] * vol * pr;
                peak = peak.max(a.abs()).max(b.abs());
                out_l[i] += a;
                out_r[i] += b;
                rev[i] += (a + b) * fx.reverb;
                del_l[i] += a * fx.delay;
                del_r[i] += b * fx.delay;
            }
            t.peak = t.peak.max(peak);
        }

        self.render_preview(&mut out_l[..n], &mut out_r[..n], &mut rev[..n]);
        for s in &mut self.sounds {
            s.render(&mut out_l[..n], &mut out_r[..n], &mut rev[..n]);
        }
        self.sounds.retain(|s| !s.is_done());

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

/// Apply an automation lane's value to the track's effective settings.
fn automate(
    target: AutoTarget,
    v: f32,
    fx: &mut FxParams,
    volume: &mut f32,
    pan: &mut f32,
    bright: &mut f32,
) {
    match target {
        AutoTarget::Volume => *volume = v,
        AutoTarget::Pan => *pan = v,
        AutoTarget::Filter => fx.filter = v,
        AutoTarget::Brightness => *bright = v,
        AutoTarget::Reverb => fx.reverb = v,
        AutoTarget::Echo => fx.delay = v,
        AutoTarget::Drive => fx.drive = v,
        AutoTarget::Phaser => fx.phaser = v,
        AutoTarget::Pump => fx.pump = v,
    }
}

/// DJ-style filter: `amount` below 0 sweeps a low-pass down from 20 kHz to
/// 100 Hz, above 0 a high-pass up from 20 Hz to 8 kHz.
fn dj_filter(svf: &mut [Svf; 2], amount: f32, sr: f32, l: &mut [f32], r: &mut [f32]) {
    let a = amount.clamp(-1.0, 1.0);
    if a.abs() < 0.01 {
        svf.iter_mut().for_each(Svf::reset);
        return;
    }
    let cutoff = if a < 0.0 {
        20000.0 * (100.0f32 / 20000.0).powf(-a)
    } else {
        20.0 * (8000.0f32 / 20.0).powf(a)
    };
    for (f, buf) in svf.iter_mut().zip([l, r]) {
        f.set(cutoff, 0.35, sr);
        for x in buf.iter_mut() {
            *x = if a < 0.0 { f.low(*x) } else { f.high(*x) };
        }
    }
}

/// Sidechain-style ducking over a beat (`phase` 0..1): a fast dip on the
/// beat that recovers over about half the beat.
fn duck(phase: f32) -> f32 {
    const DIP: f32 = 0.02;
    if phase < DIP {
        phase / DIP
    } else {
        (-(phase - DIP) * 7.0).exp()
    }
}
