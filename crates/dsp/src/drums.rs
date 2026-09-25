use orchestre_core::{DrumParams, DrumPiece};

use crate::filter::Svf;
use crate::util::{Rng, TAU, pan_gains, soft_clip};

const MAX_VOICES: usize = 16;

/// Inharmonic square-wave cluster, the classic analog cymbal recipe.
const METAL: [f32; 6] = [205.3, 304.4, 369.6, 522.7, 540.0, 800.0];

#[derive(Clone, Copy)]
struct Voice {
    active: bool,
    piece: DrumPiece,
    t: f32,
    vel: f32,
    /// Seconds after which the voice is silent enough to stop.
    length: f32,
    /// Seconds; amplitude decay time constant.
    decay: f32,
    choke: f32,
    phase: f32,
    phase2: f32,
    metal: [f32; 6],
    rng: Rng,
    f1: Svf,
    f2: Svf,
    pan: (f32, f32),
    tune: f32,
    punch: f32,
    snappy: f32,
    age: u64,
}

impl Default for Voice {
    fn default() -> Self {
        Voice {
            active: false,
            piece: DrumPiece::Kick,
            t: 0.0,
            vel: 0.0,
            length: 0.0,
            decay: 0.1,
            choke: 1.0,
            phase: 0.0,
            phase2: 0.0,
            metal: [0.0; 6],
            rng: Rng::new(1),
            f1: Svf::default(),
            f2: Svf::default(),
            pan: (0.707, 0.707),
            tune: 1.0,
            punch: 0.5,
            snappy: 0.5,
            age: 0,
        }
    }
}

/// Exponential decay with time constant `tau`.
#[inline]
fn ex(t: f32, tau: f32) -> f32 {
    (-t / tau).exp()
}

impl Voice {
    fn start(&mut self, piece: DrumPiece, vel: f32, p: &DrumParams, sr: f32, age: u64) {
        let tune = (2.0f32).powf(p.tune / 12.0);
        let d = p.decay.clamp(0.2, 4.0);
        let (decay, pan, f1, f2) = match piece {
            DrumPiece::Kick => (0.18 * d, 0.0, (0.0, 0.0), (0.0, 0.0)),
            DrumPiece::Snare => (0.07 * d, 0.05, (1800.0, 0.1), (0.0, 0.0)),
            DrumPiece::Clap => (0.09 * d, -0.1, (1100.0, 0.55), (0.0, 0.0)),
            DrumPiece::Rim => (0.012 * d, 0.15, (1700.0, 0.6), (0.0, 0.0)),
            DrumPiece::ClosedHat => (0.025 * d, 0.3, (7500.0, 0.2), (10000.0, 0.3)),
            DrumPiece::OpenHat => (0.16 * d, 0.3, (7000.0, 0.2), (10000.0, 0.3)),
            DrumPiece::LowTom => (0.16 * d, -0.35, (0.0, 0.0), (0.0, 0.0)),
            DrumPiece::MidTom => (0.13 * d, -0.1, (0.0, 0.0), (0.0, 0.0)),
            DrumPiece::HighTom => (0.11 * d, 0.2, (0.0, 0.0), (0.0, 0.0)),
            DrumPiece::Crash => (0.7 * d, -0.25, (5000.0, 0.1), (9000.0, 0.2)),
            DrumPiece::Cowbell => (0.12 * d, 0.25, (900.0, 0.5), (0.0, 0.0)),
            DrumPiece::Tambourine => (0.12 * d, -0.3, (7000.0, 0.2), (9500.0, 0.4)),
            DrumPiece::Shaker => (0.06 * d, 0.35, (6500.0, 0.3), (4000.0, 0.1)),
            DrumPiece::Claves => (0.025 * d, -0.2, (0.0, 0.0), (0.0, 0.0)),
            DrumPiece::HighConga => (0.12 * d, 0.3, (2000.0, 0.4), (0.0, 0.0)),
            DrumPiece::LowConga => (0.16 * d, 0.15, (1600.0, 0.4), (0.0, 0.0)),
            DrumPiece::Bongo => (0.07 * d, -0.35, (2500.0, 0.4), (0.0, 0.0)),
        };
        self.active = true;
        self.piece = piece;
        self.t = 0.0;
        self.vel = vel;
        self.decay = decay;
        self.length = decay * 7.0 + 0.05;
        self.choke = 1.0;
        self.phase = 0.0;
        self.phase2 = 0.0;
        self.metal = [0.0; 6];
        self.rng = Rng::new(0x1234_5678 ^ (age as u32).wrapping_mul(747_796_405));
        self.f1.reset();
        self.f2.reset();
        self.f1
            .set(f1.0 * if f1.0 > 0.0 { tune.sqrt() } else { 1.0 }, f1.1, sr);
        self.f2.set(f2.0, f2.1, sr);
        self.pan = pan_gains(pan);
        self.tune = tune;
        self.punch = p.punch;
        self.snappy = p.snappy;
        self.age = age;
    }

