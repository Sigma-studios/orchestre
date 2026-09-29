//! Talk box and vocoder: a synth sung through a "mouth". Every new note
//! steps to the next vowel of the sequence; the mouth closes between notes
//! and opens into the vowel ("wah"), which is what makes it talk.
//!
//! One mouth is shared by all voices, as with the real thing (one player's
//! mouth on the talk box tube; one singer feeding a vocoder).
//! - Talk box: the summed synth goes through four resonant band-pass
//!   formant filters that follow the vowel smoothly.
//! - Vocoder: the synth (plus noise for breath and "s") goes through a bank
//!   of 16 fixed band-pass filters, each opened as far as the vowel's
//!   spectral envelope at its centre: the grainy robot sound.

use orchestre_core::{TalkBoxParams, Vowel};

use crate::env::Env;
use crate::filter::Svf;
use crate::fx::Ensemble;
use crate::osc::Osc;
use crate::util::{Rng, TAU, midi_to_hz};

const MAX_VOICES: usize = 8;
const MONO_STACK: usize = 8;
const BANDS: usize = 16;
/// Relative level and bandwidth (Hz) of formants F1..F4.
const FORMANTS: [(f32, f32); 4] = [(1.0, 80.0), (0.7, 100.0), (0.45, 130.0), (0.3, 170.0)];
/// F4 barely moves between vowels.
const F4: f32 = 3300.0;
/// Two note-ons closer than this are one onset (a chord): one vowel step.
const ONSET_GAP: f32 = 0.03;

#[derive(Clone, Copy, Default)]
struct Voice {
    active: bool,
    note: u8,
    pitch: f32,
    target: f32,
    vel: f32,
    off_at: f64,
    age: u64,
    osc: Osc,
    amp: Env,
}

pub struct TalkBoxEngine {
    pub params: TalkBoxParams,
    sr: f32,
    voices: [Voice; MAX_VOICES],
    counter: u64,
    stack: [(u8, f32, f64); MONO_STACK],
    stack_len: usize,
    /// Index of the vowel being sung.
    step: usize,
    /// Seconds since the last vowel step.
    since_step: f32,
    started: bool,
    /// 0 = closed ("oo"), 1 = open on the vowel.
    mouth: f32,
    formants: [f32; 4],
    bp: [Svf; 4],
    bank: [Svf; BANDS],
    band_gain: [f32; BANDS],
    vib: f32,
    rng: Rng,
    chorus: Ensemble,
}

impl TalkBoxEngine {
    pub fn new(params: TalkBoxParams, sr: f32) -> Self {
        let mut e = TalkBoxEngine {
            params,
            sr,
            voices: [Voice::default(); MAX_VOICES],
            counter: 0,
            stack: [(0, 0.0, 0.0); MONO_STACK],
            stack_len: 0,
            step: 0,
            since_step: 1.0,
            started: false,
            mouth: 0.0,
            formants: [300.0, 870.0, 2240.0, F4],
            bp: [Svf::default(); 4],
            bank: [Svf::default(); BANDS],
            band_gain: [0.0; BANDS],
            vib: 0.0,
            rng: Rng::new(99),
            chorus: Ensemble::new(sr),
        };
        for (b, f) in e.bank.iter_mut().enumerate() {
            f.set_q(band_centre(b), 6.0, sr);
        }
        e
    }

    pub fn set_params(&mut self, params: TalkBoxParams) {
        if params.mono != self.params.mono {
            self.all_off();
        }
        self.params = params;
        for v in self.voices.iter_mut().filter(|v| v.active) {
            v.amp.set(&params.amp, self.sr);
        }
    }

    /// Step to the next vowel (once per onset) and close the mouth.
    fn articulate(&mut self) {
        if self.since_step < ONSET_GAP {
            return;
        }
        if self.started {
            self.step = (self.step + 1) % (self.params.steps as usize).clamp(1, 8);
        }
        self.started = true;
        self.since_step = 0.0;
        self.mouth = 1.0 - self.params.mouth.clamp(0.0, 1.0);
    }

    fn start(&mut self, idx: usize, pitch: u8, vel: f32, off_at: f64) {
        self.counter += 1;
        let v = &mut self.voices[idx];
        v.active = true;
        v.note = pitch;
        v.pitch = pitch as f32;
        v.target = pitch as f32;
        v.vel = vel;
        v.off_at = off_at;
        v.age = self.counter;
        v.amp.level = 0.0;
        v.amp.set(&self.params.amp, self.sr);
        v.amp.gate_on();
    }

