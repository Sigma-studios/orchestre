//! Drawbar organ: nine sine "tonewheels" per note, a key click, optional
//! percussion, and a rotating-speaker effect.

use orchestre_core::OrganParams;

use crate::filter::Svf;
use crate::util::{Rng, TAU, decay_coef, flush, midi_to_hz};

const MAX_VOICES: usize = 16;
/// Harmonic of each drawbar: 16', 5 1/3', 8', 4', 2 2/3', 2', 1 3/5', 1 1/3', 1'.
const RATIOS: [f32; 9] = [0.5, 1.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0];
/// The highest of the Hammond's 91 tonewheels; harmonics above it fold
/// back down an octave, as on the real organ.
const TOP_WHEEL: f32 = 5924.6;

/// Drawbar position (0..1, 8 steps) to level: each step is ~3 dB.
fn drawbar_gain(v: f32) -> f32 {
    if v <= 0.01 {
        0.0
    } else {
        10f32.powf(-3.0 * 8.0 * (1.0 - v.min(1.0)) / 20.0)
    }
}

/// One rotor of a Leslie speaker: speed with inertia, and the Doppler
/// (a delay swinging with the rotor) and loudness sweep it causes.
struct Rotor {
    phase: f32,
    speed: f32,
    slow: f32,
    fast: f32,
    /// Time constants to speed up and to slow down, seconds.
    accel: f32,
    decel: f32,
    /// Doppler swing and loudness sweep.
    swing: f32,
    am: f32,
    buf: Vec<f32>,
    pos: usize,
}

impl Rotor {
    fn new(sr: f32, slow: f32, fast: f32, accel: f32, decel: f32, swing: f32, am: f32) -> Self {
        Rotor {
            phase: 0.0,
            speed: slow,
            slow,
            fast,
            accel,
            decel,
            swing: swing * sr,
            am,
            buf: vec![0.0; (0.004 * sr) as usize + 4],
            pos: 0,
        }
    }

    /// One sample through the rotor; returns (left, right).
    #[inline]
    fn process(&mut self, x: f32, fast: bool, sr: f32) -> (f32, f32) {
        let target = if fast { self.fast } else { self.slow };
        let tau = if target > self.speed {
            self.accel
        } else {
            self.decel
        };
        self.speed += (target - self.speed) * (1.0 / (tau * sr)).min(1.0);
        self.phase = (self.phase + self.speed / sr).fract();
        let len = self.buf.len();
        self.buf[self.pos] = x;
        let sin = (self.phase * TAU).sin();
        let delay = 2.0 + self.swing * (1.0 + sin);
        let read = self.pos as f32 - delay + len as f32;
        let i0 = read.floor() as usize % len;
        let f = read.fract();
        let y = self.buf[i0] * (1.0 - f) + self.buf[(i0 + 1) % len] * f;
        self.pos = (self.pos + 1) % len;
        // Two microphones on either side: one hears the rotor coming as
        // the other hears it going.
        let cos = (self.phase * TAU).cos();
        (y * (1.0 + self.am * cos), y * (1.0 - self.am * cos))
    }
}

/// A Leslie 122-style speaker: treble horn and bass drum, split at 800 Hz.
/// Speeds and inertia from setBfree's defaults (horn 40.32 / 423.36 rpm,
/// drum 36 / 357.3 rpm); horn swing ±0.5 ms for its 17 cm radius (JOS,
/// "Doppler simulation"). The loudness sweep depths are by ear.
struct Leslie {
    split: Svf,
    horn: Rotor,
    drum: Rotor,
}

impl Leslie {
    fn new(sr: f32) -> Self {
        let mut split = Svf::default();
        split.set(800.0, 0.0, sr);
        Leslie {
            split,
            horn: Rotor::new(sr, 0.672, 7.056, 0.161, 0.321, 0.0005, 0.35),
            drum: Rotor::new(sr, 0.6, 5.955, 4.127, 1.371, 0.0002, 0.15),
        }
    }

    fn process(&mut self, l: &mut [f32], r: &mut [f32], fast: bool, sr: f32) {
        for i in 0..l.len() {
            let (low, _, high) = self.split.process((l[i] + r[i]) * 0.5);
            let (hl, hr) = self.horn.process(flush(high), fast, sr);
            let (dl, dr) = self.drum.process(flush(low), fast, sr);
            l[i] = hl + dl;
            r[i] = hr + dr;
        }
    }
}

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
    leslie: Leslie,
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
            // Fast percussion (setBfree: 1 s).
            perc_coef: decay_coef(1.0, sr),
            click_coef: decay_coef(0.006, sr),
            leslie: Leslie::new(sr),
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
        // Percussion is single-trigger: it only sounds when no other key
        // is held (legato playing doesn't retrigger it).
        let legato = self.voices.iter().any(|v| v.active && !v.released);
        let v = &mut self.voices[idx];
        for (w, ratio) in v.wheels.iter_mut().zip(RATIOS) {
            let mut f = f0 * ratio;
            while f > TOP_WHEEL {
                f *= 0.5;
            }
            let omega = TAU * f.min(sr * 0.45) / sr;
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
        v.perc = if self.params.percussion && !legato {
            0.6
        } else {
            0.0
        };
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
        let mut bars = p.drawbars.map(drawbar_gain);
        if p.percussion {
            // On a B-3, percussion takes the 1' drawbar's signal.
            bars[8] = 0.0;
        }
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

        // Rotating speaker: slow ("chorale") below the middle of the knob,
        // fast ("tremolo") above; the rotors take time to change speed.
        if p.rotary > 0.0 {
            self.leslie.process(l, r, p.rotary >= 0.5, self.sr);
        }
    }
}
