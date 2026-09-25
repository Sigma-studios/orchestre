//! Marimba, vibraphone, xylophone and glockenspiel: a few inharmonic sine
//! partials per note (the modes of a struck bar), each with its own decay.

use orchestre_core::{MalletKind, MalletParams};

use crate::filter::Svf;
use crate::util::{Rng, TAU, decay_coef, midi_to_hz, pan_gains};

const MAX_VOICES: usize = 24;
const MAX_PARTIALS: usize = 4;

/// (frequency ratio, relative level, relative decay) of each bar mode.
fn modes(kind: MalletKind) -> &'static [(f32, f32, f32)] {
    match kind {
        MalletKind::Marimba => &[(1.0, 1.0, 1.0), (3.93, 0.35, 0.35), (9.2, 0.12, 0.15)],
        // Tuned to two octaves, then two octaves and a major third: 1:4:10.
        MalletKind::Vibraphone => &[(1.0, 1.0, 1.0), (4.0, 0.25, 0.5), (10.0, 0.08, 0.3)],
        MalletKind::Xylophone => &[
            (1.0, 1.0, 1.0),
            // A bar tuned to 3:1 (euphonics.org, marimbas and xylophones).
            (3.0, 0.5, 0.6),
            (6.16, 0.2, 0.4),
            (10.29, 0.1, 0.3),
        ],
        MalletKind::Glockenspiel => &[
            (1.0, 1.0, 1.0),
            (2.76, 0.6, 0.7),
            (5.4, 0.35, 0.5),
            (8.93, 0.2, 0.4),
        ],
    }
}

/// Ring time (seconds) of the fundamental. After the decay periods in
/// US patent 4,411,187: marimba ~2 s at C3 down to 1/50 s at C7,
/// xylophone ~1 s down to 1/50 s, vibraphone 30 s at F3 down to 6 s at F6.
/// (Glockenspiel: no source found.)
fn ring_time(kind: MalletKind, pitch: f32) -> f32 {
    // (seconds at `center`, octaves per halving, lo, hi)
    let (base, center, halving, lo, hi) = match kind {
        MalletKind::Marimba => (2.0, 48.0, 0.6, 0.02, 3.0),
        MalletKind::Vibraphone => (30.0, 53.0, 1.29, 4.0, 30.0),
        MalletKind::Xylophone => (1.0, 65.0, 0.62, 0.02, 1.2),
        MalletKind::Glockenspiel => (2.8, 72.0, 2.0, 1.0, 4.0),
    };
    (base * (2.0f32).powf(-(pitch - center) / 12.0 / halving)).clamp(lo, hi)
}

#[derive(Clone, Copy, Default)]
struct Partial {
    c: f32,
    s: f32,
    cos_w: f32,
    sin_w: f32,
    amp: f32,
    coef: f32,
}

#[derive(Clone, Copy, Default)]
struct Voice {
    active: bool,
    note: u8,
    off_at: f64,
    age: u64,
    partials: [Partial; MAX_PARTIALS],
    count: usize,
    released: bool,
    damp: f32,
    damp_coef: f32,
    strike: f32,
    strike_coef: f32,
    strike_filter: Svf,
    rng: Rng,
    pan: (f32, f32),
}

impl Voice {
    fn start(&mut self, p: &MalletParams, sr: f32, pitch: u8, vel: f32, off_at: f64, age: u64) {
        let f0 = midi_to_hz(pitch as f32);
        let ring = ring_time(p.kind, pitch as f32) * p.decay;
        // Harder mallets and harder hits excite the upper modes more.
        let bright = 0.3 + 1.4 * p.hardness * (0.5 + 0.5 * vel);
        let vel_gain = 0.2 + 0.8 * vel * vel;
        let mut count = 0;
        for (i, &(ratio, level, decay)) in modes(p.kind).iter().enumerate() {
            let f = f0 * ratio;
            if f > sr * 0.45 {
                continue;
            }
            let w = TAU * f / sr;
            let level = if i == 0 { level } else { level * bright };
            self.partials[count] = Partial {
                c: 1.0,
                s: 0.0,
                cos_w: w.cos(),
                sin_w: w.sin(),
                amp: level * vel_gain,
                coef: decay_coef(ring * decay, sr),
            };
            count += 1;
        }
        self.count = count;
        self.active = true;
        self.note = pitch;
        self.off_at = off_at;
        self.age = age;
        self.released = false;
        self.damp = 1.0;
        self.damp_coef = decay_coef(0.25, sr);
        self.strike = p.hardness * vel * 0.4;
        self.strike_coef = decay_coef(0.004 + 0.01 * (1.0 - p.hardness), sr);
        self.strike_filter.reset();
        self.strike_filter.set((f0 * 3.0).min(sr * 0.4), 0.2, sr);
        self.rng = Rng::new(0x2545_f491 ^ (age as u32).wrapping_mul(2_654_435_761));
        self.pan = pan_gains((pitch as f32 - 66.0) / 36.0 * 0.5);
    }

