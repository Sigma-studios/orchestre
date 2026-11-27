//! Meow synth, after Knobster's MeowSynth (2009) as recreated by nyasynth
//! (github.com/a2aaron/nyasynth), whose author probed the original: at full
//! velocity its filter sweeps from about 350 Hz to 8 kHz over each note, at
//! zero velocity it stays at 350 Hz, and one "MEOW" envelope drives both the
//! volume and the filter.
//!
//! Written from nyasynth's behaviour (its code has no license, so none is
//! copied): a band-limited saw through an RBJ low-pass, Q 2.5, whose cutoff
//! moves in octaves along the envelope; straight-line envelope stages; an
//! exponential velocity curve for the sweep and a cubic one for the volume; a
//! portamento that reaches the new pitch in a fixed time; a global vibrato
//! LFO synced to the tempo; and, playing one note at a time, a fresh voice
//! (crossfaded in over 1 ms) for every note, so each one meows.

use orchestre_core::{MeowParams, Wave};

use crate::osc::Osc;
use crate::util::{Rng, TAU};

const MAX_VOICES: usize = 16;
const MAX_KEYS: usize = 16;
/// The filter's cutoff is updated every this many samples.
const FILTER_EVERY: u32 = 16;
/// Output level: nyasynth's -6 dB master volume, then a trim that puts the
/// default sound level with Orchestre's other instruments.
const LEVEL: f32 = 0.5 * 0.55;

/// 0 → 0, 1 → 1, steeply curved: how velocity scales the filter sweep.
fn ease_in_expo(x: f32) -> f32 {
    if x <= 0.0 {
        0.0
    } else {
        ((10.0 * x).exp2() - 1.0) / (1024.0 - 1.0)
    }
}

fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// The envelope's sustain level for a sustain knob in dB (-24..0). The knob
/// is skewed so that its middle reads -12 dB, and the envelope takes the
/// knob's position, not the gain: -15 dB gives a level of about 0.65.
pub(crate) fn sustain_level(db: f32) -> f32 {
    let (min, max) = (-24.0f32, 0.0f32);
    let (lo, hi, mid) = (
        db_to_gain(min),
        db_to_gain(max),
        db_to_gain((min + max) / 2.0),
    );
    let skew = 0.5f32.ln() / ((mid - lo) / (hi - lo)).ln();
    ((db.clamp(min, max) - min) / (max - min)).powf(skew)
}

/// Filter cutoff for envelope level `env`: from `cutoff` (0) to `cutoff`
/// plus the velocity-scaled sweep (1), evenly in octaves.
pub(crate) fn cutoff(p: &MeowParams, vel_sweep: f32, env: f32) -> f32 {
    let lo = p.cutoff.max(1.0).log2();
    let hi = (p.cutoff + p.sweep * vel_sweep).max(1.0).log2();
    (lo + (hi - lo) * env).exp2()
}

/// Direct-form-I biquad (RBJ cookbook low-pass).
#[derive(Clone, Copy, Debug, Default)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    fn lowpass(cutoff: f32, q: f32, sr: f32) -> Self {
        let mut f = Biquad::default();
        f.set_lowpass(cutoff, q, sr);
        f
    }

    fn set_lowpass(&mut self, cutoff: f32, q: f32, sr: f32) {
        let (sin, cos) = (TAU * cutoff / sr).sin_cos();
        let alpha = sin / (2.0 * q.max(0.01));
        let a0 = 1.0 + alpha;
        self.b1 = (1.0 - cos) / a0;
        self.b0 = self.b1 * 0.5;
        self.b2 = self.b0;
        self.a1 = -2.0 * cos / a0;
        self.a2 = (1.0 - alpha) / a0;
    }

    #[inline]
    fn run(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }

    fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}

/// Straight-line ADSR, computed from the time since the note started (or
/// was released). Released, it falls from wherever it was.
#[derive(Clone, Copy, Debug, Default)]
struct Env {
    from: f32,
    last: f32,
}

impl Env {
    fn held(&mut self, t: f32, attack: f32, decay: f32, sustain: f32) -> f32 {
        let v = if attack > 0.0 && t < attack {
            t / attack
        } else if decay > 0.0 && t < attack + decay {
            1.0 + (sustain - 1.0) * (t - attack) / decay
        } else {
            sustain
        };
        self.last = v;
        v
    }

    fn released(&mut self, t: f32, release: f32) -> f32 {
        let v = if release > 0.0 {
            self.from * (1.0 - (t / release).min(1.0))
        } else {
            0.0
        };
        self.last = v;
        v
    }

