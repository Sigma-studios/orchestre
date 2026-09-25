use std::f32::consts::PI;

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
