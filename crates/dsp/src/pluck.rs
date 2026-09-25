//! Guitar, harp, bass guitar and koto: Karplus–Strong string synthesis.
//! A burst of noise circulates in a delay line one period long; a gentle
//! low-pass in the loop makes the high harmonics die first, like a string.

use orchestre_core::{PluckKind, PluckParams};

use crate::filter::Svf;
use crate::util::{Rng, midi_to_hz, pan_gains};

const MAX_VOICES: usize = 16;
/// Longest period supported: about 11 Hz at 48 kHz.
const BUF_LEN: usize = 4096;

struct Voice {
    active: bool,
    note: u8,
    off_at: f64,
    age: u64,
    buf: Vec<f32>,
    write: usize,
    /// Whole samples of delay (the rest is the loop filters' delay).
    delay: usize,
    /// Loop low-pass coefficient and state.
    damp: f32,
    lp: f32,
    /// Fractional-delay allpass (tuning) coefficient and state.
    ap: f32,
    ap_x: f32,
    ap_y: f32,
    /// Loss per trip around the loop while ringing, and once muted.
    gain: f32,
    mute_gain: f32,
    muted: bool,
    period: usize,
    pan: (f32, f32),
    quiet: usize,
}

impl Voice {
    fn new() -> Self {
        Voice {
            active: false,
            note: 0,
            off_at: 0.0,
            age: 0,
            buf: vec![0.0; BUF_LEN],
            write: 0,
            delay: 1,
            damp: 0.0,
            lp: 0.0,
            ap: 0.0,
            ap_x: 0.0,
            ap_y: 0.0,
            gain: 1.0,
            mute_gain: 1.0,
            muted: false,
            period: 1,
            pan: (0.707, 0.707),
            quiet: 0,
        }
    }

    fn start(&mut self, p: &PluckParams, sr: f32, pitch: u8, vel: f32, off_at: f64, age: u64) {
        let f = midi_to_hz(pitch as f32).clamp(20.0, sr * 0.2);
        let period = sr / f;

        // Loop low-pass: darker strings lose their highs faster.
        let bright = (p.brightness * 0.7 + vel * 0.3).clamp(0.0, 1.0);
        // High strings take more trips per second through the filter, so
        // they get a lighter one to keep a natural ring.
        self.damp = (0.02 + 0.3 * (1.0 - bright)) * (200.0 / f).clamp(0.15, 1.0);
        let lp_delay = self.damp / (1.0 - self.damp);
        // Split the remaining delay into whole samples plus an allpass for
        // the fraction (kept in 0.5..1.5 samples for a well-behaved allpass).
        let d = (period - lp_delay).max(2.0);
        let whole = (d - 0.5).floor().max(1.0);
        let frac = d - whole;
        self.delay = (whole as usize).min(BUF_LEN - 1);
        self.ap = (1.0 - frac) / (1.0 + frac);
        self.ap_x = 0.0;
        self.ap_y = 0.0;
        self.lp = 0.0;

        // Ring time: low strings sustain longer.
        // Ring time falls as f^-0.4 (measured on harps, Woodhouse 2021).
        let ring = (6.0 * (f / 110.0).powf(-0.4)).clamp(0.8, 12.0) * p.decay;
        // Each sample passes the loss once per trip around the loop (one
        // period), so the per-trip factor is set from the frequency.
        self.gain = (0.001f32).powf(1.0 / (ring * f));
        self.mute_gain = (0.001f32).powf(1.0 / (0.08 * f));
        self.muted = false;

        // Excitation: filtered noise, with a notch pattern from the pluck
        // position (plucking near the middle cancels even harmonics).
        let n = (period.round() as usize).clamp(2, BUF_LEN - 1);
        let mut rng =
            Rng::new(0x9e37_79b9 ^ (age as u32).wrapping_mul(2_246_822_519) ^ pitch as u32);
        let soft = 0.6 * (1.0 - bright);
        let mut state = 0.0;
        let mut exc = [0.0f32; BUF_LEN];
        for e in exc.iter_mut().take(n) {
            state = state * soft + rng.noise() * (1.0 - soft);
            *e = state;
        }
        let offset = ((0.05 + 0.45 * p.position) * n as f32) as usize;
        let mean = exc[..n].iter().sum::<f32>() / n as f32;
        let amp = 0.3 + 0.7 * vel;
        self.buf.fill(0.0);
        for i in 0..n {
            let comb = exc[i] - if i >= offset { exc[i - offset] } else { 0.0 };
            // Stored so that the newest sample is just behind `write`.
            self.buf[(BUF_LEN + i - n) % BUF_LEN] = (comb - mean) * amp;
        }
        self.write = 0;
        self.period = n;
        self.active = true;
        self.note = pitch;
        self.off_at = off_at;
        self.age = age;
        self.quiet = 0;
        self.pan = pan_gains((pitch as f32 - 60.0) / 36.0 * 0.5);
    }

