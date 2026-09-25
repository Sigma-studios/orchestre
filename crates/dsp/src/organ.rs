//! Drawbar organ: nine sine "tonewheels" per note, a key click, optional
//! percussion, and a rotating-speaker effect.

use orchestre_core::OrganParams;

use crate::filter::Svf;
use crate::fx::Chorus;
use crate::util::{Rng, TAU, decay_coef, midi_to_hz};

const MAX_VOICES: usize = 16;
/// Harmonic of each drawbar: 16', 5 1/3', 8', 4', 2 2/3', 2', 1 3/5', 1 1/3', 1'.
const RATIOS: [f32; 9] = [0.5, 1.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0];

#[derive(Clone, Copy, Default)]
struct Wheel {
    c: f32,
    s: f32,
    cos_w: f32,
    sin_w: f32,
}

#[derive(Clone, Copy, Default)]
struct Voice {
    active: bool,
    note: u8,
    off_at: f64,
    age: u64,
    wheels: [Wheel; 9],
    /// Amplitude envelope: fast linear attack, short exponential release.
    level: f32,
    released: bool,
    perc: f32,
    click: f32,
    click_filter: Svf,
    rng: Rng,
}

pub struct OrganEngine {
    pub params: OrganParams,
    sr: f32,
    voices: [Voice; MAX_VOICES],
    counter: u64,
    attack_inc: f32,
    release_coef: f32,
    perc_coef: f32,
    click_coef: f32,
    rotor: f32,
    chorus: Chorus,
}

impl OrganEngine {
    pub fn new(params: OrganParams, sr: f32) -> Self {
        OrganEngine {
            params,
            sr,
            voices: [Voice::default(); MAX_VOICES],
            counter: 0,
            attack_inc: 1.0 / (0.005 * sr),
            release_coef: decay_coef(0.06, sr),
            perc_coef: decay_coef(0.5, sr),
            click_coef: decay_coef(0.006, sr),
            rotor: 0.0,
            chorus: Chorus::new(sr),
        }
    }

    pub fn set_params(&mut self, params: OrganParams) {
        self.params = params;
    }

    pub fn note_on(&mut self, pitch: u8, vel: f32, off_at: f64) {
        self.counter += 1;
        let sr = self.sr;
        let idx = self
            .voices
            .iter()
            .position(|v| !v.active)
            .or_else(|| {
                self.voices
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| v.age)
                    .map(|(i, _)| i)
            })
            .unwrap_or(0);
        let f0 = midi_to_hz(pitch as f32);
        let v = &mut self.voices[idx];
        for (w, ratio) in v.wheels.iter_mut().zip(RATIOS) {
            let omega = TAU * (f0 * ratio).min(sr * 0.45) / sr;
            *w = Wheel {
                c: 1.0,
                s: 0.0,
                cos_w: omega.cos(),
                sin_w: omega.sin(),
            };
        }
        v.active = true;
        v.note = pitch;
        v.off_at = off_at;
        v.age = self.counter;
        v.level = 0.0;
        v.released = false;
        v.perc = if self.params.percussion { 0.6 } else { 0.0 };
        // Organs aren't touch sensitive, but a soft hit gets a softer click.
        v.click = self.params.click * (0.5 + 0.5 * vel) * 0.5;
        v.click_filter.reset();
        v.click_filter.set(2500.0, 0.3, sr);
        v.rng = Rng::new(0x68e3_1da4 ^ (self.counter as u32).wrapping_mul(2_654_435_761));
    }

    pub fn release_due(&mut self, tick: f64) {
        for v in self
            .voices
            .iter_mut()
            .filter(|v| v.active && v.off_at <= tick)
        {
            v.off_at = f64::INFINITY;
            v.released = true;
        }
    }

    pub fn live_off(&mut self, pitch: u8) {
        for v in self
            .voices
            .iter_mut()
            .filter(|v| v.active && v.note == pitch && v.off_at.is_infinite())
        {
            v.released = true;
        }
    }

    pub fn all_off(&mut self) {
        for v in self.voices.iter_mut() {
            v.off_at = f64::INFINITY;
            v.released = true;
        }
    }

    pub fn render(&mut self, l: &mut [f32], r: &mut [f32]) {
        let p = self.params;
        let bars = p.drawbars;
        let total: f32 = bars.iter().sum();
        let norm = p.gain * 0.35 / (1.0 + total * 0.35);
        let (attack, release, perc_coef, click_coef) = (
            self.attack_inc,
            self.release_coef,
            self.perc_coef,
            self.click_coef,
        );
        for v in self.voices.iter_mut().filter(|v| v.active) {
            for i in 0..l.len() {
                let mut s = 0.0;
                for (w, &bar) in v.wheels.iter_mut().zip(&bars) {
                    let c = w.c * w.cos_w - w.s * w.sin_w;
                    w.s = w.c * w.sin_w + w.s * w.cos_w;
                    w.c = c;
                    s += w.s * bar;
                }
                // Percussion rings on the 2 2/3' wheel (3rd harmonic).
                s += v.wheels[4].s * v.perc;
                v.perc *= perc_coef;
                if v.click > 1e-5 {
                    s += v.click_filter.band(v.rng.noise()) * v.click;
                    v.click *= click_coef;
                }
                if v.released {
                    v.level *= release;
                } else if v.level < 1.0 {
                    v.level = (v.level + attack).min(1.0);
                }
                let out = s * v.level * norm;
                l[i] += out;
                r[i] += out;
            }
            for w in v.wheels.iter_mut() {
                let m = (w.c * w.c + w.s * w.s).sqrt();
                if m > 0.0 {
                    w.c /= m;
                    w.s /= m;
                }
            }
            if v.released && v.level < 1e-4 {
                v.active = false;
            }
        }

        // Rotating speaker: the horn sweeps left/right (amplitude) and
        // towards/away from you (pitch wobble, via the chorus).
        if p.rotary > 0.0 {
            let speed = 0.8 + 5.9 * p.rotary;
            let depth = 0.25 + 0.15 * p.rotary;
            for i in 0..l.len() {
                let m = (self.rotor * TAU).sin() * depth;
                l[i] *= 1.0 + m;
                r[i] *= 1.0 - m;
                self.rotor = (self.rotor + speed / self.sr).fract();
            }
            self.chorus.process(l, r, 0.3 + 0.4 * p.rotary);
        }
    }
}
