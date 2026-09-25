//! The voice source: the Liljencrants–Fant (LF) model of the vocal folds'
//! airflow, set by Fant's single voice-quality parameter Rd (pressed 0.3 …
//! normal ~1 … breathy 2.7).
//!
//! One period of the flow derivative is solved numerically for 16 values
//! of Rd, then stored band-limited at several harmonic counts ("mipmaps"),
//! so the voice can read it at any pitch without aliasing and without
//! solving anything on the audio thread.

use std::f64::consts::PI;
use std::sync::OnceLock;

/// Samples per stored period.
pub const TABLE: usize = 1024;
/// Voice qualities stored, evenly spaced on the 0..1 quality axis.
pub const QUALITIES: usize = 16;
/// Harmonics kept at each band-limit level, highest first.
pub const LEVELS: [usize; 8] = [256, 128, 64, 32, 16, 8, 4, 2];

/// Harmonic amplitudes kept per quality, for estimating loudness.
pub const SPECTRUM: usize = 64;

pub struct Tables {
    /// `[quality][level][sample]`, peak magnitude 1.
    data: Vec<f32>,
    /// Amplitude of the first harmonics of the full-band table, per quality.
    spectrum: Vec<[f32; SPECTRUM]>,
    /// Fraction of the period during which the folds are open, per quality.
    open: [f32; QUALITIES],
}

/// Rd for a voice quality 0..1 (0 pressed, 0.5 normal, 1 breathy).
pub fn rd(quality: f32) -> f64 {
    0.3 * 9f64.powf(quality.clamp(0.0, 1.0) as f64)
}

/// LF timing (as fractions of one period) from Rd, after Fant (1995):
/// the peak of the flow `tp`, the instant of closure `te` and the
/// return-phase constant `ta`.
fn timing(rd: f64) -> (f64, f64, f64) {
    let ra = (-1.0 + 4.8 * rd) / 100.0;
    let rk = (22.4 + 11.8 * rd) / 100.0;
    let rg = rk / (4.0 * (0.11 * rd / (0.5 + 1.2 * rk) - ra));
    let tp = 1.0 / (2.0 * rg);
    let te = (tp * (1.0 + rk)).min(0.98);
    let ta = ra.max(1e-4).min((1.0 - te) * 0.9);
    (tp, te, ta)
}