    fn render(&mut self, out_gain: f32, l: &mut [f32], r: &mut [f32]) {
        let loss = if self.muted {
            self.mute_gain.min(self.gain)
        } else {
            self.gain
        };
        let (gl, gr) = (self.pan.0 * out_gain, self.pan.1 * out_gain);
        let mut peak = 0.0f32;
        for i in 0..l.len() {
            let x = self.buf[(self.write + BUF_LEN - self.delay) % BUF_LEN];
            self.lp = x * (1.0 - self.damp) + self.lp * self.damp;
            let y = self.ap * self.lp + self.ap_x - self.ap * self.ap_y;
            self.ap_x = self.lp;
            self.ap_y = y;
            let y = y * loss;
            self.buf[self.write] = y;
            self.write = (self.write + 1) % BUF_LEN;
            peak = peak.max(y.abs());
            l[i] += y * gl;
            r[i] += y * gr;
        }
        self.quiet = if peak < 1e-4 { self.quiet + l.len() } else { 0 };
        if self.quiet > self.period + 64 {
            self.active = false;
        }
    }
}

pub struct PluckEngine {
    pub params: PluckParams,
    sr: f32,
    voices: Vec<Voice>,
    counter: u64,
    /// Wooden body: two broad resonances.
    body: [Svf; 2],
}

impl PluckEngine {
    pub fn new(params: PluckParams, sr: f32) -> Self {
        let mut e = PluckEngine {
            params,
            sr,
            voices: (0..MAX_VOICES).map(|_| Voice::new()).collect(),
            counter: 0,
            body: [Svf::default(); 2],
        };
        e.set_params(params);
        e
    }

    pub fn set_params(&mut self, params: PluckParams) {
        self.params = params;
        let (a, b) = match params.kind {
            PluckKind::BassGuitar => (70.0, 160.0),
            // Measured koto: 85 Hz air mode, 100 Hz first body mode (ICA 2019).
            PluckKind::Koto => (85.0, 100.0),
            _ => (105.0, 230.0),
        };
        self.body[0].set(a, 0.5, self.sr);
        self.body[1].set(b, 0.5, self.sr);
    }

    pub fn note_on(&mut self, pitch: u8, vel: f32, off_at: f64) {
        self.counter += 1;
        // Re-plucking the same string restarts it.
        let idx = self
            .voices
            .iter()
            .position(|v| v.active && v.note == pitch)
            .or_else(|| self.voices.iter().position(|v| !v.active))
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

    fn release(v: &mut Voice, ring: bool) {
        v.off_at = f64::INFINITY;
        if !ring {
            v.muted = true;
        }
    }

    pub fn release_due(&mut self, tick: f64) {
        let ring = self.params.ring;
        for v in self
            .voices
            .iter_mut()
            .filter(|v| v.active && v.off_at <= tick)
        {
            Self::release(v, ring);
        }
    }

    pub fn live_off(&mut self, pitch: u8) {
        let ring = self.params.ring;
        for v in self
            .voices
            .iter_mut()
            .filter(|v| v.active && v.note == pitch && v.off_at.is_infinite())
        {
            Self::release(v, ring);
        }
    }

    pub fn all_off(&mut self) {
        for v in self.voices.iter_mut() {
            v.off_at = f64::INFINITY;
            v.muted = true;
        }
    }

    pub fn render(&mut self, l: &mut [f32], r: &mut [f32]) {
        let gain = self.params.gain * 0.4;
        for v in self.voices.iter_mut().filter(|v| v.active) {
            v.render(gain, l, r);
        }
        let body = self.params.body * 0.6;
        if body > 0.0 {
            for i in 0..l.len() {
                let mono = (l[i] + r[i]) * 0.5;
                let res = self.body[0].band(mono) + self.body[1].band(mono);
                l[i] += res * body;
                r[i] += res * body;
            }
        }
    }
}
