//! Choir: a few detuned buzzy voices per note, shaped into vowels by five
//! resonant "formant" filters, like a human vocal tract.

use orchestre_core::{Adsr, ChoirParams, Wave};

use crate::env::Env;
use crate::filter::Svf;
use crate::osc::Osc;
use crate::util::{Rng, TAU, midi_to_hz, pan_gains};

const MAX_VOICES: usize = 16;
const MAX_SINGERS: usize = 4;

/// Sung-vowel formants (frequency Hz, level dB, bandwidth Hz), F1..F5,
/// from the Csound manual's formant tables (Appendix D), for "aah", "ooh"
/// and "eeh" in four voice types.
type Vowel = [(f32, f32, f32); 5];

const BASS: [Vowel; 3] = [
    [
        (600.0, 0.0, 60.0),
        (1040.0, -7.0, 70.0),
        (2250.0, -9.0, 110.0),
        (2450.0, -9.0, 120.0),
        (2750.0, -20.0, 130.0),
    ],
    [
        (350.0, 0.0, 40.0),
        (600.0, -20.0, 80.0),
        (2400.0, -32.0, 100.0),
        (2675.0, -28.0, 120.0),
        (2950.0, -36.0, 120.0),
    ],
    [
        (250.0, 0.0, 60.0),
        (1750.0, -30.0, 90.0),
        (2600.0, -16.0, 100.0),
        (3050.0, -22.0, 120.0),
        (3340.0, -28.0, 120.0),
    ],
];
const TENOR: [Vowel; 3] = [
    [
        (650.0, 0.0, 80.0),
        (1080.0, -6.0, 90.0),
        (2650.0, -7.0, 120.0),
        (2900.0, -8.0, 130.0),
        (3250.0, -22.0, 140.0),
    ],
    [
        (350.0, 0.0, 40.0),
        (600.0, -20.0, 60.0),
        (2700.0, -17.0, 100.0),
        (2900.0, -14.0, 120.0),
        (3300.0, -26.0, 120.0),
    ],
    [
        (290.0, 0.0, 40.0),
        (1870.0, -15.0, 90.0),
        (2800.0, -18.0, 100.0),
        (3250.0, -20.0, 120.0),
        (3540.0, -30.0, 120.0),
    ],
];
const ALTO: [Vowel; 3] = [
    [
        (800.0, 0.0, 80.0),
        (1150.0, -4.0, 90.0),
        (2800.0, -20.0, 120.0),
        (3500.0, -36.0, 130.0),
        (4950.0, -60.0, 140.0),
    ],
    [
        (325.0, 0.0, 50.0),
        (700.0, -12.0, 60.0),
        (2530.0, -30.0, 170.0),
        (3500.0, -40.0, 180.0),
        (4950.0, -64.0, 200.0),
    ],
    [
        (350.0, 0.0, 50.0),
        (1700.0, -20.0, 100.0),
        (2700.0, -30.0, 120.0),
        (3700.0, -36.0, 150.0),
        (4950.0, -60.0, 200.0),
    ],
];
const SOPRANO: [Vowel; 3] = [
    [
        (800.0, 0.0, 80.0),
        (1150.0, -6.0, 90.0),
        (2900.0, -32.0, 120.0),
        (3900.0, -20.0, 130.0),
        (4950.0, -50.0, 140.0),
    ],
    [
        (325.0, 0.0, 50.0),
        (700.0, -16.0, 60.0),
        (2700.0, -35.0, 170.0),
        (3800.0, -40.0, 180.0),
        (4950.0, -60.0, 200.0),
    ],
    [
        (270.0, 0.0, 60.0),
        (2140.0, -12.0, 90.0),
        (2950.0, -26.0, 100.0),
        (3900.0, -26.0, 120.0),
        (4950.0, -44.0, 120.0),
    ],
];

/// Voice types and the note (MIDI) at the middle of their range.
const VOICE_TYPES: [(f32, &[Vowel; 3]); 4] = [
    (48.0, &BASS),
    (57.0, &TENOR),
    (65.0, &ALTO),
    (72.0, &SOPRANO),
];

fn lerp_vowel(a: &Vowel, b: &Vowel, t: f32) -> Vowel {
    std::array::from_fn(|i| {
        let (x, y) = (a[i], b[i]);
        (
            x.0 + (y.0 - x.0) * t,
            x.1 + (y.1 - x.1) * t,
            x.2 + (y.2 - x.2) * t,
        )
    })
}

/// Formants (frequency, linear level, bandwidth) for a vowel position
/// 0..1 (aah → ooh → eeh), sung at `note`: low notes by basses, high
/// notes by sopranos, blending between voice types in between.
fn formants(vowel: f32, note: f32) -> [(f32, f32, f32); 5] {
    let x = vowel.clamp(0.0, 1.0) * 2.0;
    let (va, vb, vt) = if x <= 1.0 { (0, 1, x) } else { (1, 2, x - 1.0) };
    let at = |table: &[Vowel; 3]| lerp_vowel(&table[va], &table[vb], vt);
    let i = VOICE_TYPES
        .iter()
        .position(|(center, _)| note < *center)
        .unwrap_or(VOICE_TYPES.len());
    let v = if i == 0 {
        at(VOICE_TYPES[0].1)
    } else if i == VOICE_TYPES.len() {
        at(VOICE_TYPES[i - 1].1)
    } else {
        let (lo, hi) = (VOICE_TYPES[i - 1], VOICE_TYPES[i]);
        lerp_vowel(&at(lo.1), &at(hi.1), (note - lo.0) / (hi.0 - lo.0))
    };
    v.map(|(f, db, bw)| (f, 10f32.powf(db / 20.0), bw))
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
    formants: [Svf; 5],
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
        let norm = 1.0 / (singers as f32).sqrt();
        // The sung formant levels are low; keep the choir as loud as before.
        let gain = p.gain * 1.6;
        for v in self.voices.iter_mut().filter(|v| v.active) {
            // Pitch and formants are updated once per block.
            let mut dts = [0.0f32; MAX_SINGERS];
            for ((dt, vib), detune) in dts.iter_mut().zip(&mut v.vib).zip(&v.detune).take(singers) {
                // Singers: ~5.5–6.7 Hz (Prame), up to about ±1 semitone.
                let wobble = (*vib * TAU).sin() * p.vibrato * 0.75;
                *vib = (*vib + (5.8 + 0.4 * detune) * n as f32 / sr).fract();
                let pitch = v.note as f32 + wobble + detune * spread / 100.0;
                *dt = midi_to_hz(pitch) / sr;
            }
            let form = formants(p.vowel, v.note as f32);
            let mut ks = [0.0f32; 5];
            for (i, f) in v.formants.iter_mut().enumerate() {
                let q = form[i].0 / form[i].2;
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
