use orchestre_core::SynthParams;

use crate::env::Env;
use crate::filter::Svf;
use crate::fx::Chorus;
use crate::osc::Osc;
use crate::util::{Rng, TAU, midi_to_hz, pan_gains};

pub const MAX_VOICES: usize = 16;
const MAX_UNISON: usize = 5;
const MONO_STACK: usize = 8;

#[derive(Clone, Copy, Default)]
struct Voice {
    active: bool,
    /// Pitch the voice was triggered with (for live note-off).
    note: u8,
    /// Current (gliding) and target pitch, in semitones.
    pitch: f32,
    target: f32,
    vel: f32,
    /// Tick at which the note is released; infinite for live notes.
    off_at: f64,
    age: u64,
    osc1: [Osc; MAX_UNISON],
    osc2: [Osc; MAX_UNISON],
    sub: Osc,
    amp: Env,
    filt: Env,
    svf: [Svf; 2],
    lfo_phase: f32,
    rng: Rng,
    /// Remaining pitch bend in semitones (falls to 0).
    bend: f32,
}

impl Voice {
    #[allow(clippy::too_many_arguments)]
    fn start(
        &mut self,
        p: &SynthParams,
        sr: f32,
        pitch: u8,
        vel: f32,
        off_at: f64,
        age: u64,
        seed: u32,
    ) {
        self.active = true;
        self.note = pitch;
        self.pitch = pitch as f32;
        self.bend = p.bend;
        self.target = pitch as f32;
        self.vel = vel;
        self.off_at = off_at;
        self.age = age;
        self.amp.level = 0.0;
        self.filt.level = 0.0;
        self.amp.set(&p.amp, sr);
        self.filt.set(&p.filter_adsr, sr);
        self.amp.gate_on();
        self.filt.gate_on();
        for s in &mut self.svf {
            s.reset();
        }
        // Random start phases keep unison stacks from sounding phasey.
        let mut rng = Rng::new(seed);
        for o in self.osc1.iter_mut().chain(self.osc2.iter_mut()) {
            o.phase = rng.noise() * 0.5 + 0.5;
        }
        self.rng = rng;
    }

    fn render(&mut self, p: &SynthParams, sr: f32, l: &mut [f32], r: &mut [f32]) {
        let n = l.len();
        if p.mono && p.glide > 0.0 {
            let k = (-(n as f32) / (p.glide * sr)).exp();
            self.pitch = self.target + (self.pitch - self.target) * k;
        } else {
            self.pitch = self.target;
        }
        let lfo = (self.lfo_phase * TAU).sin();
        self.lfo_phase = (self.lfo_phase + p.lfo_rate * n as f32 / sr).fract();

        let pitch = self.pitch + self.bend + p.octave as f32 * 12.0 + lfo * p.lfo_pitch;
        self.bend = if p.bend_time > 0.0 {
            self.bend * (-(n as f32) / (p.bend_time * sr)).exp()
        } else {
            0.0
        };
        let f1 = midi_to_hz(pitch);
        let f2 = midi_to_hz(pitch + p.osc2_semitones as f32 + p.osc2_detune / 100.0);

        let uni = (p.unison as usize).clamp(1, MAX_UNISON);
        let mut dt1 = [0.0f32; MAX_UNISON];
        let mut dt2 = [0.0f32; MAX_UNISON];
        let mut gl = [0.0f32; MAX_UNISON];
        let mut gr = [0.0f32; MAX_UNISON];
        let norm = 1.0 / (uni as f32).sqrt();
        for i in 0..uni {
            let pos = if uni == 1 {
                0.0
            } else {
                i as f32 / (uni - 1) as f32 * 2.0 - 1.0
            };
            let ratio = (2.0f32).powf(pos * p.unison_spread * 0.5 / 1200.0);
            dt1[i] = f1 * ratio / sr;
            dt2[i] = f2 * ratio / sr;
            let (a, b) = pan_gains(pos * p.width);
            gl[i] = a * norm;
            gr[i] = b * norm;
        }
        let dt_sub = f1 * 0.5 / sr;

        // Filter cutoff is updated once per block from the envelope and LFO.
        let fenv = self.filt.level;
        for _ in 0..n {
            self.filt.next();
        }
        let key_track = (self.pitch - 60.0) / 12.0 * 0.4;
        let vel_bright = (self.vel - 0.7) * 1.2;
        let octaves = p.filter_env * fenv + lfo * p.lfo_cutoff + key_track + vel_bright;
        let cutoff = p.cutoff * (2.0f32).powf(octaves);
        for s in &mut self.svf {
            s.set(cutoff, p.resonance, sr);
        }

        let mix = p.osc_mix.clamp(0.0, 1.0);
        let vel_gain = 0.25 + 0.75 * self.vel * self.vel;
        let gain = p.gain * vel_gain * 0.7;
        let fm = p.fm;
        for i in 0..n {
            let mut sl = 0.0;
            let mut sr_ = 0.0;
            for u in 0..uni {
                let o2 = self.osc2[u].next(p.osc2, dt2[u], 0.0);
                let o1 = self.osc1[u].next(p.osc1, dt1[u], o2 * fm * 0.25);
                let s = o1 * (1.0 - mix) + o2 * mix;
                sl += s * gl[u];
                sr_ += s * gr[u];
            }
            if p.sub > 0.0 {
                let s = self.sub.next(orchestre_core::Wave::Square, dt_sub, 0.0) * p.sub * 0.7;
                sl += s;
                sr_ += s;
            }
            if p.noise > 0.0 {
                let s = self.rng.noise() * p.noise * 0.5;
                sl += s;
                sr_ += s;
            }
            let a = self.amp.next() * gain;
            l[i] += self.svf[0].low(sl) * a;
            r[i] += self.svf[1].low(sr_) * a;
        }
        if self.amp.is_idle() {
            self.active = false;
        }
    }
}

