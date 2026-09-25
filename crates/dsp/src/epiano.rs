//! Electric piano: two-operator FM (a sine modulating a sine), whose
//! modulation fades after the attack, plus a short high "tine" ping.

use orchestre_core::EPianoParams;

use crate::util::{TAU, decay_coef, midi_to_hz};

const MAX_VOICES: usize = 16;

#[derive(Clone, Copy, Default)]
struct Voice {
    active: bool,
    note: u8,
    off_at: f64,
    age: u64,
    carrier: f32,
    modulator: f32,
    tine: f32,
    freq: f32,
    /// Modulation index (fades from the attack towards a warm tone).
    index: f32,
    index_floor: f32,
    index_coef: f32,
    ping: f32,
    ping_coef: f32,
    amp: f32,
    amp_coef: f32,
    released: bool,
    release_coef: f32,
    attack: f32,
}

pub struct EPianoEngine {
    pub params: EPianoParams,
    sr: f32,
    voices: [Voice; MAX_VOICES],
    counter: u64,
    trem: f32,
}

/// Ring time (T60) of a tine, from Shear's measurements of a 1974 Rhodes
/// Mark I (Q at E♭2..E♭6 converted to T60): long in the bass, flat through
/// the middle, short at the top. One instrument; interpolated in between.
fn ring_time(pitch: f32) -> f32 {
    const MEASURED: [(f32, f32); 5] = [
        (39.0, 27.0),
        (51.0, 10.0),
        (63.0, 11.0),
        (75.0, 7.7),
        (87.0, 3.1),
    ];
    let (first, last) = (MEASURED[0], MEASURED[MEASURED.len() - 1]);
    if pitch <= first.0 {
        return first.1;
    }
    if pitch >= last.0 {
        // Past E♭6, keep shortening at the same rate.
        let slope = (last.1 / MEASURED[3].1).ln() / 12.0;
        return (last.1 * (slope * (pitch - last.0)).exp()).max(0.8);
    }
    let i = MEASURED
        .iter()
        .position(|m| m.0 > pitch)
        .unwrap_or(MEASURED.len() - 1);
    let (a, b) = (MEASURED[i - 1], MEASURED[i]);
    let t = (pitch - a.0) / (b.0 - a.0);
    a.1 * (b.1 / a.1).powf(t)
}

impl EPianoEngine {
    pub fn new(params: EPianoParams, sr: f32) -> Self {
        EPianoEngine {
            params,
            sr,
            voices: [Voice::default(); MAX_VOICES],
            counter: 0,
            trem: 0.0,
        }
    }

    pub fn set_params(&mut self, params: EPianoParams) {
        self.params = params;
    }

    pub fn note_on(&mut self, pitch: u8, vel: f32, off_at: f64) {
        self.counter += 1;
        let (p, sr) = (self.params, self.sr);
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
        // High notes get less modulation, or they turn harsh.
        let register = (2.0f32)
            .powf(-(pitch as f32 - 60.0) / 30.0)
            .clamp(0.35, 1.6);
        let bark = p.tone * (0.4 + 0.6 * vel) * register;
        let ring = ring_time(pitch as f32) * p.decay;
        self.voices[idx] = Voice {
            active: true,
            note: pitch,
            off_at,
            age: self.counter,
            freq: midi_to_hz(pitch as f32),
            index: bark * 2.8,
            index_floor: bark * 0.6,
            index_coef: decay_coef(0.9, sr),
            ping: p.bell * vel * 0.9,
            ping_coef: decay_coef(0.08, sr),
            amp: 0.25 + 0.75 * vel,
            amp_coef: decay_coef(ring, sr),
            release_coef: decay_coef(0.15, sr),
            attack: 0.0,
            ..Voice::default()
        };
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
        let sr = self.sr;
        let gain = self.params.gain * 0.35;
        let depth = self.params.tremolo * 0.6;
        let attack_inc = 1.0 / (0.001 * sr);
        let n = l.len();
        // Stereo tremolo, shared by all notes (computed once per block).
        let mut pans = [(1.0f32, 1.0f32); 64];
        for p in pans.iter_mut().take(n) {
            let m = (self.trem * TAU).sin();
            *p = (1.0 - depth * (0.5 + 0.5 * m), 1.0 - depth * (0.5 - 0.5 * m));
            self.trem = (self.trem + 4.5 / sr).fract();
        }
        for v in self.voices.iter_mut().filter(|v| v.active) {
            let dt = v.freq / sr;
            for i in 0..n {
                let m = (v.modulator * TAU).sin() * v.index;
                let t = (v.tine * TAU).sin() * v.ping;
                let s = ((v.carrier + (m + t) / TAU) * TAU).sin();
                v.carrier = (v.carrier + dt).fract();
                v.modulator = (v.modulator + dt).fract();
                v.tine = (v.tine + dt * 14.0).fract();
                v.index = v.index_floor + (v.index - v.index_floor) * v.index_coef;
                v.ping *= v.ping_coef;
                v.amp *= if v.released {
                    v.release_coef
                } else {
                    v.amp_coef
                };
                v.attack = (v.attack + attack_inc).min(1.0);
                let out = s * v.amp * v.attack * gain;
                l[i] += out * pans[i].0;
                r[i] += out * pans[i].1;
            }
            if v.amp < 1e-4 {
                v.active = false;
            }
        }
    }
}
