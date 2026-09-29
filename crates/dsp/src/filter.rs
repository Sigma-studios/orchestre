use std::f32::consts::PI;

use orchestre_core::FilterType;

use crate::util::soft_clip;

/// Topology-preserving-transform state variable filter (Cytomic/Simper).
#[derive(Clone, Copy, Debug, Default)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    k: f32,
}

impl Svf {
    /// `resonance` in 0..1.
    pub fn set(&mut self, cutoff: f32, resonance: f32, sr: f32) {
        let fc = cutoff.clamp(20.0, sr * 0.45);
        let g = (PI * fc / sr).tan();
        self.k = 2.0 - 1.94 * resonance.clamp(0.0, 1.0);
        self.a1 = 1.0 / (1.0 + g * (g + self.k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }

    /// Set cutoff and quality factor directly (for narrow resonances).
    pub fn set_q(&mut self, cutoff: f32, q: f32, sr: f32) {
        let fc = cutoff.clamp(20.0, sr * 0.45);
        let g = (PI * fc / sr).tan();
        self.k = 1.0 / q.max(0.1);
        self.a1 = 1.0 / (1.0 + g * (g + self.k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }

    pub fn reset(&mut self) {
        self.ic1 = 0.0;
        self.ic2 = 0.0;
    }

    /// Returns (low, band, high).
    #[inline]
    pub fn process(&mut self, v0: f32) -> (f32, f32, f32) {
        let v3 = v0 - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v2, v1, v0 - self.k * v1 - v2)
    }

    #[inline]
    pub fn low(&mut self, x: f32) -> f32 {
        self.process(x).0
    }

    #[inline]
    pub fn band(&mut self, x: f32) -> f32 {
        self.process(x).1
    }

    #[inline]
    pub fn high(&mut self, x: f32) -> f32 {
        self.process(x).2
    }

    /// Band-pass with unity gain at the centre frequency.
    #[inline]
    pub fn band_norm(&mut self, x: f32) -> f32 {
        self.process(x).1 * self.k
    }
}

/// Four-pole transistor ladder (Moog-style), after Zavalishin's
/// zero-delay-feedback form in "The Art of VA Filter Design": the feedback
/// is solved linearly, then the stage input is soft-clipped, which keeps it
/// stable up to self-oscillation and adds the ladder's warmth.
#[derive(Clone, Copy, Debug, Default)]
pub struct Ladder {
    s: [f32; 4],
    g: f32,
    k: f32,
}

impl Ladder {
    /// `resonance` in 0..1 (1 is at the edge of self-oscillation).
    pub fn set(&mut self, cutoff: f32, resonance: f32, sr: f32) {
        let fc = cutoff.clamp(20.0, sr * 0.45);
        let g = (PI * fc / sr).tan();
        self.g = g / (1.0 + g);
        self.k = 4.0 * resonance.clamp(0.0, 1.0) * 0.98;
    }

    pub fn reset(&mut self) {
        self.s = [0.0; 4];
    }

    /// Returns the 24 dB and 18 dB low-pass outputs, with some of the bass
    /// lost to resonance made up.
    #[inline]
    pub fn process(&mut self, x: f32) -> (f32, f32) {
        let g = self.g;
        let b = 1.0 - g;
        let s = &mut self.s;
        let (g2, g3) = (g * g, g * g * g);
        let g4 = g2 * g2;
        let sum = b * (g3 * s[0] + g2 * s[1] + g * s[2] + s[3]);
        let y4 = (g4 * x + sum) / (1.0 + self.k * g4);
        let mut input = 2.0 * soft_clip(0.5 * (x - self.k * y4));
        let mut out = [0.0f32; 4];
        for (i, o) in out.iter_mut().enumerate() {
            let v = (input - s[i]) * g;
            let y = v + s[i];
            s[i] = crate::util::flush(y + v);
            *o = y;
            input = y;
        }
        let makeup = 1.0 + 0.6 * self.k;
        (out[3] * makeup, out[2] * makeup)
    }
}

/// The classic synth's filter: any [`FilterType`].
#[derive(Clone, Copy, Debug, Default)]
pub struct MultiFilter {
    kind: FilterType,
    svf: Svf,
    ladder: Ladder,
}

impl MultiFilter {
    pub fn set(&mut self, kind: FilterType, cutoff: f32, resonance: f32, sr: f32) {
        self.kind = kind;
        match kind {
            FilterType::Ladder | FilterType::Acid => self.ladder.set(cutoff, resonance, sr),
            _ => self.svf.set(cutoff, resonance, sr),
        }
    }

    pub fn reset(&mut self) {
        self.svf.reset();
        self.ladder.reset();
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        match self.kind {
            FilterType::Smooth => self.svf.low(x),
            FilterType::HighPass => self.svf.high(x),
            FilterType::BandPass => self.svf.band_norm(x),
            FilterType::Ladder => self.ladder.process(x).0,
            FilterType::Acid => self.ladder.process(x).1,
        }
    }
}

/// One-pole low-pass, used for smoothing and damping.
#[derive(Clone, Copy, Debug, Default)]
pub struct OnePole {
    z: f32,
    a: f32,
}

impl OnePole {
    pub fn set(&mut self, cutoff: f32, sr: f32) {
        self.a = 1.0 - (-2.0 * PI * cutoff / sr).exp();
    }

    #[inline]
    pub fn low(&mut self, x: f32) -> f32 {
        self.z += self.a * (x - self.z);
        self.z
    }
}