    fn remember(&mut self) {
        self.from = self.last;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Fade {
    #[default]
    None,
    In,
    Out,
}

#[derive(Clone, Copy, Default)]
struct Voice {
    active: bool,
    note: u8,
    /// Tick at which the note is released (chords); infinite for live notes.
    off_at: f64,
    /// Samples since the note started, and when it was released.
    t: u32,
    released_at: Option<u32>,
    /// Portamento, in octaves (log2 Hz).
    from_pitch: f32,
    to_pitch: f32,
    vel: f32,
    /// `vel` on the sweep's curve.
    vel_sweep: f32,
    osc: Osc,
    meow: Env,
    vibrato: Env,
    filter: Biquad,
    fade: Fade,
    fade_n: u32,
    age: u64,
}

impl Voice {
    fn start(&mut self, pitch: u8, vel: f32, from: Option<f32>, off_at: f64, sr: f32, age: u64) {
        let to = (crate::util::midi_to_hz(pitch as f32)).log2();
        *self = Voice {
            active: true,
            note: pitch,
            off_at,
            from_pitch: from.unwrap_or(to),
            to_pitch: to,
            vel,
            vel_sweep: ease_in_expo(vel),
            // Until the first cutoff update.
            filter: Biquad::lowpass(
                10_000.0f32.min(sr * 0.45),
                std::f32::consts::FRAC_1_SQRT_2,
                sr,
            ),
            age,
            ..Voice::default()
        };
    }

    fn release(&mut self) {
        if self.released_at.is_none() {
            self.meow.remember();
            self.vibrato.remember();
            self.released_at = Some(self.t);
        }
    }

    /// Current pitch along the portamento, in octaves.
    fn pitch(&self, p: &MeowParams, sr: f32) -> f32 {
        if p.glide <= 0.0 {
            return self.to_pitch;
        }
        let k = (self.t as f32 / sr / p.glide).min(1.0);
        self.from_pitch + (self.to_pitch - self.from_pitch) * k
    }

    fn is_done(&self, p: &MeowParams, sr: f32) -> bool {
        match self.released_at {
            None => self.fade == Fade::Out && self.fade_n >= fade_len(sr),
            Some(at) => {
                (self.t - at) as f32 / sr >= p.release
                    || (self.fade == Fade::Out && self.fade_n >= fade_len(sr))
            }
        }
    }

    /// `vibrato` is the global LFO times the vibrato amount.
    #[inline]
    fn next(&mut self, p: &MeowParams, sr: f32, vibrato: f32, noise: f32) -> f32 {
        self.t += 1;
        let secs = self.t as f32 / sr;
        let (env, vib_env) = match self.released_at {
            None => (
                self.meow
                    .held(secs, p.attack, p.decay, sustain_level(p.sustain_db)),
                self.vibrato.held(secs, p.vibrato_attack, 0.001, 1.0),
            ),
            Some(at) => {
                let t = (self.t - at) as f32 / sr;
                (
                    self.meow.released(t, p.release),
                    self.vibrato.released(t, 5.0),
                )
            }
        };
        let volume = self.vel * ((env * env * env + env) * 0.5).max(0.0);

        // Up to ±2 semitones of vibrato.
        let pitch = self.pitch(p, sr) + vibrato * vib_env * 2.0 / 12.0;
        let mut x = self.osc.next(Wave::Saw, pitch.exp2() / sr, 0.0);
        if p.noise > 0.01 {
            x += noise * p.noise;
        }

        if self.t.is_multiple_of(FILTER_EVERY) {
            let cutoff = cutoff(p, self.vel_sweep, env).clamp(20.0, sr * 0.99 / 2.0);
            self.filter.set_lowpass(cutoff, p.q, sr);
        }
        let mut y = self.filter.run(x);
        if !y.is_finite() {
            self.filter.reset();
            y = x;
        }
        y *= volume;

        let n = fade_len(sr);
        match self.fade {
            Fade::None => y,
            Fade::In => {
                let g = (self.fade_n as f32 / n as f32).min(1.0);
                self.fade_n += 1;
                y * g
            }
            Fade::Out => {
                let g = 1.0 - (self.fade_n as f32 / n as f32).min(1.0);
                self.fade_n += 1;
                y * g
            }
        }
    }
}

/// Crossfade between notes: 44 samples at 44.1 kHz.
fn fade_len(sr: f32) -> u32 {
    ((sr / 1000.0).round() as u32).max(1)
}

/// nyasynth's chorus: one delay tap, ~10–12 ms, swept by a slow sine,
/// darkened by a gentle low-pass.
struct Chorus {
    buf: Vec<f32>,
    pos: usize,
    phase: f32,
    lp: Biquad,
    /// Samples per sample at 44.1 kHz (its delays are given in samples).
    scale: f32,
}

impl Chorus {
    fn new(sr: f32) -> Self {
        let scale = sr / 44_100.0;
        Chorus {
            buf: vec![0.0; (1300.0 * scale) as usize + 4],
            pos: 0,
            phase: 0.0,
            lp: Biquad::lowpass(sr / 4.0, 0.1, sr),
            scale,
        }
    }

    fn next(&mut self, x: f32, sr: f32) -> f32 {
        let len = self.buf.len();
        self.pos = (self.pos + 1) % len;
        self.buf[self.pos] = x;
        let lfo = (self.phase * TAU).sin();
        self.phase = (self.phase + 0.33 / sr).fract();
        let delay = (450.0 + (lfo + 1.0) * 44.0) * self.scale;
        let read = self.pos as f32 - delay + len as f32;
        let i = read.floor() as usize;
        let f = read - read.floor();
        let at = |k: usize| self.buf[(i + len + k - 1) % len];
        let (y0, y1, y2, y3) = (at(0), at(1), at(2), at(3));
        // 4-point Lagrange interpolation between y1 and y2.
        let c = y1
            + f * (-y0 / 3.0 - y1 / 2.0 + y2 - y3 / 6.0)
            + f * f * (y0 / 2.0 - y1 + y2 / 2.0)
            + f * f * f * (-y0 / 6.0 + y1 / 2.0 - y2 / 2.0 + y3 / 6.0);
        self.lp.run(c)
    }
}

pub struct MeowEngine {
    pub params: MeowParams,
    sr: f32,
    bpm: f32,
    voices: [Voice; MAX_VOICES],
    /// Held keys, oldest first: (pitch, velocity, release tick).
    keys: [(u8, f32, f64); MAX_KEYS],
    nkeys: usize,
    /// Chords: the held note the next one slides from.
    porta_key: Option<u8>,
    counter: u64,
    vibrato_phase: f32,
    rng: Rng,
    chorus: Chorus,
}

impl MeowEngine {
    pub fn new(params: MeowParams, sr: f32) -> Self {
        MeowEngine {
            params,
            sr,
            bpm: 120.0,
            voices: [Voice::default(); MAX_VOICES],
            keys: [(0, 0.0, 0.0); MAX_KEYS],
            nkeys: 0,
            porta_key: None,
            counter: 0,
            vibrato_phase: 0.0,
            rng: Rng::new(0x6d65_6f77),
            chorus: Chorus::new(sr),
        }
    }

    pub fn set_params(&mut self, params: MeowParams) {
        self.params = params;
    }

    pub fn set_tempo(&mut self, bpm: f32) {
        self.bpm = bpm;
    }

    /// The most recently started voice that is still sounding.
    fn last(&self) -> Option<usize> {
        self.voices
            .iter()
            .enumerate()
            .filter(|(_, v)| v.active && v.fade != Fade::Out)
            .max_by_key(|(_, v)| v.age)
            .map(|(i, _)| i)
    }

    fn alloc(&self) -> usize {
        self.voices
            .iter()
            .position(|v| !v.active)
            .or_else(|| {
                // Steal: prefer the oldest released voice, then the oldest.
                self.voices
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| (v.released_at.is_none(), v.age))
                    .map(|(i, _)| i)
            })
            .unwrap_or(0)
    }

    fn start_voice(&mut self, pitch: u8, vel: f32, from: Option<f32>, off_at: f64) -> usize {
        self.counter += 1;
        let i = self.alloc();
        self.voices[i].start(pitch, vel, from, off_at, self.sr, self.counter);
        i
    }

    /// One note at a time: hand over from the current voice to a new one,
    /// sliding from its pitch if `bend`.
    fn retrigger(&mut self, pitch: u8, vel: f32, bend: bool) {
        match self.last() {
            Some(old) => {
                let from = bend.then(|| self.voices[old].pitch(&self.params, self.sr));
                let v = &mut self.voices[old];
                v.release();
                v.fade = Fade::Out;
                v.fade_n = 0;
                let new = self.start_voice(pitch, vel, from, f64::INFINITY);
                self.voices[new].fade = Fade::In;
            }
            None => {
                self.start_voice(pitch, vel, None, f64::INFINITY);
            }
        }
    }

    pub fn note_on(&mut self, pitch: u8, vel: f32, off_at: f64) {
        if self.nkeys == MAX_KEYS {
            self.keys.copy_within(1.., 0);
            self.nkeys -= 1;
        }
        self.keys[self.nkeys] = (pitch, vel, off_at);
        self.nkeys += 1;
        if self.params.polycat {
            let from = self
                .porta_key
                .map(|k| crate::util::midi_to_hz(k as f32).log2());
            self.porta_key = Some(pitch);
            self.start_voice(pitch, vel, from, off_at);
        } else {
            // A voice still held slides into the new note; a released one
            // doesn't, but each note starts a fresh meow either way.
            let bend = self
                .last()
                .is_some_and(|i| self.voices[i].released_at.is_none());
            self.retrigger(pitch, vel, bend);
        }
    }

    /// Lift the held key at `index`.
    fn key_off(&mut self, index: usize) {
        let (pitch, _, _) = self.keys[index];
        self.keys.copy_within(index + 1..self.nkeys, index);
        self.nkeys -= 1;
        if self.porta_key == Some(pitch) {
            self.porta_key = None;
        }
        if self.params.polycat {
            return;
        }
        if self.nkeys == 0 {
            for v in self.voices.iter_mut().filter(|v| v.active) {
                v.release();
            }
        } else if index == self.nkeys {
            // The top key went up: meow again on the one still held under it.
            let (pitch, vel, _) = self.keys[self.nkeys - 1];
            if self.last().is_some() {
                self.retrigger(pitch, vel, true);
            }
        }
    }

    pub fn release_due(&mut self, tick: f64) {
        while let Some(i) = self.keys[..self.nkeys].iter().position(|k| k.2 <= tick) {
            self.key_off(i);
        }
        for v in self
            .voices
            .iter_mut()
            .filter(|v| v.active && v.off_at <= tick)
        {
            v.off_at = f64::INFINITY;
            v.release();
        }
    }

    pub fn live_off(&mut self, pitch: u8) {
        if let Some(i) = self.keys[..self.nkeys]
            .iter()
            .position(|k| k.0 == pitch && k.2.is_infinite())
        {
            self.key_off(i);
        }
        if self.params.polycat {
            for v in self
                .voices
                .iter_mut()
                .filter(|v| v.active && v.note == pitch && v.off_at.is_infinite())
            {
                v.release();
            }
        }
    }

    pub fn all_off(&mut self) {
        self.nkeys = 0;
        self.porta_key = None;
        for v in self.voices.iter_mut().filter(|v| v.active) {
            v.off_at = f64::INFINITY;
            v.release();
        }
    }

    pub fn render(&mut self, l: &mut [f32], r: &mut [f32]) {
        let p = self.params;
        let sr = self.sr;
        let rate = self.bpm / 60.0 / p.vibrato_beats.max(0.01);
        let gain = LEVEL * p.gain;
        for i in 0..l.len() {
            // Triangle, from -1 at phase 0.
            let ph = self.vibrato_phase;
            let lfo = if ph < 0.5 {
                4.0 * ph - 1.0
            } else {
                3.0 - 4.0 * ph
            };
            self.vibrato_phase = (ph + rate / sr).fract();
            let vibrato = lfo * p.vibrato;
            let mut s = 0.0;
            for v in self.voices.iter_mut().filter(|v| v.active) {
                let noise = self.rng.noise();
                s += v.next(&p, sr, vibrato, noise);
            }
            if p.chorus > 0.0 {
                let c = self.chorus.next(s, sr);
                s += (c - s) * p.chorus;
            }
            l[i] += s * gain;
            r[i] += s * gain;
        }
        for v in self.voices.iter_mut().filter(|v| v.active) {
            if v.is_done(&p, sr) {
                v.active = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sustain_knob_matches_nyasynth() {
        assert!((sustain_level(-15.0) - 0.6548).abs() < 1e-3);
        assert!((sustain_level(0.0) - 1.0).abs() < 1e-6);
        assert!(sustain_level(-24.0).abs() < 1e-6);
    }

    #[test]
    fn sweep_spans_the_measured_range() {
        // Measured on the original: ~350 Hz → ~8 kHz at full velocity, a
        // constant ~350 Hz at zero velocity.
        let p = MeowParams::default();
        let full = ease_in_expo(1.0);
        assert!((cutoff(&p, full, 0.0) - 350.0).abs() < 0.5);
        assert!((cutoff(&p, full, 1.0) - 7350.0).abs() < 2.0);
        assert!((cutoff(&p, ease_in_expo(0.0), 1.0) - 350.0).abs() < 0.5);
        // The app's default velocity: a modest meow.
        let mid = cutoff(&p, ease_in_expo(0.8), 1.0);
        assert!((2000.0..2200.0).contains(&mid), "{mid}");
    }
}