    fn render(&mut self, gain: f32, tremolo: &[f32], l: &mut [f32], r: &mut [f32]) {
        let (gl, gr) = (self.pan.0 * gain, self.pan.1 * gain);
        for i in 0..l.len() {
            let mut s = 0.0;
            for p in &mut self.partials[..self.count] {
                let c = p.c * p.cos_w - p.s * p.sin_w;
                p.s = p.c * p.sin_w + p.s * p.cos_w;
                p.c = c;
                p.amp *= p.coef;
                s += p.s * p.amp;
            }
            if self.strike > 1e-5 {
                s += self.strike_filter.band(self.rng.noise()) * self.strike;
                self.strike *= self.strike_coef;
            }
            if self.released {
                self.damp *= self.damp_coef;
            }
            let out = s * self.damp * tremolo[i];
            l[i] += out * gl;
            r[i] += out * gr;
        }
        for p in &mut self.partials[..self.count] {
            let m = (p.c * p.c + p.s * p.s).sqrt();
            if m > 0.0 {
                p.c /= m;
                p.s /= m;
            }
        }
        let remaining: f32 = self.partials[..self.count]
            .iter()
            .map(|p| p.amp)
            .sum::<f32>()
            * self.damp;
        if remaining < 1e-4 && self.strike < 1e-4 {
            self.active = false;
        }
    }
}

pub struct MalletEngine {
    pub params: MalletParams,
    sr: f32,
    voices: [Voice; MAX_VOICES],
    counter: u64,
    trem_phase: f32,
}

impl MalletEngine {
    pub fn new(params: MalletParams, sr: f32) -> Self {
        MalletEngine {
            params,
            sr,
            voices: [Voice::default(); MAX_VOICES],
            counter: 0,
            trem_phase: 0.0,
        }
    }

    pub fn set_params(&mut self, params: MalletParams) {
        self.params = params;
    }

    pub fn note_on(&mut self, pitch: u8, vel: f32, off_at: f64) {
        self.counter += 1;
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
        let p = self.params;
        self.voices[idx].start(&p, self.sr, pitch, vel, off_at, self.counter);
    }

    fn release(v: &mut Voice, damper: bool) {
        v.off_at = f64::INFINITY;
        if damper {
            v.released = true;
        }
    }

    pub fn release_due(&mut self, tick: f64) {
        let damper = self.params.damper;
        for v in self
            .voices
            .iter_mut()
            .filter(|v| v.active && v.off_at <= tick)
        {
            Self::release(v, damper);
        }
    }

    pub fn live_off(&mut self, pitch: u8) {
        let damper = self.params.damper;
        for v in self
            .voices
            .iter_mut()
            .filter(|v| v.active && v.note == pitch && v.off_at.is_infinite())
        {
            Self::release(v, damper);
        }
    }

    pub fn all_off(&mut self) {
        for v in self.voices.iter_mut() {
            v.off_at = f64::INFINITY;
            v.released = true;
        }
    }

    pub fn render(&mut self, l: &mut [f32], r: &mut [f32]) {
        // Vibraphone motor: a shared tremolo on all bars.
        let mut trem = [1.0f32; 64];
        let n = l.len().min(trem.len());
        let depth = self.params.tremolo * 0.45;
        for t in trem.iter_mut().take(n) {
            *t = 1.0 - depth * (0.5 + 0.5 * (self.trem_phase * TAU).sin());
            self.trem_phase = (self.trem_phase + 5.5 / self.sr).fract();
        }
        let gain = self.params.gain * 0.5;
        for v in self.voices.iter_mut().filter(|v| v.active) {
            v.render(gain, &trem[..n], &mut l[..n], &mut r[..n]);
        }
    }
}
