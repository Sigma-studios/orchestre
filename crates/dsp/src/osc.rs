use orchestre_core::Wave;

use crate::util::TAU;

#[inline]
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        t + t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

#[inline]
fn frac(x: f32) -> f32 {
    x - x.floor()
}

/// Band-limited (PolyBLEP) oscillator. Phase is in cycles, 0..1.
#[derive(Clone, Copy, Debug, Default)]
pub struct Osc {
    pub phase: f32,
}

impl Osc {
    /// Next sample for frequency `dt` (cycles per sample), with phase offset
    /// `pm` (in cycles) for FM-style phase modulation.
    #[inline]
    pub fn next(&mut self, wave: Wave, dt: f32, pm: f32) -> f32 {
        let dt = dt.clamp(0.0, 0.49);
        let t = if pm == 0.0 {
            self.phase
        } else {
            frac(self.phase + pm)
        };
        let out = match wave {
            Wave::Sine => (t * TAU).sin(),
            Wave::Triangle => 1.0 - 4.0 * (t - 0.5).abs(),
            Wave::Saw => 2.0 * t - 1.0 - poly_blep(t, dt),
            Wave::Square => {
                let v = if t < 0.5 { 1.0 } else { -1.0 };
                v + poly_blep(t, dt) - poly_blep(frac(t + 0.5), dt)
            }
            Wave::Pulse => {
                let v = if t < 0.25 { 1.0 } else { -1.0 };
                v + poly_blep(t, dt) - poly_blep(frac(t + 0.75), dt) + 0.5
            }
        };
        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        out
    }
}