    #[inline]
    fn metal(&mut self, sr: f32, mult: f32) -> f32 {
        let mut s = 0.0;
        for (ph, f) in self.metal.iter_mut().zip(METAL) {
            *ph = (*ph + f * mult / sr).fract();
            s += if *ph < 0.5 { 1.0 } else { -1.0 };
        }
        s / 6.0
    }

    #[inline]
    fn tone(&mut self, freq: f32, sr: f32) -> f32 {
        self.phase = (self.phase + freq / sr).fract();
        (self.phase * TAU).sin()
    }

    #[inline]
    fn sample(&mut self, sr: f32) -> f32 {
        let t = self.t;
        let tune = self.tune;
        match self.piece {
            DrumPiece::Kick => {
                let f = 48.0 * tune + 110.0 * tune * ex(t, 0.035);
                let body = self.tone(f, sr) * ex(t, self.decay);
                let click = self.rng.noise() * ex(t, 0.003) * self.punch * 0.8;
                soft_clip((body * 1.4 + click) * 1.2)
            }
            DrumPiece::Snare => {
                let a = self.tone(185.0 * tune, sr);
                self.phase2 = (self.phase2 + 330.0 * tune / sr).fract();
                let b = (self.phase2 * TAU).sin();
                let body = (a * 0.6 + b * 0.4) * ex(t, 0.045) * (1.0 - self.snappy * 0.5);
                let n =
                    self.f1.high(self.rng.noise()) * ex(t, self.decay * 1.6) * (0.3 + self.snappy);
                body + n * 0.8
            }
            DrumPiece::Clap => {
                let n = self.f1.band(self.rng.noise()) * 2.5;
                // Three quick bursts, then a diffuse tail.
                let env = if t < 0.03 {
                    ex(t % 0.01, 0.003)
                } else {
                    ex(t - 0.03, self.decay)
                };
                n * env
            }
            DrumPiece::Rim => {
                let a = self.tone(1700.0 * tune, sr);
                self.phase2 = (self.phase2 + 480.0 * tune / sr).fract();
                let b = (self.phase2 * TAU).sin();
                let n = self.f1.band(self.rng.noise());
                ((a + b) * 0.5 + n) * ex(t, self.decay)
            }
            DrumPiece::ClosedHat | DrumPiece::OpenHat => {
                let m = self.metal(sr, 1.7 * tune.sqrt());
                let n = self.rng.noise() * 0.4;
                let s = self.f2.band(self.f1.high(m + n));
                s * 2.2 * ex(t, self.decay) * self.choke
            }
            DrumPiece::Crash => {
                let m = self.metal(sr, 2.3 * tune.sqrt());
                let n = self.rng.noise() * 0.8;
                let s = self.f1.high(m + n);
                (s + self.f2.band(s)) * 0.7 * ex(t, self.decay) * (1.0 - ex(t, 0.002))
            }
            DrumPiece::LowTom | DrumPiece::MidTom | DrumPiece::HighTom => {
                let base = match self.piece {
                    DrumPiece::LowTom => 95.0,
                    DrumPiece::MidTom => 135.0,
                    _ => 180.0,
                } * tune;
                let f = base * (1.0 + 0.6 * ex(t, 0.04));
                let body = self.tone(f, sr) * ex(t, self.decay);
                let n = self.rng.noise() * ex(t, 0.01) * 0.15;
                body + n
            }
            DrumPiece::Cowbell => {
                // The classic analog recipe: two detuned square waves.
                self.phase = (self.phase + 540.0 * tune / sr).fract();
                self.phase2 = (self.phase2 + 800.0 * tune / sr).fract();
                let sq = |ph: f32| if ph < 0.5 { 1.0 } else { -1.0 };
                let s = self.f1.band((sq(self.phase) + sq(self.phase2)) * 0.5);
                s * 1.1 * (0.6 * ex(t, 0.015) + 0.4 * ex(t, self.decay))
            }
            DrumPiece::Tambourine => {
                let m = self.metal(sr, 4.0);
                let n = self.rng.noise() * 0.6;
                let s = self.f2.band(self.f1.high(m + n));
                // Jingles rattle: a fast wobble on the decay.
                let rattle = 0.75 + 0.25 * (t * 90.0 * TAU).sin();
                s * 2.5 * ex(t, self.decay) * rattle
            }
            DrumPiece::Shaker => {
                let s = self.f2.high(self.f1.band(self.rng.noise()));
                // Soft attack: the beads take a moment to hit the shell.
                s * 2.4 * (1.0 - ex(t, 0.012)) * ex(t, self.decay)
            }
            DrumPiece::Claves => {
                let a = self.tone(2500.0 * tune, sr);
                self.phase2 = (self.phase2 + 5400.0 * tune / sr).fract();
                let b = (self.phase2 * TAU).sin();
                (a + b * 0.3) * ex(t, self.decay)
            }
            DrumPiece::HighConga | DrumPiece::LowConga | DrumPiece::Bongo => {
                let base = match self.piece {
                    DrumPiece::HighConga => 330.0,
                    DrumPiece::LowConga => 220.0,
                    _ => 420.0,
                } * tune;
                let f = base * (1.0 + 0.15 * ex(t, 0.02));
                let body = self.tone(f, sr) * ex(t, self.decay);
                // Slap of the hand on the skin.
                let slap = self.f1.band(self.rng.noise()) * ex(t, 0.006) * 1.5;
                body + slap
            }
        }
    }
}