/// One period of the LF flow derivative, `n` samples, closure level -1.
pub fn lf_period(rd: f64, n: usize) -> (Vec<f64>, f64) {
    let (tp, te, ta) = timing(rd);
    let wg = PI / tp;
    // Return phase: e * ta = 1 - exp(-e * (1 - te)).
    let mut eps = 1.0 / ta;
    for _ in 0..50 {
        let f = eps * ta - 1.0 + (-eps * (1.0 - te)).exp();
        let df = ta - (1.0 - te) * (-eps * (1.0 - te)).exp();
        let next = eps - f / df;
        if !next.is_finite() || next <= 0.0 {
            break;
        }
        if (next - eps).abs() < 1e-10 {
            eps = next;
            break;
        }
        eps = next;
    }
    let tail = (-eps * (1.0 - te)).exp();
    // The flow returns to where it started: the derivative's area is zero.
    // Solve for the open phase's growth `alpha` by bisection.
    let return_area = -(1.0 / (eps * ta)) * ((1.0 - tail) / eps - (1.0 - te) * tail);
    let open_area = |alpha: f64| {
        let e0 = -1.0 / ((alpha * te).exp() * (wg * te).sin());
        e0 * ((alpha * te).exp() * (alpha * (wg * te).sin() - wg * (wg * te).cos()) + wg)
            / (alpha * alpha + wg * wg)
    };
    let area = |alpha: f64| open_area(alpha) + return_area;
    let (mut lo, mut hi) = (-50.0, 500.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if (area(lo) < 0.0) == (area(mid) < 0.0) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let alpha = 0.5 * (lo + hi);
    let e0 = -1.0 / ((alpha * te).exp() * (wg * te).sin());
    let wave: Vec<f64> = (0..n)
        .map(|i| {
            let t = i as f64 / n as f64;
            if t <= te {
                e0 * (alpha * t).exp() * (wg * t).sin()
            } else {
                -(1.0 / (eps * ta)) * ((-eps * (t - te)).exp() - tail)
            }
        })
        .collect();
    (wave, te)
}

impl Tables {
    fn build() -> Tables {
        let n = TABLE;
        let cos: Vec<f64> = (0..n)
            .map(|i| (2.0 * PI * i as f64 / n as f64).cos())
            .collect();
        let sin: Vec<f64> = (0..n)
            .map(|i| (2.0 * PI * i as f64 / n as f64).sin())
            .collect();
        let max_h = LEVELS[0];
        let mut data = vec![0.0f32; QUALITIES * LEVELS.len() * n];
        let mut open = [0.0f32; QUALITIES];
        let mut spectrum = vec![[0.0f32; SPECTRUM]; QUALITIES];
        for q in 0..QUALITIES {
            let (wave, te) = lf_period(rd(q as f32 / (QUALITIES - 1) as f32), n);
            open[q] = te as f32;
            // Harmonics by a plain DFT (once, at startup).
            let harmonics: Vec<(f64, f64)> = (1..=max_h)
                .map(|k| {
                    let (mut re, mut im) = (0.0, 0.0);
                    for (i, &x) in wave.iter().enumerate() {
                        let j = (k * i) % n;
                        re += x * cos[j];
                        im += x * sin[j];
                    }
                    (re * 2.0 / n as f64, im * 2.0 / n as f64)
                })
                .collect();
            for (l, &h) in LEVELS.iter().enumerate() {
                let out = &mut data[(q * LEVELS.len() + l) * n..][..n];
                for (i, o) in out.iter_mut().enumerate() {
                    let mut s = 0.0;
                    for (k, &(re, im)) in harmonics[..h].iter().enumerate() {
                        let j = ((k + 1) * i) % n;
                        s += re * cos[j] + im * sin[j];
                    }
                    *o = s as f32;
                }
                let peak = out.iter().fold(0.0f32, |m, x| m.max(x.abs())).max(1e-6);
                out.iter_mut().for_each(|x| *x /= peak);
                if l == 0 {
                    for (k, a) in spectrum[q].iter_mut().enumerate() {
                        let (re, im) = harmonics[k];
                        *a = (re * re + im * im).sqrt() as f32 / peak;
                    }
                }
            }
        }
        Tables {
            data,
            spectrum,
            open,
        }
    }

    /// Band-limit level for a pitch: the most harmonics below `nyquist`.
    #[inline]
    pub fn level(f0: f32, nyquist: f32) -> usize {
        let allowed = (nyquist / f0.max(1.0)) as usize;
        LEVELS
            .iter()
            .position(|&h| h <= allowed)
            .unwrap_or(LEVELS.len() - 1)
    }

    /// Flow derivative at `phase` (0..1) for a quality (0..1).
    #[inline]
    pub fn sample(&self, quality: f32, level: usize, phase: f32) -> f32 {
        let qf = quality.clamp(0.0, 1.0) * (QUALITIES - 1) as f32;
        let q0 = (qf as usize).min(QUALITIES - 2);
        let qm = qf - q0 as f32;
        let x = phase.fract() * TABLE as f32;
        let i0 = x as usize % TABLE;
        let i1 = (i0 + 1) % TABLE;
        let f = x - x.floor();
        let read = |q: usize| {
            let t = &self.data[(q * LEVELS.len() + level) * TABLE..][..TABLE];
            t[i0] + (t[i1] - t[i0]) * f
        };
        let (a, b) = (read(q0), read(q0 + 1));
        a + (b - a) * qm
    }

    /// Amplitudes of the first harmonics (as stored) for a quality.
    pub fn spectrum(&self, quality: f32) -> &[f32; SPECTRUM] {
        let qf = quality.clamp(0.0, 1.0) * (QUALITIES - 1) as f32;
        &self.spectrum[(qf.round() as usize).min(QUALITIES - 1)]
    }

    /// Fraction of the period during which the folds are open.
    #[inline]
    pub fn open_phase(&self, quality: f32) -> f32 {
        let qf = quality.clamp(0.0, 1.0) * (QUALITIES - 1) as f32;
        self.open[(qf.round() as usize).min(QUALITIES - 1)]
    }
}

/// The shared tables, built on first use (a few milliseconds).
pub fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(Tables::build)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lf_pulses_are_balanced_and_close_at_minus_one() {
        for q in [0.0, 0.3, 0.5, 0.8, 1.0] {
            let (wave, te) = lf_period(rd(q), 4096);
            let mean = wave.iter().sum::<f64>() / wave.len() as f64;
            let min = wave.iter().cloned().fold(f64::MAX, f64::min);
            assert!(mean.abs() < 2e-3, "q {q}: flow doesn't return ({mean})");
            assert!((min + 1.0).abs() < 0.02, "q {q}: closure at {min}");
            assert!(te > 0.3 && te < 1.0, "q {q}: te {te}");
            assert!(wave.iter().all(|x| x.is_finite()));
        }
    }

    #[test]
    fn breathier_voices_are_darker() {
        // Energy above the 10th harmonic, relative to the whole.
        let brightness = |q: f32| {
            let (wave, _) = lf_period(rd(q), 2048);
            let n = wave.len();
            let power = |k: usize| {
                let (mut re, mut im) = (0.0, 0.0);
                for (i, x) in wave.iter().enumerate() {
                    let a = 2.0 * PI * (k * i) as f64 / n as f64;
                    re += x * a.cos();
                    im += x * a.sin();
                }
                re * re + im * im
            };
            let high: f64 = (10..60).map(power).sum();
            let all: f64 = (1..60).map(power).sum();
            high / all
        };
        let (pressed, normal, breathy) = (brightness(0.0), brightness(0.5), brightness(1.0));
        assert!(
            pressed > normal && normal > breathy,
            "{pressed} {normal} {breathy}"
        );
    }

    #[test]
    fn tables_are_band_limited_and_normalized() {
        let t = tables();
        assert_eq!(Tables::level(50.0, 22050.0), 0);
        // At 1 kHz, 22 harmonics fit below 22 kHz: the 16-harmonic table.
        assert_eq!(Tables::level(1000.0, 22050.0), 4);
        assert_eq!(Tables::level(20000.0, 22050.0), LEVELS.len() - 1);
        for level in 0..LEVELS.len() {
            let peak = (0..TABLE)
                .map(|i| t.sample(7.0 / 15.0, level, i as f32 / TABLE as f32).abs())
                .fold(0.0f32, f32::max);
            assert!((peak - 1.0).abs() < 0.05, "level {level}: {peak}");
        }
        assert!(
            t.open_phase(0.0) < t.open_phase(1.0),
            "breathy voices stay open longer"
        );
    }
}
