use crate::util::{TAU, decay_coef};

/// Metronome click: a short sine blip, higher on the first beat of a bar.
pub struct Click {
    sr: f32,
    phase: f32,
    freq: f32,
    amp: f32,
    decay: f32,
}

impl Click {
    pub fn new(sr: f32) -> Self {
        Click {
            sr,
            phase: 0.0,
            freq: 1320.0,
            amp: 0.0,
            decay: decay_coef(0.06, sr),
        }
    }

    pub fn trigger(&mut self, accent: bool) {
        self.phase = 0.0;
        self.freq = if accent { 1760.0 } else { 1320.0 };
        self.amp = if accent { 0.45 } else { 0.3 };
    }

    #[inline]
    pub fn sample(&mut self) -> f32 {
        if self.amp < 1e-4 {
            return 0.0;
        }
        let s = (self.phase * TAU).sin() * self.amp;
        self.phase = (self.phase + self.freq / self.sr).fract();
        self.amp *= self.decay;
        s
    }
}