pub struct SynthEngine {
    pub params: SynthParams,
    sr: f32,
    voices: [Voice; MAX_VOICES],
    counter: u64,
    /// Held notes in mono mode: (pitch, vel, off_at), most recent last.
    stack: [(u8, f32, f64); MONO_STACK],
    stack_len: usize,
    chorus: Chorus,
}

impl SynthEngine {
    pub fn new(params: SynthParams, sr: f32) -> Self {
        SynthEngine {
            params,
            sr,
            voices: [Voice::default(); MAX_VOICES],
            counter: 0,
            stack: [(0, 0.0, 0.0); MONO_STACK],
            stack_len: 0,
            chorus: Chorus::new(sr),
        }
    }

    pub fn set_params(&mut self, params: SynthParams) {
        if params.mono != self.params.mono {
            self.all_off();
        }
        self.params = params;
        for v in self.voices.iter_mut().filter(|v| v.active) {
            v.amp.set(&params.amp, self.sr);
            v.filt.set(&params.filter_adsr, self.sr);
        }
    }

    pub fn note_on(&mut self, pitch: u8, vel: f32, off_at: f64) {
        self.counter += 1;
        let seed = (self.counter as u32).wrapping_mul(2_654_435_761) ^ pitch as u32;
        if self.params.mono {
            if self.stack_len == MONO_STACK {
                self.stack.copy_within(1.., 0);
                self.stack_len -= 1;
            }
            self.stack[self.stack_len] = (pitch, vel, off_at);
            self.stack_len += 1;
            let v = &mut self.voices[0];
            if v.active && !v.amp.is_released() {
                // Legato: glide to the new pitch without retriggering.
                v.target = pitch as f32;
                v.note = pitch;
                v.off_at = off_at;
                v.vel = vel;
            } else {
                let glide_from = if v.active { Some(v.pitch) } else { None };
                v.start(
                    &self.params,
                    self.sr,
                    pitch,
                    vel,
                    off_at,
                    self.counter,
                    seed,
                );
                if let Some(from) = glide_from {
                    v.pitch = from;
                }
            }
            return;
        }
        let idx = self
            .voices
            .iter()
            .position(|v| !v.active)
            .or_else(|| {
                // Steal: prefer the oldest released voice, then the oldest.
                self.voices
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| (!v.amp.is_released(), v.age))
                    .map(|(i, _)| i)
            })
            .unwrap_or(0);
        self.voices[idx].start(
            &self.params,
            self.sr,
            pitch,
            vel,
            off_at,
            self.counter,
            seed,
        );
    }

    fn mono_update(&mut self) {
        let v = &mut self.voices[0];
        if self.stack_len == 0 {
            v.amp.gate_off();
            v.filt.gate_off();
        } else {
            let (pitch, vel, off_at) = self.stack[self.stack_len - 1];
            v.target = pitch as f32;
            v.note = pitch;
            v.vel = vel;
            v.off_at = off_at;
        }
    }

    fn mono_remove(&mut self, pred: impl Fn(&(u8, f32, f64)) -> bool) {
        let before = self.stack_len;
        let mut j = 0;
        for i in 0..self.stack_len {
            if !pred(&self.stack[i]) {
                self.stack[j] = self.stack[i];
                j += 1;
            }
        }
        self.stack_len = j;
        if j != before {
            self.mono_update();
        }
    }

    pub fn release_due(&mut self, tick: f64) {
        if self.params.mono {
            self.mono_remove(|e| e.2 <= tick);
            return;
        }
        for v in self
            .voices
            .iter_mut()
            .filter(|v| v.active && v.off_at <= tick)
        {
            v.amp.gate_off();
            v.filt.gate_off();
            v.off_at = f64::INFINITY;
        }
    }

    pub fn live_off(&mut self, pitch: u8) {
        if self.params.mono {
            self.mono_remove(|e| e.0 == pitch && e.2.is_infinite());
            return;
        }
        for v in self
            .voices
            .iter_mut()
            .filter(|v| v.active && v.note == pitch && v.off_at.is_infinite())
        {
            v.amp.gate_off();
            v.filt.gate_off();
        }
    }

    pub fn all_off(&mut self) {
        self.stack_len = 0;
        for v in self.voices.iter_mut() {
            v.amp.gate_off();
            v.filt.gate_off();
            v.off_at = f64::INFINITY;
        }
    }

    pub fn render(&mut self, l: &mut [f32], r: &mut [f32]) {
        for v in self.voices.iter_mut().filter(|v| v.active) {
            v.render(&self.params, self.sr, l, r);
        }
        if self.params.chorus > 0.0 {
            self.chorus.process(l, r, self.params.chorus);
        }
    }
}
