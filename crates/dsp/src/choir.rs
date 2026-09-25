//! Choir: a few detuned buzzy voices per note, shaped into vowels by three
//! resonant "formant" filters, like a human vocal tract.

use orchestre_core::{Adsr, ChoirParams, Wave};

use crate::env::Env;
use crate::filter::Svf;
use crate::osc::Osc;
use crate::util::{Rng, TAU, midi_to_hz, pan_gains};

const MAX_VOICES: usize = 16;
const MAX_SINGERS: usize = 4;

/// Formant (frequency Hz, level) for "aah", "ooh", "eeh".
const VOWELS: [[(f32, f32); 3]; 3] = [
    [(800.0, 1.0), (1150.0, 0.5), (2900.0, 0.25)],
    [(350.0, 1.0), (600.0, 0.35), (2700.0, 0.1)],
    [(300.0, 1.0), (2100.0, 0.3), (2900.0, 0.2)],
];
/// Formant bandwidths, Hz.
const BANDWIDTHS: [f32; 3] = [80.0, 90.0, 120.0];

/// Formants for a vowel position 0..1 (aah → ooh → eeh).
fn formants(vowel: f32) -> [(f32, f32); 3] {
    let x = vowel.clamp(0.0, 1.0) * 2.0;
    let (a, b, t) = if x <= 1.0 { (0, 1, x) } else { (1, 2, x - 1.0) };
    let mut out = [(0.0, 0.0); 3];
    for (i, o) in out.iter_mut().enumerate() {
        let (fa, la) = VOWELS[a][i];
        let (fb, lb) = VOWELS[b][i];
        *o = (fa + (fb - fa) * t, la + (lb - la) * t);
    }
    out
}

#[derive(Clone, Copy, Default)]
struct Voice {
    active: bool,
    note: u8,
    off_at: f64,
    age: u64,
    singers: [Osc; MAX_SINGERS],
    /// Each singer's own vibrato phase and detune (cents).
    vib: [f32; MAX_SINGERS],
    detune: [f32; MAX_SINGERS],
    env: Env,
    formants: [Svf; 3],
    rng: Rng,
    pan: (f32, f32),
}

pub struct ChoirEngine {
    pub params: ChoirParams,
    sr: f32,
    voices: [Voice; MAX_VOICES],
    counter: u64,
}

impl ChoirEngine {
    pub fn new(params: ChoirParams, sr: f32) -> Self {
        ChoirEngine {
            params,
            sr,
            voices: [Voice::default(); MAX_VOICES],
            counter: 0,
        }
    }

    fn adsr(p: &ChoirParams) -> Adsr {
        Adsr::new(p.attack.max(0.01), 0.3, 0.9, p.release.max(0.02))
    }

    pub fn set_params(&mut self, params: ChoirParams) {
        self.params = params;
        let adsr = Self::adsr(&params);
        for v in self.voices.iter_mut().filter(|v| v.active) {
            v.env.set(&adsr, self.sr);
        }
    }

    pub fn note_on(&mut self, pitch: u8, vel: f32, off_at: f64) {
        let _ = vel;
        self.counter += 1;
        let idx = self
            .voices
            .iter()
            .position(|v| !v.active)
            .or_else(|| {
                self.voices
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| (!v.env.is_released(), v.age))
                    .map(|(i, _)| i)
            })
            .unwrap_or(0);
        let mut rng = Rng::new(
            0x1b87_3593 ^ (self.counter as u32).wrapping_mul(2_654_435_761) ^ pitch as u32,
        );
        let v = &mut self.voices[idx];
        v.active = true;
        v.note = pitch;
        v.off_at = off_at;
        v.age = self.counter;
        for i in 0..MAX_SINGERS {
            v.singers[i].phase = rng.noise() * 0.5 + 0.5;
            v.vib[i] = rng.noise() * 0.5 + 0.5;
            v.detune[i] = rng.noise();
        }
        v.env.level = 0.0;
        v.env.set(&Self::adsr(&self.params), self.sr);
        v.env.gate_on();
        for f in &mut v.formants {
            f.reset();
        }
        v.pan = pan_gains(rng.noise() * 0.4);
        v.rng = rng;
    }

    pub fn release_due(&mut self, tick: f64) {
        for v in self
            .voices
            .iter_mut()
            .filter(|v| v.active && v.off_at <= tick)
        {
            v.off_at = f64::INFINITY;
            v.env.gate_off();
        }
    }

    pub fn live_off(&mut self, pitch: u8) {
        for v in self
            .voices
            .iter_mut()
            .filter(|v| v.active && v.note == pitch && v.off_at.is_infinite())
        {
            v.env.gate_off();
        }
    }

    pub fn all_off(&mut self) {
        for v in self.voices.iter_mut() {
            v.off_at = f64::INFINITY;
            v.env.gate_off();
        }
    }

    pub fn render(&mut self, l: &mut [f32], r: &mut [f32]) {
        let (p, sr, n) = (self.params, self.sr, l.len());
        let singers = 1 + (p.ensemble * 3.0).round() as usize;
        let spread = 4.0 + p.ensemble * 14.0;
        let form = formants(p.vowel);
        let norm = 1.0 / (singers as f32).sqrt();
        let gain = p.gain * 0.9;
        for v in self.voices.iter_mut().filter(|v| v.active) {
            // Pitch and formants are updated once per block.
            let mut dts = [0.0f32; MAX_SINGERS];
            for ((dt, vib), detune) in dts.iter_mut().zip(&mut v.vib).zip(&v.detune).take(singers) {
                let wobble = (*vib * TAU).sin() * p.vibrato * 0.25;
                *vib = (*vib + (5.0 + 0.4 * detune) * n as f32 / sr).fract();
                let pitch = v.note as f32 + wobble + detune * spread / 100.0;
                *dt = midi_to_hz(pitch) / sr;
            }
            let mut ks = [0.0f32; 3];
            for (i, f) in v.formants.iter_mut().enumerate() {
                let q = form[i].0 / BANDWIDTHS[i];
                f.set_q(form[i].0, q, sr);
                ks[i] = 1.0 / q;
            }
            for i in 0..n {
                let mut src = 0.0;
                for (singer, &dt) in v.singers.iter_mut().zip(&dts).take(singers) {
                    src += singer.next(Wave::Saw, dt, 0.0);
                }
                src = src * norm + v.rng.noise() * p.breath * 0.4;
                let mut out = 0.0;
                for (j, f) in v.formants.iter_mut().enumerate() {
                    out += f.band(src) * ks[j] * form[j].1;
                }
                let e = v.env.next() * gain;
                l[i] += out * e * v.pan.0;
                r[i] += out * e * v.pan.1;
            }
            if v.env.is_idle() {
                v.active = false;
            }
        }
    }
}
