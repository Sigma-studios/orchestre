use std::f32::consts::PI;

pub const TAU: f32 = 2.0 * PI;

#[inline]
pub fn midi_to_hz(pitch: f32) -> f32 {
    440.0 * (2.0f32).powf((pitch - 69.0) / 12.0)
}

/// Coefficient of a one-pole decay that falls to -60 dB in `seconds`.
#[inline]
pub fn decay_coef(seconds: f32, sr: f32) -> f32 {
    if seconds <= 0.0 {
        return 0.0;
    }
    (-6.9078 / (seconds * sr)).exp()
}

/// Cheap rational tanh approximation, exact at 0 and saturating to ±1.
#[inline]
pub fn soft_clip(x: f32) -> f32 {
    if x > 3.0 {
        1.0
    } else if x < -3.0 {
        -1.0
    } else {
        let x2 = x * x;
        x * (27.0 + x2) / (27.0 + 9.0 * x2)
    }
}

/// Equal-power pan gains for `pan` in -1..1.
#[inline]
pub fn pan_gains(pan: f32) -> (f32, f32) {
    let a = (pan.clamp(-1.0, 1.0) + 1.0) * 0.25 * PI;
    (a.cos(), a.sin())
}

/// Small, fast xorshift noise source.
#[derive(Clone, Copy, Debug)]
pub struct Rng(u32);

impl Default for Rng {
    fn default() -> Self {
        Rng(0x9e37_79b9)
    }
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        Rng(seed.max(1))
    }

    /// Uniform in -1..1.
    #[inline]
    pub fn noise(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// Denormal-safe flush for feedback paths.
#[inline]
pub fn flush(x: f32) -> f32 {
    if x.abs() < 1e-15 { 0.0 } else { x }
}