    pub fn note_on(&mut self, pitch: u8, vel: f32, off_at: f64) {
        self.articulate();
        if self.params.mono {
            if self.stack_len == MONO_STACK {
                self.stack.copy_within(1.., 0);
                self.stack_len -= 1;
            }
            self.stack[self.stack_len] = (pitch, vel, off_at);
            self.stack_len += 1;
            let v = &mut self.voices[0];
            if v.active && !v.amp.is_released() {
                v.target = pitch as f32;
                v.note = pitch;
                v.off_at = off_at;
                v.vel = vel;
            } else {
                let from = v.active.then_some(v.pitch);
                self.start(0, pitch, vel, off_at);
                if let Some(from) = from {
                    self.voices[0].pitch = from;
                }
            }
            return;
        }
        let idx = self
            .voices
            .iter()
            .position(|v| !v.active)
            .or_else(|| {
                self.voices
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| (!v.amp.is_released(), v.age))
                    .map(|(i, _)| i)
            })
            .unwrap_or(0);
        self.start(idx, pitch, vel, off_at);
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
        if j == before {
            return;
        }
        let v = &mut self.voices[0];
        if j == 0 {
            v.amp.gate_off();
        } else {
            let (pitch, vel, off_at) = self.stack[j - 1];
            v.target = pitch as f32;
            v.note = pitch;
            v.vel = vel;
            v.off_at = off_at;
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
        }
    }

    pub fn all_off(&mut self) {
        self.stack_len = 0;
        for v in self.voices.iter_mut() {
            v.amp.gate_off();
            v.off_at = f64::INFINITY;
        }
    }

    /// Move the mouth and formants for a block of `n` samples.
    fn update_mouth(&mut self, n: usize) {
        let p = &self.params;
        let dt = n as f32 / self.sr;
        self.since_step += dt;
        let gated = self.voices.iter().any(|v| v.active && !v.amp.is_released());
        // Open into the vowel while notes are held; close after.
        let target = if gated { 1.0 } else { 1.0 - p.mouth };
        let k = 1.0 - (-dt / p.mouth_time.max(0.005)).exp();
        self.mouth += (target - self.mouth) * k;

        let vowel = p.vowels[self.step.min(p.vowels.len() - 1)];
        let (o, f) = vowel.shape();
        let (co, cf) = Vowel::Oo.shape();
        let m = self.mouth;
        let [f1, f2, f3] =
            orchestre_core::sfx::vowel_formants(co + (o - co) * m, cf + (f - cf) * m);
        let scale = (2.0f32).powf(p.shift / 12.0);
        let target = [f1 * scale, f2 * scale, f3 * scale, F4 * scale];
        // A little smoothing so legato vowel changes glide like a tongue.
        let glide = 1.0 - (-dt / 0.025).exp();
        for (cur, t) in self.formants.iter_mut().zip(target) {
            *cur += (t - *cur) * glide;
        }
        if p.vocoder {
            for b in 0..BANDS {
                let c = band_centre(b);
                let mut g = 0.0;
                for (i, &(level, _)) in FORMANTS.iter().enumerate() {
                    let fi = self.formants[i];
                    let width = fi * 0.2 + 60.0;
                    let d = (c - fi) / width;
                    g += level / (1.0 + d * d);
                }
                self.band_gain[b] += (g.min(1.2) - self.band_gain[b]) * glide;
            }
        } else {
            for (i, bp) in self.bp.iter_mut().enumerate() {
                let fi = self.formants[i].min(self.sr * 0.45);
                bp.set_q(fi, fi / FORMANTS[i].1, self.sr);
            }
        }
    }

    pub fn render(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len();
        self.update_mouth(n);
        let p = self.params;
        let sr = self.sr;
        // The carrier: every voice, summed.
        let mut carrier = [0.0f32; 64];
        let carrier = &mut carrier[..n.min(64)];
        let vib = (self.vib * TAU).sin() * p.vibrato * 0.3;
        self.vib = (self.vib + 5.5 * n as f32 / sr).fract();
        for v in self.voices.iter_mut().filter(|v| v.active) {
            if p.mono && p.glide > 0.0 {
                let k = (-(n as f32) / (p.glide * sr)).exp();
                v.pitch = v.target + (v.pitch - v.target) * k;
            } else {
                v.pitch = v.target;
            }
            let dt = midi_to_hz(v.pitch + p.octave as f32 * 12.0 + vib) / sr;
            let gain = 0.3 + 0.7 * v.vel;
            for c in carrier.iter_mut() {
                *c += v.osc.next(p.wave, dt, 0.0) * v.amp.next() * gain;
            }
            if v.amp.is_idle() {
                v.active = false;
            }
        }
        let open = 0.35 + 0.65 * self.mouth;
        for (i, c) in carrier.iter().enumerate() {
            let noise = self.rng.noise() * p.noise * 0.6 * c.abs().sqrt();
            let x = c + noise;
            let y = if p.vocoder {
                let mut y = 0.0;
                for (f, g) in self.bank.iter_mut().zip(self.band_gain) {
                    y += f.band_norm(x) * g;
                }
                y * 0.4
            } else {
                let mut y = 0.0;
                for (f, &(level, _)) in self.bp.iter_mut().zip(&FORMANTS) {
                    y += f.band_norm(x) * level;
                }
                y * 1.2
            };
            let y = y * open * p.gain;
            l[i] += y;
            r[i] += y;
        }
        if p.chorus > 0.0 {
            self.chorus.process(l, r, p.chorus);
        }
    }
}

/// Vocoder band centres: log-spaced from 150 Hz to 7 kHz.
fn band_centre(b: usize) -> f32 {
    150.0 * (7000.0f32 / 150.0).powf(b as f32 / (BANDS - 1) as f32)
}