pub struct DrumEngine {
    pub params: DrumParams,
    sr: f32,
    voices: [Voice; MAX_VOICES],
    counter: u64,
}

impl DrumEngine {
    pub fn new(params: DrumParams, sr: f32) -> Self {
        DrumEngine {
            params,
            sr,
            voices: [Voice::default(); MAX_VOICES],
            counter: 0,
        }
    }

    pub fn set_params(&mut self, params: DrumParams) {
        self.params = params;
    }

    pub fn note_on(&mut self, pitch: u8, vel: f32, _off_at: f64) {
        let Some(piece) = DrumPiece::from_pitch(pitch) else {
            return;
        };
        self.counter += 1;
        // A closed hat chokes a ringing open hat, like on a real kit.
        if piece == DrumPiece::ClosedHat {
            for v in self
                .voices
                .iter_mut()
                .filter(|v| v.active && v.piece == DrumPiece::OpenHat)
            {
                v.length = v.length.min(v.t + 0.02);
                v.choke = 0.5;
            }
        }
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
        self.voices[idx].start(piece, vel, &p, self.sr, self.counter);
    }

    /// Drum hits are one-shots; note ends are ignored.
    pub fn release_due(&mut self, _tick: f64) {}

    pub fn live_off(&mut self, _pitch: u8) {}

    pub fn all_off(&mut self) {}

    pub fn render(&mut self, l: &mut [f32], r: &mut [f32]) {
        let sr = self.sr;
        let dt = 1.0 / sr;
        let p = self.params;
        for v in self.voices.iter_mut().filter(|v| v.active) {
            let idx = DrumPiece::ALL
                .iter()
                .position(|&x| x == v.piece)
                .unwrap_or(0);
            let g = p.gain * p.level(idx) * (0.2 + 0.8 * v.vel) * 0.7;
            let (gl, gr) = (v.pan.0 * g, v.pan.1 * g);
            for i in 0..l.len() {
                // Short fade at the very end to avoid a click.
                let fade = ((v.length - v.t) / 0.01).clamp(0.0, 1.0);
                let s = v.sample(sr) * fade;
                l[i] += s * gl;
                r[i] += s * gr;
                v.t += dt;
            }
            if v.t >= v.length {
                v.active = false;
            }
        }
    }
}
