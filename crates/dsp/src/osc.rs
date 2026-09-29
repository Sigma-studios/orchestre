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
        self.next_pw(wave, dt, pm, default_width(wave))
    }

    /// Like [`Osc::next`], with the pulse width of square and pulse waves
    /// (the fraction of the cycle spent high).
    #[inline]
    pub fn next_pw(&mut self, wave: Wave, dt: f32, pm: f32, width: f32) -> f32 {
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
            Wave::Square | Wave::Pulse => {
                let w = width;
                let v = if t < w { 1.0 } else { -1.0 };
                // Offset so the average stays at zero whatever the width.
                v + poly_blep(t, dt) - poly_blep(frac(t + 1.0 - w), dt) + 1.0 - 2.0 * w
            }
        };
        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        out
    }
}

/// Pulse width of a wave when not modulated.
#[inline]
pub fn default_width(wave: Wave) -> f32 {
    match wave {
        Wave::Pulse => 0.25,
        _ => 0.5,
    }
}

/// The wave without band-limiting.
#[inline]
fn naive(wave: Wave, t: f32, width: f32) -> f32 {
    match wave {
        Wave::Sine => (t * TAU).sin(),
        Wave::Triangle => 1.0 - 4.0 * (t - 0.5).abs(),
        Wave::Saw => 2.0 * t - 1.0,
        Wave::Square | Wave::Pulse => (if t < width { 1.0 } else { -1.0 }) + 1.0 - 2.0 * width,
    }
}

/// An oscillator hard-synced to another: it restarts its cycle whenever the
/// master's does. Both its own edges and the jumps at each restart are
/// smoothed with a two-point PolyBLEP, placed at the exact sub-sample time;
/// that needs one sample of lookahead, so the output is one sample late.
#[derive(Clone, Copy, Debug, Default)]
pub struct SyncOsc {
    pub phase: f32,
    held: f32,
}

/// PolyBLEP corrections gathered for the previous and the current sample.
#[derive(Default)]
struct Blep {
    before: f32,
    after: f32,
}

impl Blep {
    /// A step of height `h`, which happened `a` samples (0..1) before the
    /// current sample.
    #[inline]
    fn step(&mut self, h: f32, a: f32) {
        self.before += h * a * a * 0.5;
        self.after -= h * (1.0 - a) * (1.0 - a) * 0.5;
    }
}

impl SyncOsc {
    /// Steps of the wave's own edges while its phase moves from `p` by
    /// `span`, a segment that ends `end` samples before the current sample.
    #[inline]
    fn edges(wave: Wave, width: f32, p: f32, span: f32, dt: f32, end: f32, blep: &mut Blep) {
        let edges: &[(f32, f32)] = match wave {
            Wave::Saw => &[(1.0, -2.0)],
            Wave::Square | Wave::Pulse => &[(width, -2.0), (1.0, 2.0), (1.0 + width, -2.0)],
            _ => &[],
        };
        for &(at, h) in edges {
            if p < at && p + span >= at && dt > 0.0 {
                let since = (p + span - at) / dt + end;
                blep.step(h, since.clamp(0.0, 1.0));
            }
        }
    }

    /// Next sample. `reset` is `Some(a)` when the master restarted its
    /// cycle `a` samples (0..1) before this sample.
    #[inline]
    pub fn next(&mut self, wave: Wave, dt: f32, width: f32, reset: Option<f32>) -> f32 {
        let dt = dt.clamp(0.0, 0.49);
        let mut blep = Blep::default();
        let p0 = self.phase;
        let end = match reset {
            None => {
                Self::edges(wave, width, p0, dt, dt, 0.0, &mut blep);
                frac(p0 + dt)
            }
            Some(a) => {
                let a = a.clamp(0.0, 1.0);
                let span = (1.0 - a) * dt;
                Self::edges(wave, width, p0, span, dt, a, &mut blep);
                let h = naive(wave, 0.0, width) - naive(wave, frac(p0 + span), width);
                blep.step(h, a);
                Self::edges(wave, width, 0.0, a * dt, dt, 0.0, &mut blep);
                a * dt
            }
        };
        self.phase = end;
        let out = self.held + blep.before;
        self.held = naive(wave, end, width) + blep.after;
        out
    }
}
