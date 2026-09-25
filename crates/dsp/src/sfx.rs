//! Sound effect voices. A voice is one play of a [`Sound`]: its random
//! variation is applied once, when it starts, then each layer's generator
//! runs until it has died out. Intensity can change while it plays, and
//! looping sounds hold or repeat until they are released.

use orchestre_core::Wave;
use orchestre_core::sfx::{
    BubbleParams, EngineParams, FilterMode, Generator, Layer, Looping, ModalParams, NoiseParams,
    PlayOpts, PulseParams, Shape, Sound, ThumpParams, ToneParams, VoiceParams, intensity_gain,
};

use crate::filter::Svf;
use crate::fx::Reverb;
use crate::osc::Osc;
use crate::util::{Rng, TAU, flush, pan_gains, soft_clip};

/// Sounds playing at once in the engine; the oldest is cut when full.
pub const MAX_SOUNDS: usize = 16;
/// Passes of a repeating sound that can overlap; the oldest is dropped.
const MAX_REPEATS: usize = 8;
/// Fade used when a sound is cut short, and at the very end of each layer.
const FADE: f32 = 0.005;
/// Seconds an edited version of a playing sound fades in over while the old
/// one fades out (see [`SfxVoice::swapped`]): long enough to hide the jump in
/// every oscillator's phase, short enough to sound instant.
pub const XFADE: f32 = 0.02;
/// Filters are retuned every this many samples while sweeping.
const RETUNE: u32 = 16;
/// Voices render in chunks of this many samples; live changes (intensity,
/// repeats) apply per chunk.
const CHUNK: usize = 64;
/// Seconds for a live intensity change to settle.
const SMOOTH: f32 = 0.03;

impl Rng {
    /// Uniform in 0..1.
    #[inline]
    fn unit(&mut self) -> f32 {
        (self.noise() + 1.0) * 0.5
    }
}

/// Amplitude of a [`Shape`] at time `t`.
#[inline]
fn envelope(s: &Shape, t: f32) -> f32 {
    if t < s.attack {
        return t / s.attack;
    }
    let t = t - s.attack;
    if t < s.hold {
        return 1.0;
    }
    decay(s.decay, t - s.hold)
}

/// Exponential fade reaching -60 dB after `secs`.
#[inline]
fn decay(secs: f32, t: f32) -> f32 {
    if t >= secs {
        0.0
    } else {
        (-6.9078 * t / secs.max(1e-4)).exp()
    }
}

/// Pitch slide from `a` to `b` over `time` seconds, exponential.
#[inline]
fn slide(a: f32, b: f32, time: f32, t: f32) -> f32 {
    let (a, b) = (a.max(1.0), b.max(1.0));
    a * (b / a).powf((t / time.max(1e-3)).min(1.0))
}

/// Filtered noise loses loudness as the band narrows; bring it back up.
fn noise_makeup(filter: FilterMode, cutoff: f32) -> f32 {
    match filter {
        FilterMode::Low => (3000.0 / cutoff).sqrt().clamp(1.0, 5.0),
        FilterMode::Band => 1.5 * (2500.0 / cutoff).sqrt().clamp(0.8, 4.0),
        FilterMode::High => 1.0,
    }
}

/// Live changes to a layer: pitch and brightness multipliers.
#[derive(Clone, Copy)]
struct Mods {
    pitch: f32,
    bright: f32,
    /// Crackle grains per second, relative: harder plays are denser
    /// (heavier rain, more debris).
    density: f32,
    /// Speed of pulses (see [`PlayOpts::rate`]).
    rate: f32,
}

/// A smooth random drift in -1..1, heading somewhere new about `rate`
/// times a second.
struct Drift {
    value: f32,
    target: f32,
    left: u32,
    period: u32,
    coef: f32,
}

impl Drift {
    fn new(rate: f32, sr: f32) -> Self {
        let period = (sr / rate.max(0.01)) as u32;
        Drift {
            value: 0.0,
            target: 0.0,
            left: 0,
            period: period.max(1),
            coef: 1.0 - (-1.0 / (period.max(1) as f32 * 0.5)).exp(),
        }
    }

    #[inline]
    fn next(&mut self, rng: &mut Rng) -> f32 {
        if self.left == 0 {
            self.target = rng.noise();
            self.left = self.period;
        }
        self.left -= 1;
        self.value += (self.target - self.value) * self.coef;
        self.value
    }
}

struct NoiseVoice {
    p: NoiseParams,
    rng: Rng,
    svf: Svf,
    makeup: f32,
    grain: f32,
    grain_coef: f32,
    grain_chance: f32,
    drift: Drift,
    n: u32,
    sr: f32,
}

impl NoiseVoice {
    fn new(p: NoiseParams, sr: f32, seed: u32) -> Self {
        NoiseVoice {
            p,
            rng: Rng::new(seed),
            svf: Svf::default(),
            makeup: 1.0,
            grain: 0.0,
            grain_coef: (-1.0 / (0.002 * sr)).exp(),
            grain_chance: p.density.max(0.0) / sr,
            drift: Drift::new(p.wobble_rate, sr),
            n: 0,
            sr,
        }
    }

    #[inline]
    fn sample(&mut self, t: f32, env: f32, m: Mods) -> f32 {
        let p = &self.p;
        let drift = if p.wobble > 0.0 {
            self.drift.next(&mut self.rng)
        } else {
            0.0
        };
        if self.n.is_multiple_of(RETUNE) {
            let len = p.shape.length().max(1e-3);
            let (a, b) = (p.cutoff_start.max(20.0), p.cutoff_end.max(20.0));
            let mut cutoff = a * (b / a).powf((t / len).min(1.0)) * m.pitch * m.bright;
            cutoff *= 2.0f32.powf(p.wobble * drift);
            self.svf.set(cutoff, p.resonance, self.sr);
            self.makeup = noise_makeup(p.filter, cutoff.max(20.0));
        }
        self.n += 1;
        let w = self.rng.noise();
        let mut src = w;
        if p.crackle > 0.0 {
            if self.rng.unit() < self.grain_chance * m.density {
                self.grain = 0.4 + 0.6 * self.rng.unit();
            }
            self.grain *= self.grain_coef;
            src = w * ((1.0 - p.crackle) + p.crackle * self.grain * 3.0);
        }
        let y = match p.filter {
            FilterMode::Low => self.svf.low(src),
            FilterMode::Band => self.svf.band(src),
            FilterMode::High => self.svf.high(src),
        };
        // Gusts: the level drifts along with the tone.
        let gust = 1.0 + 0.5 * p.wobble.min(1.0) * drift;
        y * self.makeup * env * gust
    }
}

struct ToneVoice {
    p: ToneParams,
    osc: Osc,
    mod_phase: f32,
    lp: Svf,
    bright: f32,
    held: f32,
    hold_left: u32,
    hold_len: u32,
    levels: f32,
    n: u32,
    sr: f32,
}

impl ToneVoice {
    fn new(p: ToneParams, sr: f32) -> Self {
        let mut lp = Svf::default();
        lp.set(p.cutoff, 0.0, sr);
        let crush = p.crush.clamp(0.0, 1.0);
        ToneVoice {
            p,
            osc: Osc::default(),
            mod_phase: 0.0,
            lp,
            bright: 1.0,
            held: 0.0,
            hold_left: 0,
            hold_len: 1 + (crush * crush * 16.0) as u32,
            levels: 2.0f32.powf(16.0 - crush * 13.0),
            n: 0,
            sr,
        }
    }

    #[inline]
    fn sample(&mut self, t: f32, env: f32, m: Mods) -> f32 {
        let p = &self.p;
        if self.n.is_multiple_of(RETUNE) && m.bright != self.bright {
            self.bright = m.bright;
            self.lp.set(p.cutoff * m.bright, 0.0, self.sr);
        }
        self.n += 1;
        let mut f = slide(p.pitch_start, p.pitch_end, p.glide, t) * m.pitch;
        if p.jump != 0.0 && t >= p.jump_time {
            f *= 2.0f32.powf(p.jump / 12.0);
        }
        if p.vibrato > 0.0 {
            f *= 2.0f32.powf(p.vibrato * (TAU * p.vibrato_rate * t).sin() / 12.0);
        }
        let pm = if p.fm > 0.0 {
            self.mod_phase = (self.mod_phase + f * p.fm_ratio / self.sr).fract();
            p.fm * 0.2 * (TAU * self.mod_phase).sin()
        } else {
            0.0
        };
        let x = self.osc.next(p.wave, f / self.sr, pm);
        let mut y = self.lp.low(x);
        if p.crush > 0.0 {
            // Sample-and-hold plus quantization: a retro, lo-fi chip.
            if self.hold_left == 0 {
                self.held = (y * self.levels).round() / self.levels;
                self.hold_left = self.hold_len;
            }
            self.hold_left -= 1;
            y = self.held;
        }
        let level = match p.wave {
            Wave::Square | Wave::Pulse | Wave::Saw => 0.5,
            Wave::Sine | Wave::Triangle => 0.8,
        };
        y * level * env
    }
}

/// One resonance, as a rotating, decaying phasor: two multiplies a sample.
#[derive(Clone, Copy)]
struct Partial {
    start: u32,
    re: f32,
    im: f32,
    cos: f32,
    sin: f32,
    decay: f32,
}

struct ModalVoice {
    partials: Vec<Partial>,
    /// Strike times (samples) and click levels, sorted.
    clicks: Vec<(u32, f32)>,
    next_click: usize,
    click: f32,
    click_coef: f32,
    hp: Svf,
    rng: Rng,
    n: u32,
}

impl ModalVoice {
    fn new(p: ModalParams, sr: f32, seed: u32) -> Self {
        let mut rng = Rng::new(seed);
        // The material's modes, then (for dense objects) irregular ones
        // climbing above them: 8–43% apart, like a plate's crowded modes.
        let count = (p.modes as usize).clamp(1, orchestre_core::sfx::MAX_MODES as usize);
        let mut ratios: Vec<f32> = p.material.ratios().into_iter().take(count).collect();
        while ratios.len() < count {
            let last = ratios[ratios.len() - 1];
            ratios.push(last * (1.08 + 0.35 * rng.unit()));
        }
        let lowest = ratios.iter().copied().fold(f32::MAX, f32::min);
        let hardness = p.hardness.clamp(0.0, 1.0);
        let tilt = 1.4 * (1.0 - hardness) + 0.2;
        // The main strike, then smaller ones scattered over `spread`.
        let mut strikes = vec![(0.0f32, 1.0f32, 1.0f32, 1.0f32)];
        for _ in 0..p.hits.min(orchestre_core::sfx::MAX_HITS) {
            let t = p.spread.max(0.0) * rng.unit().powf(1.5);
            let amp = (0.25 + 0.5 * rng.unit()) * (1.0 - 0.5 * t / p.spread.max(1e-3));
            let pitch = 2.0f32.powf(p.scatter * rng.noise() / 12.0);
            strikes.push((t, amp, pitch, 0.5));
        }
        strikes.sort_by(|a, b| a.0.total_cmp(&b.0));

        let mut partials = Vec::with_capacity(strikes.len() * ratios.len());
        let mut clicks = Vec::with_capacity(strikes.len());
        for &(t, amp, pitch, decay_mult) in &strikes {
            let start = (t * sr) as u32;
            let weights: Vec<f32> = (0..ratios.len())
                .map(|i| (1.0 / (1.0 + i as f32)).powf(tilt) * (0.7 + 0.6 * rng.unit()))
                .collect();
            let total: f32 = weights.iter().sum();
            // Dense objects already sound rough: no beating pairs on top.
            let beat = if count > 6 { 0.0 } else { p.material.beating() };
            for (&ratio, &w) in ratios.iter().zip(&weights) {
                // Irregular objects: resonances land anywhere near the ideal.
                let ratio = ratio * (1.0 + p.randomize.clamp(0.0, 1.0) * 0.4 * rng.noise());
                let f = p.pitch * ratio * pitch;
                if f >= sr * 0.45 || f < 20.0 {
                    continue;
                }
                // Damping grows with frequency: ring time ∝ (f/f0)^-damping.
                let tau = p.decay * decay_mult * (ratio / lowest).powf(-p.damping.clamp(0.0, 1.0));
                // Round objects: each resonance is a close pair that beats.
                let split: &[(f32, f32)] = if beat > 0.0 {
                    &[(-0.5, 0.5), (0.5, 0.5)]
                } else {
                    &[(0.0, 1.0)]
                };
                for &(offset, share) in split {
                    let w_rad = TAU * (f + offset * beat) / sr;
                    partials.push(Partial {
                        start,
                        // Starts at zero phase: struck resonances begin as sines.
                        re: amp * w / total * share,
                        im: 0.0,
                        cos: w_rad.cos(),
                        sin: w_rad.sin(),
                        decay: (-6.9078 / (tau.max(1e-3) * sr)).exp(),
                    });
                }
            }
            clicks.push((start, amp * hardness * 0.6));
        }
        let mut hp = Svf::default();
        hp.set(p.pitch * 2.0, 0.1, sr);
        ModalVoice {
            partials,
            clicks,
            next_click: 0,
            click: 0.0,
            click_coef: (-1.0 / (0.0008 * sr)).exp(),
            hp,
            rng,
            n: 0,
        }
    }

    #[inline]
    fn sample(&mut self) -> f32 {
        let n = self.n;
        self.n += 1;
        let mut sum = 0.0;
        for p in &mut self.partials {
            if n < p.start {
                break;
            }
            let re = (p.re * p.cos - p.im * p.sin) * p.decay;
            let im = (p.re * p.sin + p.im * p.cos) * p.decay;
            p.re = re;
            p.im = im;
            sum += im;
        }
        while let Some(&(at, level)) = self.clicks.get(self.next_click) {
            if at > n {
                break;
            }
            self.click = self.click.max(level);
            self.next_click += 1;
        }
        let click = if self.click > 1e-5 {
            let c = self.hp.high(self.rng.noise()) * self.click;
            self.click *= self.click_coef;
            c
        } else {
            0.0
        };
        // Resonances are sparse and short: bring them level with the others.
        (sum + click) * 2.0
    }
}

struct ThumpVoice {
    p: ThumpParams,
    phase: f32,
    rng: Rng,
    lp: Svf,
    bright: f32,
    makeup: f32,
    n: u32,
    sr: f32,
}

impl ThumpVoice {
    fn new(p: ThumpParams, sr: f32, seed: u32) -> Self {
        let mut lp = Svf::default();
        lp.set(p.cutoff, 0.1, sr);
        ThumpVoice {
            p,
            phase: 0.0,
            rng: Rng::new(seed),
            lp,
            bright: 1.0,
            makeup: noise_makeup(FilterMode::Low, p.cutoff),
            n: 0,
            sr,
        }
    }

    #[inline]
    fn sample(&mut self, t: f32, env: f32, m: Mods) -> f32 {
        let p = &self.p;
        if self.n.is_multiple_of(RETUNE) && m.bright != self.bright {
            self.bright = m.bright;
            let cutoff = p.cutoff * m.bright;
            self.lp.set(cutoff, 0.1, self.sr);
            self.makeup = noise_makeup(FilterMode::Low, cutoff.max(20.0));
        }
        self.n += 1;
        let f =
            (p.pitch_end + (p.pitch_start - p.pitch_end) * (-t / p.drop.max(1e-3)).exp()) * m.pitch;
        self.phase = (self.phase + f / self.sr).fract();
        let body = (self.phase * TAU).sin();
        let n = self.lp.low(self.rng.noise()) * self.makeup;
        let click = n * (-t / 0.0025).exp() * p.click * 1.5;
        let mut x = (body * (1.0 - p.noise * 0.5) + n * p.noise) * env + click;
        if p.blast > 0.0 {
            // Friedlander wave: overpressure falling through zero into a
            // longer, weaker suction.
            let ts = 0.25 / p.pitch_start.max(20.0);
            x += p.blast * 1.5 * (1.0 - t / ts) * (-t / ts).exp();
        }
        soft_clip(x * 1.2)
    }
}

/// One formant as a Klatt digital resonator: a two-pole filter with unity
/// gain at 0 Hz, so a cascade of them gets each vowel's formant levels
/// right by itself.
#[derive(Clone, Copy, Default)]
struct Resonator {
    a: f32,
    b: f32,
    c: f32,
    y1: f32,
    y2: f32,
    on: bool,
}

impl Resonator {
    fn set(&mut self, f: f32, bw: f32, sr: f32) {
        // Formants past the top of the band would alias; leave them out.
        self.on = f < sr * 0.45;
        if !self.on {
            return;
        }
        let r = (-std::f32::consts::PI * bw / sr).exp();
        self.c = -r * r;
        self.b = 2.0 * r * (TAU * f / sr).cos();
        self.a = 1.0 - self.b - self.c;
    }

    /// Magnitude of the response at `f` Hz.
    fn gain(&self, f: f32, sr: f32) -> f32 {
        if !self.on {
            return 1.0;
        }
        let w = TAU * f / sr;
        // |1 - b e^-jw - c e^-2jw|
        let re = 1.0 - self.b * w.cos() - self.c * (2.0 * w).cos();
        let im = self.b * w.sin() + self.c * (2.0 * w).sin();
        self.a.abs() / (re * re + im * im).sqrt().max(1e-9)
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        if !self.on {
            return x;
        }
        let y = self.a * x + self.b * self.y1 + self.c * self.y2;
        self.y2 = self.y1;
        self.y1 = crate::util::flush(y);
        y
    }
}

/// Klatt's anti-resonator: the inverse of a [`Resonator`], a notch where
/// that one has a peak. The nasal zero.
#[derive(Clone, Copy, Default)]
struct AntiResonator {
    a: f32,
    b: f32,
    c: f32,
    x1: f32,
    x2: f32,
    on: bool,
}

impl AntiResonator {
    fn set(&mut self, f: f32, bw: f32, sr: f32) {
        let mut r = Resonator::default();
        r.set(f, bw, sr);
        self.on = r.on;
        if !self.on {
            return;
        }
        // y = (x - b x1 - c x2) / a: the resonator's equation, solved for x.
        self.a = 1.0 / r.a;
        self.b = -r.b / r.a;
        self.c = -r.c / r.a;
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        if !self.on {
            return x;
        }
        let y = self.a * x + self.b * self.x1 + self.c * self.x2;
        self.x2 = self.x1;
        self.x1 = x;
        y
    }
}

/// Bandwidths of the nasal pole and zero (Klatt 1980, Table I).
const NASAL_WIDTH: f32 = 100.0;
/// Level of the parallel branch at full frication, against a voice's 0.25:
/// a hiss about as loud as a vowel. By ear.
const FRICATION_LEVEL: f32 = 0.35;

/// The two highest formants barely move between vowels.
/// (Klatt 1980's typical F4 and F5.)
const HIGH_FORMANTS: [f32; 2] = [3300.0, 3750.0];

/// Formant bandwidth for a frequency: Tappert, Martony and Fant (1963), as
/// corrected by Khodai-Joopari & Clermont (2002) and used in soundgen.
/// Below 250 Hz the formula is not defined; hold its value there.
fn formant_bandwidth(f: f32) -> f32 {
    if f >= 500.0 {
        50.0 * (1.0 + f * f / 6e6)
    } else {
        let f = f.max(250.0);
        52.08 * (1.0 + (f - 500.0).powi(2) / 1e5)
    }
}

/// Vocal folds: an LF pulse per vibration, with the vibration-to-vibration
/// irregularities of real voices.
struct Folds {
    phase: f32,
    step: f32,
    amp: f32,
    quality: f32,
    level: usize,
    /// Alternates for subharmonics.
    odd: bool,
}

impl Folds {
    fn new() -> Self {
        Folds {
            // Start a new vibration on the first sample.
            phase: 1.0,
            step: 0.0,
            amp: 1.0,
            quality: 0.5,
            level: 0,
            odd: false,
        }
    }

    /// Advance one sample; `start` is called when a new vibration begins,
    /// and returns (pitch in Hz, level, quality) for it.
    #[inline]
    fn next(&mut self, sr: f32, start: impl FnOnce(bool) -> (f32, f32, f32)) -> f32 {
        if self.phase >= 1.0 {
            self.phase -= self.phase.floor();
            self.odd = !self.odd;
            let (f0, amp, quality) = start(self.odd);
            self.step = f0.max(1.0) / sr;
            self.amp = amp;
            self.quality = quality;
            self.level = crate::glottis::Tables::level(f0, sr * 0.45);
        }
        let x = crate::glottis::tables().sample(self.quality, self.level, self.phase);
        self.phase += self.step;
        x * self.amp
    }
}

/// Formant voice (after Klatt): LF vocal-fold pulses and breath noise
/// through five formant resonators in cascade.
struct VoiceVoice {
    p: VoiceParams,
    folds: Folds,
    /// The second, independent pitch of a split voice.
    folds2: Folds,
    formants: [Resonator; 5],
    /// Klatt's nasal pole and zero, ahead of the formants, when set.
    nasal: Option<(Resonator, AntiResonator)>,
    /// The parallel branch, when there is frication: F1–F6, each with the
    /// gain that makes a level of 1 about as loud whatever its width.
    parallel: Option<[(Resonator, f32); 6]>,
    rng: Rng,
    /// Slow pitch wander that makes long notes less static.
    wander: Drift,
    n: u32,
    sr: f32,
    len: f32,
    /// Makes every voice about as loud, whatever its pitch and vowel.
    norm: f32,
}

/// Formant frequencies and bandwidths at a point of a voice.
fn formants(p: &VoiceParams, u: f32) -> ([f32; 5], [f32; 5]) {
    let size = p.size.clamp(0.25, 4.0);
    let freqs = match p.tract {
        orchestre_core::sfx::Tract::Human => {
            let [f1, f2, f3] =
                orchestre_core::sfx::vowel_formants(p.openness.at(u), p.frontness.at(u));
            [f1, f2, f3, HIGH_FORMANTS[0], HIGH_FORMANTS[1]]
        }
        orchestre_core::sfx::Tract::Dog => {
            // The jaw opening moves the dog's formants up together.
            let o = p.openness.at(u).clamp(0.0, 1.0);
            let [a, b] = orchestre_core::sfx::DOG_FORMANTS;
            let f = |k: usize| a[k] + (b[k] - a[k]) * o;
            [f(0), f(1), f(2), f(3), f(3) + 800.0]
        }
    };
    let widths = freqs.map(formant_bandwidth);
    // A bigger throat: lower and narrower resonances.
    let mut freqs = freqs.map(|f| f / size);
    let mut widths = widths.map(|w| w / size.sqrt());
    // Set by hand, in Hz as heard: the throat's size is already in them.
    let k = &p.klatt;
    for i in 0..5 {
        if let Some(f) = k.formants[i] {
            freqs[i] = f.at_log(u).clamp(50.0, 20_000.0);
            widths[i] = formant_bandwidth(freqs[i]) / size.sqrt();
        }
        if let Some(w) = k.bandwidths[i] {
            widths[i] = w.at_log(u).clamp(10.0, 5000.0);
        }
    }
    (freqs, widths)
}

/// F1–F5 and their bandwidths, in Hz, at a point `u` (0..1) of a voice: what
/// the editor starts a formant from when it is first set by hand.
pub fn voice_formants(p: &VoiceParams, u: f32) -> ([f32; 5], [f32; 5]) {
    formants(p, u)
}

/// RMS of a voice at a few points of its length, from the source's
/// harmonics and the formants' response to them.
fn voice_rms(p: &VoiceParams, sr: f32) -> f32 {
    let tables = crate::glottis::tables();
    let points = [0.15, 0.5, 0.85];
    let mut power = 0.0;
    for u in points {
        let f0 = p.pitch.at_log(u).max(20.0);
        let (freqs, widths) = formants(p, u);
        let mut rs = [Resonator::default(); 5];
        for ((r, f), w) in rs.iter_mut().zip(freqs).zip(widths) {
            r.set(f, w, sr);
        }
        for (k, &a) in tables.spectrum(p.quality.at(u)).iter().enumerate() {
            let f = f0 * (k + 1) as f32;
            if f >= sr * 0.45 {
                break;
            }
            let h: f32 = rs.iter().map(|r| r.gain(f, sr)).product();
            power += (a * h).powi(2) / 2.0;
        }
    }
    (power / points.len() as f32).sqrt()
}

impl VoiceVoice {
    fn new(p: VoiceParams, sr: f32, seed: u32) -> Self {
        crate::glottis::tables();
        VoiceVoice {
            p,
            folds: Folds::new(),
            folds2: Folds::new(),
            formants: [Resonator::default(); 5],
            nasal: p
                .klatt
                .nasal_zero
                .map(|_| (Resonator::default(), AntiResonator::default())),
            parallel: p
                .klatt
                .frication_on()
                .then_some([(Resonator::default(), 1.0); 6]),
            rng: Rng::new(seed),
            wander: Drift::new(4.0, sr),
            n: 0,
            sr,
            len: p.shape.length().max(1e-3),
            norm: 0.25 / voice_rms(&p, sr).max(1e-3),
        }
    }

    #[inline]
    fn sample(&mut self, t: f32, env: f32, m: Mods) -> f32 {
        let p = &self.p;
        let u = (t / self.len).min(1.0);
        if self.n.is_multiple_of(RETUNE) {
            let (freqs, widths) = formants(p, u);
            for ((r, f), w) in self.formants.iter_mut().zip(freqs).zip(widths) {
                r.set(f, w, self.sr);
            }
            if let (Some((pole, zero)), Some(fz)) = (&mut self.nasal, p.klatt.nasal_zero) {
                pole.set(
                    p.klatt.nasal_pole.clamp(100.0, 2000.0),
                    NASAL_WIDTH,
                    self.sr,
                );
                zero.set(fz.at_log(u).clamp(100.0, 4000.0), NASAL_WIDTH, self.sr);
            }
            if let Some(parallel) = &mut self.parallel {
                let f6 = p.klatt.f6.clamp(2000.0, 12_000.0);
                let bands = freqs.into_iter().chain([f6]).zip(p.klatt.parallel_widths);
                for ((r, gain), (f, w)) in parallel.iter_mut().zip(bands) {
                    let w = w.clamp(20.0, 5000.0);
                    r.set(f, w, self.sr);
                    // Peak to 1, then up by how little of a white hiss a band
                    // this wide lets through (its noise bandwidth is πw/2 of
                    // the sr/2 there is).
                    let share = (std::f32::consts::PI * w / self.sr).min(1.0);
                    *gain = 1.0 / (r.gain(f, self.sr).max(1e-6) * share.sqrt());
                }
            }
        }
        self.n += 1;
        let sr = self.sr;
        let rough = p.roughness.clamp(0.0, 1.0);
        let sub = p.subharmonics.clamp(0.0, 1.0);
        let wander = self.wander.next(&mut self.rng);
        let vib = p.vibrato * (TAU * p.vibrato_rate * t).sin();
        let f0 = p.pitch.at_log(u) * m.pitch * 2.0f32.powf((vib + rough * 0.8 * wander) / 12.0);
        let quality = p.quality.at(u).clamp(0.0, 1.0);
        let rng = &mut self.rng;
        let mut vibration = |odd: bool, f0: f32| {
            // Jitter and shimmer: each vibration a little off in length and
            // strength. Subharmonics: every other one weaker and longer.
            let mut f = f0 * (1.0 + rough * 0.05 * rng.noise());
            let mut a = 1.0 + rough * 0.35 * rng.noise();
            if sub > 0.0 {
                f *= if odd {
                    1.0 - sub * 0.18
                } else {
                    1.0 + sub * 0.18
                };
                a *= if odd { 1.0 - sub * 0.6 } else { 1.0 };
            }
            (f, a.max(0.0), quality)
        };
        let mut src = self.folds.next(sr, |odd| vibration(odd, f0));
        if p.split > 0.0 {
            let f = f0 * p.split_ratio.max(0.1);
            src += p.split.min(1.0) * 0.7 * self.folds2.next(sr, |odd| vibration(odd, f));
        }
        let k = &p.klatt;
        // AV. The default is a flat 1, and multiplying by it changes nothing.
        let voicing = k.voicing.at(u).clamp(0.0, 1.0);
        src *= voicing;
        // AVS: the voice bar, a sine at the folds' rate, under a v or a z.
        let bar = k.voice_bar.at(u).clamp(0.0, 1.0);
        if bar > 0.0 {
            src += bar * 0.5 * (TAU * self.folds.phase).sin();
        }
        // Breath: louder while the folds are open, and in lax voices. AH on
        // top, for an h.
        let open = self.folds.phase < crate::glottis::tables().open_phase(quality);
        let breath =
            (p.breath + k.aspiration.at(u).max(0.0) + (quality - 0.6).max(0.0) * 0.6).min(1.0);
        let aspiration = self.rng.noise() * breath * if open { 1.0 } else { 0.4 };
        let mut y = src * (1.0 - breath * 0.5) + aspiration * 0.15;
        if let Some((pole, zero)) = &mut self.nasal {
            y = zero.process(pole.process(y));
        }
        for r in &mut self.formants {
            y = r.process(y);
        }
        let rasp = (p.rasp > 0.0).then(|| {
            let d = p.rasp.min(1.0);
            let wave = 0.5 + 0.5 * (TAU * p.rasp_rate * t).sin();
            if p.trill > 0.0 {
                // A trill: open most of the time, snapping shut at the
                // bottom of each cycle. The higher the power, the shorter
                // and sharper the closure.
                1.0 - d * (1.0 - wave).powf(1.0 + 7.0 * p.trill.min(1.0))
            } else {
                1.0 - d + d * wave
            }
        });
        if let Some(rasp) = rasp {
            y *= rasp;
        }
        let mut out = y * self.norm;

        // The parallel branch: turbulence at a narrowing, through each
        // formant at its own level (signs alternating, as Klatt does, so
        // neighbouring peaks don't cancel), and straight through.
        if let Some(parallel) = &mut self.parallel {
            let af = k.frication.at(u).clamp(0.0, 1.0);
            // Voiced hiss comes in puffs, half as strong while the folds are
            // shut (Klatt 1980).
            let puffs = if open { 1.0 } else { 1.0 - 0.5 * voicing };
            let noise = self.rng.noise() * af * puffs;
            let mut hiss = 0.0;
            for (i, (r, peak)) in parallel.iter_mut().enumerate() {
                let level = k.parallel[i].clamp(0.0, 1.0) * *peak;
                let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
                hiss += sign * level * r.process(noise);
            }
            hiss += k.parallel[6].clamp(0.0, 1.0) * noise;
            out += hiss * FRICATION_LEVEL * rasp.unwrap_or(1.0);
        }
        out * env * p.loudness.at(u).max(0.0)
    }
}

#[derive(Clone, Copy, Default)]
struct Bubble {
    phase: f32,
    freq: f32,
    /// Pitch rise per second, as a fraction of the start pitch.
    rise: f32,
    level: f32,
    fall: f32,
    age: f32,
}

const MAX_BUBBLES: usize = 48;

/// sin(2πx) for x in 0..1, within ~0.1%: plenty for bubbles, several times
/// cheaper than `sin`.
#[inline]
fn fast_sin(x: f32) -> f32 {
    // Parabola, then one refinement step.
    let t = x * 2.0 - 1.0;
    let y = 4.0 * t * (1.0 - t.abs());
    -(0.225 * (y * y.abs() - y) + y)
}

/// A cloud of bubbles after van den Doel (2005): radius from a power law,
/// f = 3/r, decay 0.043f + 0.0014f^1.5 per second, pitch rising by
/// `rise`·decay·t.
struct BubblesVoice {
    p: BubbleParams,
    rng: Rng,
    bubbles: [Bubble; MAX_BUBBLES],
    sr: f32,
}

impl BubblesVoice {
    fn new(p: BubbleParams, sr: f32, seed: u32) -> Self {
        BubblesVoice {
            p,
            rng: Rng::new(seed),
            bubbles: [Bubble::default(); MAX_BUBBLES],
            sr,
        }
    }

    fn spawn(&mut self, pitch: f32) {
        let p = &self.p;
        let (lo, hi) = (
            p.size_min.max(0.1),
            p.size_max.max(p.size_min.max(0.1) + 0.01),
        );
        // Inverse transform sampling of p(r) ∝ r^-γ on [lo, hi].
        let g = p.small.clamp(0.0, 12.0);
        let u = self.rng.unit();
        let r = if (g - 1.0).abs() < 1e-3 {
            lo * (hi / lo).powf(u)
        } else {
            let e = 1.0 - g;
            (lo.powf(e) + u * (hi.powf(e) - lo.powf(e))).powf(1.0 / e)
        };
        let freq = 3000.0 / r * pitch;
        if freq >= self.sr * 0.45 {
            return;
        }
        let d = 0.043 * freq + 0.0014 * freq.powf(1.5);
        let slot = self
            .bubbles
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.level.total_cmp(&b.1.level))
            .map_or(0, |(i, _)| i);
        self.bubbles[slot] = Bubble {
            phase: 0.0,
            freq,
            rise: p.rise * d,
            // Bigger bubbles are louder.
            level: (r / hi).powf(0.7) * (0.5 + 0.5 * self.rng.unit()),
            fall: (-d / self.sr).exp(),
            age: 0.0,
        };
    }

    #[inline]
    fn sample(&mut self, env: f32, m: Mods) -> f32 {
        if env > 1e-3 && self.rng.unit() < self.p.rate.max(0.0) * m.density / self.sr {
            self.spawn(m.pitch);
        }
        let dt = 1.0 / self.sr;
        let mut sum = 0.0;
        for b in self.bubbles.iter_mut().filter(|b| b.level > 1e-4) {
            let f = b.freq * (1.0 + b.rise * b.age);
            b.phase = (b.phase + f * dt).fract();
            sum += fast_sin(b.phase) * b.level;
            b.level *= b.fall;
            b.age += dt;
        }
        // Bubbles start while the envelope is open; each rings out on its own.
        sum * 0.5
    }
}

const MAX_CYLINDERS: usize = 12;

/// Engine: each cylinder fires a combustion pulse in turn; the pulses run
/// through the exhaust pipe (a waveguide, its length setting the note's
/// colour) and a muffler. Pulses get shorter as the engine spins faster,
/// while the pipe stays the same: that's what makes the timbre change with
/// rpm. Intake hiss and valve ticks grow with speed.
struct EngineVoice {
    p: EngineParams,
    rng: Rng,
    /// Position within one engine cycle (all cylinders firing once), 0..1.
    crank: f32,
    fire_at: [f32; MAX_CYLINDERS],
    imbalance: [f32; MAX_CYLINDERS],
    pulse_t: [f32; MAX_CYLINDERS],
    pulse_amp: [f32; MAX_CYLINDERS],
    pipe: Vec<f32>,
    pipe_pos: usize,
    pipe_damp: crate::filter::OnePole,
    muffler: Svf,
    intake: Svf,
    tick: f32,
    tick_coef: f32,
    dc_x: f32,
    dc_y: f32,
    /// Each cylinder's header pipe ("extractor"), before they merge.
    headers: Vec<Vec<f32>>,
    header_pos: usize,
    /// Slow combustion-to-combustion variation.
    drift: Drift,
    /// Muffler chambers: short resonant sections in a row (as in
    /// waveguide engine models' muffler cascades).
    chambers: Vec<Vec<f32>>,
    chamber_pos: usize,
    /// The engine block shaking at crank rate.
    block_phase: f32,
    n: u32,
    sr: f32,
}

/// Muffler chamber lengths, seconds.
const CHAMBERS: [f32; 4] = [0.00023, 0.00034, 0.00045, 0.00057];

impl EngineVoice {
    fn new(p: EngineParams, sr: f32, seed: u32) -> Self {
        let mut rng = Rng::new(seed);
        let cyl = (p.cylinders as usize).clamp(1, MAX_CYLINDERS);
        let mut fire_at = [2.0f32; MAX_CYLINDERS];
        let mut imbalance = [0.0f32; MAX_CYLINDERS];
        for k in 0..cyl {
            // Even firing, with every other cylinder pulled earlier for an
            // uneven (lopey) engine.
            let shift = if k % 2 == 1 {
                p.uneven.clamp(0.0, 1.0) * 0.25 / cyl as f32
            } else {
                0.0
            };
            fire_at[k] = k as f32 / cyl as f32 - shift;
            // Cylinders never fire quite alike: the lopey rhythm at crank rate
            // that gives an engine its character.
            imbalance[k] = 1.0 + p.roughness.clamp(0.0, 1.0) * 0.45 * rng.noise();
        }
        // Round trip down the pipe and back (open end: the wave inverts).
        let len = ((2.0 * p.exhaust.clamp(0.1, 10.0) / 343.0) * sr) as usize;
        let mut pipe_damp = crate::filter::OnePole::default();
        pipe_damp.set(2500.0, sr);
        let mut intake = Svf::default();
        intake.set(900.0, 0.3, sr);
        EngineVoice {
            p,
            rng,
            crank: 0.999,
            fire_at,
            imbalance,
            pulse_t: [1.0; MAX_CYLINDERS],
            pulse_amp: [0.0; MAX_CYLINDERS],
            pipe: vec![0.0; len.max(2)],
            pipe_pos: 0,
            pipe_damp,
            muffler: Svf::default(),
            intake,
            tick: 0.0,
            tick_coef: (-1.0 / (0.001 * sr)).exp(),
            dc_x: 0.0,
            dc_y: 0.0,
            // Slightly different header lengths per cylinder (1.2–2.4 ms).
            headers: (0..cyl)
                .map(|k| vec![0.0; ((0.0012 + 0.0004 * (k % 4) as f32) * sr) as usize + 1])
                .collect(),
            header_pos: 0,
            drift: Drift::new(8.0, sr),
            chambers: CHAMBERS
                .iter()
                .map(|t| vec![0.0; (t * sr) as usize + 1])
                .collect(),
            chamber_pos: 0,
            block_phase: 0.0,
            n: 0,
            sr,
        }
    }

    #[inline]
    fn sample(&mut self, env: f32, m: Mods) -> f32 {
        let p = &self.p;
        let sr = self.sr;
        let cyl = (p.cylinders as usize).clamp(1, MAX_CYLINDERS);
        let rpm = (p.rpm * m.pitch).clamp(60.0, 20000.0);
        let cycle_hz = rpm / 60.0 / if p.two_stroke { 1.0 } else { 2.0 };
        let load = (p.load * m.bright.sqrt()).clamp(0.0, 1.0);
        if self.n.is_multiple_of(RETUNE) {
            self.muffler
                .set((300.0 + 3200.0 * load) * m.bright.sqrt(), 0.2, sr);
        }
        self.n += 1;

        // Fire the cylinders the crank passed.
        let prev = self.crank;
        self.crank += cycle_hz / sr;
        let wrapped = self.crank >= 1.0;
        if wrapped {
            self.crank -= 1.0;
        }
        let rough = p.roughness.clamp(0.0, 1.0);
        // Roughness and combustion noise are heard at idle; at speed the
        // firings blur into a smooth note (Farnell's engines stay clean).
        let calm = (1000.0 / rpm).min(1.0).sqrt();
        for k in 0..cyl {
            let at = self.fire_at[k].rem_euclid(1.0);
            let crossed = if wrapped {
                at > prev || at <= self.crank
            } else {
                at > prev && at <= self.crank
            };
            if crossed {
                self.pulse_t[k] = 0.0;
                let misfire = self.rng.unit() < rough * 0.02;
                self.pulse_amp[k] = if misfire {
                    0.0
                } else {
                    self.imbalance[k] * (1.0 + rough * calm * 0.3 * self.rng.noise())
                };
                self.tick = self.tick.max(p.mechanics * 0.3);
            }
        }

        // Each cylinder: its combustion pulse, then turbulent gas rushing
        // out while its exhaust valve is open (and in through the intake).
        // The flow noise is the rasp that stays at high rpm (after Baldan et
        // al.'s four-process cylinder model).
        let gap = 1.0 / (cycle_hz * cyl as f32);
        let a = (0.12 * gap).clamp(0.0006, 0.012);
        let dt = 1.0 / sr;
        let drift = 1.0 + rough * 0.15 * self.drift.next(&mut self.rng);
        let flow = load * (0.5 + (rpm / 3000.0).min(2.0)) * 0.5;
        let (exhaust_open, intake_open) = if p.two_stroke {
            ((0.15, 0.45), (0.6, 0.9))
        } else {
            ((0.25, 0.5), (0.5, 0.75))
        };
        let window = |local: f32, (lo, hi): (f32, f32)| {
            if local > lo && local < hi {
                let u = (local - lo) / (hi - lo);
                (std::f32::consts::PI * u).sin().powi(2)
            } else {
                0.0
            }
        };
        let mut x = 0.0;
        let mut intake_gate = 0.0;
        let hpos = self.header_pos;
        for k in 0..cyl {
            let t = self.pulse_t[k];
            let mut c = 0.0;
            if t < a * 12.0 {
                let u = t / a;
                c = self.pulse_amp[k] * drift * u * (1.0 - u).exp();
                // Harder work: rougher combustion.
                c += c.abs() * load * calm * 0.8 * self.rng.noise();
                self.pulse_t[k] = t + dt;
            }
            let local = (self.crank - self.fire_at[k]).rem_euclid(1.0);
            c += self.rng.noise() * flow * window(local, exhaust_open);
            intake_gate += window(local, intake_open);
            // Through this cylinder's header pipe.
            let h = &mut self.headers[k];
            let len = h.len();
            let back = h[hpos % len];
            let y = c + 0.35 * back;
            h[hpos % len] = flush(y);
            x += y;
        }
        self.header_pos = self.header_pos.wrapping_add(1);

        // The exhaust pipe.
        let len = self.pipe.len();
        let back = self.pipe[self.pipe_pos];
        let y = x - 0.45 * self.pipe_damp.low(back);
        self.pipe[self.pipe_pos] = flush(y);
        self.pipe_pos = (self.pipe_pos + 1) % len;
        // Through the muffler's chambers, each ringing a little.
        let mut m = y;
        let cpos = self.chamber_pos;
        for c in &mut self.chambers {
            let n = c.len();
            let back = c[cpos % n];
            let out = m + 0.3 * back;
            c[cpos % n] = flush(out);
            m = out * 0.8;
        }
        self.chamber_pos = self.chamber_pos.wrapping_add(1);
        let exhaust = self.muffler.low(m);
        // The block shakes at crank rate (and twice that): weight, not tone.
        let crank_hz = rpm / 60.0;
        self.block_phase = (self.block_phase + crank_hz / sr).fract();
        let block = ((self.block_phase * TAU).sin() * 0.6
            + (self.block_phase * 2.0 * TAU).sin() * 0.4)
            * (0.1 + 0.2 * rough)
            * x.abs().min(1.0);

        // Remove the pulses' DC offset.
        let hp = exhaust - self.dc_x + 0.995 * self.dc_y;
        self.dc_x = exhaust;
        self.dc_y = flush(hp);

        // Intake: air drawn in on each intake stroke, louder with speed.
        let intake = self.intake.band(self.rng.noise())
            * (p.mechanics * 0.6 + load * 0.4)
            * (0.3 + 0.7 * intake_gate.min(1.0))
            * (rpm / 4000.0).min(2.0)
            * 0.4;
        let tick = if self.tick > 1e-4 {
            let t = self.rng.noise() * self.tick;
            self.tick *= self.tick_coef;
            t
        } else {
            0.0
        };
        // Faster engines are louder.
        let revs = (rpm / 1000.0).powf(0.25);
        soft_clip((hp + block) * (1.2 + 1.2 * load) * revs + intake + tick) * env * 0.6
    }
}

/// A train of short noise bursts ringing a resonance.
struct PulseVoice {
    p: PulseParams,
    phase: f32,
    stretch: f32,
    exc: f32,
    exc_coef: f32,
    bp: Svf,
    gain: f32,
    /// The wooden body: broad resonances driven by the same pulses.
    body: [Svf; 6],
    /// The door panel: a bank of short delays (Farnell's "square panel").
    panel: Vec<f32>,
    panel_pos: usize,
    panel_hp: Svf,
    /// Seconds since the last pulse: friction builds up while stuck.
    since: f32,
    rng: Rng,
    n: u32,
    sr: f32,
}

/// Farnell's panel delays, in seconds.
const PANEL: [f32; 8] = [
    0.00452, 0.00506, 0.00627, 0.008, 0.00548, 0.00714, 0.01012, 0.016,
];
/// Q of each wooden-body resonance (Farnell: 1, 1, 2, 2, 3, 3).
const WOOD_Q: [f32; 6] = [1.0, 1.0, 2.0, 2.0, 3.0, 3.0];

/// Farnell's creaking-door body resonances (62.5 … 790 Hz, Q 1–3), as
/// fractions of the top one: they follow the pulses' tone.
const WOOD_BODY: [f32; 6] = [62.5, 125.0, 250.0, 395.0, 560.0, 790.0];

impl PulseVoice {
    fn new(p: PulseParams, sr: f32, seed: u32) -> Self {
        let q = 0.7 + p.resonance.clamp(0.0, 1.0).powi(2) * 40.0;
        PulseVoice {
            p,
            // Fire the first pulse right away.
            phase: 1.0,
            stretch: 1.0,
            exc: 0.0,
            exc_coef: (-1.0 / (p.burst.max(0.0002) * sr)).exp(),
            bp: Svf::default(),
            // The resonance rings louder as it narrows: even it out.
            gain: 2.5 / q.sqrt(),
            body: [Svf::default(); 6],
            panel: vec![0.0; (0.017 * sr) as usize + 2],
            panel_pos: 0,
            panel_hp: {
                let mut f = Svf::default();
                f.set(125.0, 0.0, sr);
                f
            },
            since: 1.0,
            rng: Rng::new(seed),
            n: 0,
            sr,
        }
    }

    #[inline]
    fn sample(&mut self, t: f32, env: f32, m: Mods) -> f32 {
        let p = &self.p;
        if self.n.is_multiple_of(RETUNE) {
            let q = 0.7 + p.resonance.clamp(0.0, 1.0).powi(2) * 40.0;
            self.bp.set_q(p.tone * m.bright, q, self.sr);
            if p.body > 0.0 {
                for ((f, q), r) in WOOD_BODY.iter().zip(WOOD_Q).zip(&mut self.body) {
                    r.set_q(f / 790.0 * p.tone * m.bright, q, self.sr);
                }
            }
        }
        self.n += 1;
        let rate = slide(p.rate_start, p.rate_end, p.glide, t) * m.pitch * m.rate;
        self.phase += rate / self.sr / self.stretch;
        let irr = p.irregular.clamp(0.0, 1.0);
        let mut impulse = 0.0;
        if self.phase >= 1.0 {
            self.phase -= self.phase.floor();
            let mut level = 1.0 - irr * 0.8 * self.rng.unit();
            // Now and then a pulse doesn't fire at all: an engine misfiring,
            // a creak catching.
            if self.rng.unit() < irr * 0.1 {
                level = 0.0;
            }
            if p.body > 0.0 {
                // Stick-slip: the longer it was stuck, the harder it slips
                // (Farnell).
                level *= (self.since * 10.0).min(1.0).sqrt();
            }
            self.since = 0.0;
            self.exc = level;
            impulse = level;
            self.stretch = (1.0 + irr * 0.6 * self.rng.noise()).max(0.3);
        }
        self.since += 1.0 / self.sr;
        // Each pulse: a burst of noise, or (grit 0) a clean decaying click.
        let grit = p.grit.clamp(0.0, 1.0);
        let x = (self.rng.noise() * grit + (1.0 - grit)) * self.exc + impulse;
        self.exc *= self.exc_coef;
        let mut y = self.bp.band(x) * self.gain * 3.0;
        if p.body > 0.0 {
            let wood: f32 = self.body.iter_mut().map(|r| r.band(x)).sum::<f32>() + x * 0.2;
            let len = self.panel.len();
            self.panel[self.panel_pos] = wood;
            let mut sum = 0.0;
            for d in PANEL {
                let back = (d * self.sr) as usize;
                sum += self.panel[(self.panel_pos + len - back.min(len - 1)) % len];
            }
            self.panel_pos = (self.panel_pos + 1) % len;
            let panel = self.panel_hp.high(sum / PANEL.len() as f32) * 4.0;
            y = y * (1.0 - p.body * 0.5) + panel * p.body * 1.2;
        }
        y * env
    }
}

enum GenVoice {
    Noise(NoiseVoice),
    Tone(ToneVoice),
    Modal(ModalVoice),
    Thump(ThumpVoice),
    Voice(Box<VoiceVoice>),
    Pulses(PulseVoice),
    Bubbles(Box<BubblesVoice>),
    Engine(Box<EngineVoice>),
}

struct LayerVoice {
    source: GenVoice,
    /// Seconds, relative to the start of the pass.
    start: f32,
    /// Infinite while a sustained layer holds.
    end: f32,
    shape: Option<Shape>,
    /// Holds after fading in, until released.
    sustain: bool,
    /// Local time of the release, and the level it fades out from.
    released: Option<(f32, f32)>,
    gains: (f32, f32),
    drive: f32,
    follow: f32,
}

impl LayerVoice {
    #[inline]
    fn level(&self, t: f32) -> f32 {
        let Some(s) = &self.shape else {
            return 1.0;
        };
        if !self.sustain {
            return envelope(s, t);
        }
        match self.released {
            Some((at, from)) if t >= at => from * decay(s.decay, t - at),
            _ if t < s.attack => t / s.attack,
            _ => 1.0,
        }
    }

    /// Start fading out; `t` is the pass's time.
    fn release(&mut self, t: f32) {
        if !self.sustain || self.released.is_some() {
            return;
        }
        let local = t - self.start;
        if local <= 0.0 {
            // Not started yet: it never will.
            self.end = self.start;
            return;
        }
        let decay = self.shape.map_or(0.0, |s| s.decay);
        self.released = Some((local, self.level(local)));
        self.end = t + decay;
    }
}

/// A pitch curve with its rises and falls scaled around its (geometric)
/// middle: 0 flattens it, 2 doubles every swoop.
fn swing(curve: &orchestre_core::sfx::Curve, amount: f32) -> orchestre_core::sfx::Curve {
    if (amount - 1.0).abs() < 1e-4 {
        return *curve;
    }
    let pts = curve.points();
    let mid = (pts.iter().map(|p| p[1].max(1.0).ln()).sum::<f32>() / pts.len() as f32).exp();
    curve.map(|v| mid * (v.max(1.0) / mid).powf(amount.max(0.0)))
}

/// Random variation, applied to one layer when a pass starts.
fn resolve(l: &Layer, variation: f32, opts: &PlayOpts, rng: &mut Rng) -> (f32, f32, Generator) {
    let j = &l.jitter;
    let v = variation.max(0.0);
    let i = opts.intensity.clamp(0.0, 2.0);
    // The play's own pitch is applied live (see `Pass::render`), except
    // for resonances, which are tuned when they're struck.
    let jitter = 2.0f32.powf(j.pitch * v * rng.noise() / 12.0);
    let pitch = jitter;
    let struck = jitter * 2.0f32.powf(opts.pitch / 12.0);
    let gain_db = j.volume * v * rng.noise();
    let start = (l.start + j.timing * v * rng.noise()).max(0.0);
    let tone = 2.0f32.powf(j.tone * v * rng.noise());
    // Harder plays ring longer; this is fixed when the pass starts.
    let length = ((1.0 + j.length * v * rng.noise()) * (0.6 + 0.4 * i)).max(0.1);
    let gain = l.gain * 10f32.powf(gain_db / 20.0);
    let stretch = |s: Shape| Shape::new(s.attack, s.hold * length, s.decay * length);
    let hz = |f: f32| (f * pitch).clamp(1.0, 20000.0);
    let bright = |f: f32| (f * pitch * tone).clamp(20.0, 20000.0);
    let g = match l.generator {
        Generator::Noise(p) => Generator::Noise(NoiseParams {
            shape: stretch(p.shape),
            cutoff_start: bright(p.cutoff_start),
            cutoff_end: bright(p.cutoff_end),
            ..p
        }),
        Generator::Tone(p) => Generator::Tone(ToneParams {
            shape: stretch(p.shape),
            pitch_start: hz(p.pitch_start),
            pitch_end: hz(p.pitch_end),
            glide: p.glide * length,
            jump_time: p.jump_time * length,
            cutoff: (p.cutoff * tone).clamp(20.0, 20000.0),
            ..p
        }),
        Generator::Modal(p) => Generator::Modal(ModalParams {
            pitch: (p.pitch * struck).clamp(1.0, 20000.0),
            decay: p.decay * length,
            spread: p.spread * length,
            hardness: (p.hardness + (tone.log2() + i - 1.0) * 0.3).clamp(0.0, 1.0),
            ..p
        }),
        Generator::Thump(p) => Generator::Thump(ThumpParams {
            shape: stretch(p.shape),
            pitch_start: hz(p.pitch_start),
            pitch_end: hz(p.pitch_end),
            cutoff: bright(p.cutoff),
            ..p
        }),
        Generator::Voice(p) => Generator::Voice(VoiceParams {
            shape: stretch(p.shape),
            pitch: swing(&p.pitch, opts.swing).map(hz),
            // Brightness variation reads as a slightly different throat.
            size: p.size / tone.sqrt(),
            ..p
        }),
        Generator::Pulses(p) => Generator::Pulses(PulseParams {
            shape: stretch(p.shape),
            rate_start: p.rate_start * pitch,
            rate_end: p.rate_end * pitch,
            glide: p.glide * length,
            tone: bright(p.tone),
            ..p
        }),
        Generator::Engine(p) => Generator::Engine(EngineParams {
            shape: stretch(p.shape),
            rpm: p.rpm * pitch,
            ..p
        }),
        // Higher pitch: smaller bubbles.
        Generator::Bubbles(p) => Generator::Bubbles(BubbleParams {
            shape: stretch(p.shape),
            size_min: p.size_min / pitch,
            size_max: p.size_max / pitch,
            ..p
        }),
    };
    (start, gain, g)
}

/// What can change while a sound plays.
#[derive(Clone, Copy)]
struct Live {
    intensity: f32,
    /// Semitones.
    pitch: f32,
    rate: f32,
}

/// One pass through a sound's layers.
struct Pass {
    layers: Vec<LayerVoice>,
    t: f32,
    /// What it was started with, so an edited version of it can be started
    /// the same way.
    opts: PlayOpts,
}

impl Pass {
    fn new(sound: &Sound, opts: &PlayOpts, sr: f32) -> Pass {
        let mut rng = Rng::new(opts.seed.wrapping_mul(0x9e37_79b9) ^ 0x5bd1_e995);
        let sustain = sound.looping == Looping::Sustain;
        let mut layers = Vec::with_capacity(sound.layers.len());
        for l in &sound.layers {
            // Draw the random numbers even for silent layers, so muting one
            // layer doesn't change how the others vary.
            let (start, gain, g) = resolve(l, sound.variation, opts, &mut rng);
            let seed = rng.noise().to_bits();
            if !sound.audible(l) {
                continue;
            }
            let shape = g.shape().copied();
            let sustain = sustain && shape.is_some();
            let end = if sustain {
                f32::INFINITY
            } else {
                start + g.length()
            };
            let source = match g {
                Generator::Noise(p) => GenVoice::Noise(NoiseVoice::new(p, sr, seed)),
                Generator::Tone(p) => GenVoice::Tone(ToneVoice::new(p, sr)),
                Generator::Modal(p) => GenVoice::Modal(ModalVoice::new(p, sr, seed)),
                Generator::Thump(p) => GenVoice::Thump(ThumpVoice::new(p, sr, seed)),
                Generator::Voice(p) => GenVoice::Voice(Box::new(VoiceVoice::new(p, sr, seed))),
                Generator::Pulses(p) => GenVoice::Pulses(PulseVoice::new(p, sr, seed)),
                Generator::Bubbles(p) => {
                    GenVoice::Bubbles(Box::new(BubblesVoice::new(p, sr, seed)))
                }
                Generator::Engine(p) => GenVoice::Engine(Box::new(EngineVoice::new(p, sr, seed))),
            };
            let (pl, pr) = pan_gains(l.pan);
            let g = gain * std::f32::consts::SQRT_2;
            layers.push(LayerVoice {
                source,
                start,
                end,
                shape,
                sustain,
                released: None,
                gains: (pl * g, pr * g),
                drive: l.drive.clamp(0.0, 1.0),
                follow: l.follow,
            });
        }
        Pass {
            layers,
            t: 0.0,
            opts: *opts,
        }
    }

    fn end(&self) -> f32 {
        self.layers.iter().map(|l| l.end).fold(0.0, f32::max)
    }

    fn is_done(&self) -> bool {
        self.t >= self.end()
    }

    fn release(&mut self) {
        let t = self.t;
        self.layers.iter_mut().for_each(|l| l.release(t));
    }

    /// Render every layer into the buffers and advance time.
    fn render(&mut self, l: &mut [f32], r: &mut [f32], dt: f32, live: Live) {
        let Live {
            intensity,
            pitch,
            rate,
        } = live;
        let n = l.len();
        let t0 = self.t;
        let pitch = 2.0f32.powf(pitch / 12.0);
        let bright = 2.0f32.powf(intensity - 1.0);
        let density = 2.0f32.powf(1.5 * (intensity - 1.0));
        for layer in &mut self.layers {
            if t0 + n as f32 * dt <= layer.start || t0 >= layer.end {
                continue;
            }
            let mods = Mods {
                pitch: pitch * 2.0f32.powf(layer.follow * (intensity - 1.0) / 12.0),
                bright,
                density,
                rate,
            };
            let drive = 1.0 + layer.drive * 6.0;
            let makeup = 1.0 / (1.0 + layer.drive * 1.5);
            for i in 0..n {
                let t = t0 + i as f32 * dt;
                if t < layer.start || t >= layer.end {
                    continue;
                }
                let local = t - layer.start;
                let env = layer.level(local);
                let mut x = match &mut layer.source {
                    GenVoice::Noise(v) => v.sample(local, env, mods),
                    GenVoice::Tone(v) => v.sample(local, env, mods),
                    GenVoice::Modal(v) => v.sample(),
                    GenVoice::Thump(v) => v.sample(local, env, mods),
                    GenVoice::Voice(v) => v.sample(local, env, mods),
                    GenVoice::Pulses(v) => v.sample(local, env, mods),
                    GenVoice::Bubbles(v) => v.sample(env, mods),
                    GenVoice::Engine(v) => v.sample(env, mods),
                };
                if layer.drive > 0.0 {
                    x = soft_clip(x * drive) * makeup;
                }
                // Short fade at the very end so a cut tail doesn't click.
                x *= ((layer.end - t) / FADE).min(1.0);
                l[i] += x * layer.gains.0;
                r[i] += x * layer.gains.1;
            }
        }
        self.t += n as f32 * dt;
    }
}

/// One play of a sound: one pass, or for repeating sounds a pass every
/// few seconds until released.
pub struct SfxVoice {
    /// Kept to start new passes of repeating sounds.
    sound: Option<Box<Sound>>,
    opts: PlayOpts,
    sr: f32,
    dt: f32,
    passes: Vec<Pass>,
    looping: Looping,
    t: f32,
    next_pass: f32,
    count: u32,
    released: bool,
    intensity: f32,
    target: f32,
    pitch: f32,
    rate: f32,
    smooth: f32,
    volume: f32,
    space: f32,
    /// Remaining gain of a sound being cut short.
    fade: Option<f32>,
    fade_step: f32,
    /// Gain of a sound fading in, taking over from an older version of itself.
    fade_in: Option<f32>,
}

impl SfxVoice {
    pub fn new(sound: &Sound, opts: &PlayOpts, sr: f32) -> Self {
        let repeat = matches!(sound.looping, Looping::Repeat { .. });
        let every = match sound.looping {
            Looping::Repeat { every } => every.max(0.02),
            _ => f32::INFINITY,
        };
        let intensity = opts.intensity.clamp(0.0, 2.0);
        SfxVoice {
            sound: repeat.then(|| Box::new(sound.clone())),
            opts: *opts,
            sr,
            dt: 1.0 / sr,
            passes: vec![Pass::new(sound, opts, sr)],
            looping: sound.looping,
            t: 0.0,
            next_pass: every / opts.rate.max(0.01),
            count: 1,
            released: false,
            intensity,
            target: intensity,
            pitch: opts.pitch,
            rate: opts.rate.max(0.01),
            smooth: 1.0 - (-(CHUNK as f32) / (SMOOTH * sr)).exp(),
            volume: sound.volume * opts.volume,
            space: sound.space.clamp(0.0, 1.0),
            fade: None,
            fade_step: 1.0 / (FADE * sr),
            fade_in: None,
        }
    }

    /// Whether an edited version of the sound can take over from this play:
    /// a loop that is still holding or repeating.
    pub fn swappable(&self) -> bool {
        self.looping != Looping::Once && !self.released && self.fade.is_none()
    }

    /// This play, as an edited version of the sound would have it at the
    /// same moment: the same variation, the same time into every pass. The
    /// new one fades in over [`XFADE`]; [`Self::cross_fade_out`] this one.
    ///
    /// Layers that ring or bubble on from their own past (resonances,
    /// bubbles, engines) start that afresh; everything else is a function of
    /// time and carries on where it was.
    pub fn swapped(&self, sound: &Sound) -> SfxVoice {
        let mut v = SfxVoice::new(sound, &self.opts, self.sr);
        v.passes = self
            .passes
            .iter()
            .map(|p| {
                let mut pass = Pass::new(sound, &p.opts, self.sr);
                pass.t = p.t;
                pass
            })
            .collect();
        v.t = self.t;
        v.count = self.count;
        v.intensity = self.intensity;
        v.target = self.target;
        v.pitch = self.pitch;
        v.rate = self.rate;
        // A new repeat interval counts from now, not from the start.
        v.next_pass = match sound.looping {
            Looping::Repeat { every } => self.next_pass.min(self.t + every.max(0.02) / self.rate),
            _ => f32::INFINITY,
        };
        v.fade_in = Some(0.0);
        v.fade_step = 1.0 / (XFADE * self.sr);
        v
    }

    /// Fade out over [`XFADE`], as a newer version of the sound fades in.
    pub fn cross_fade_out(&mut self) {
        self.fade_step = 1.0 / (XFADE * self.sr);
        self.stop();
    }

    /// Seconds from the start until the last layer is silent (infinite for
    /// a looping sound until it is released).
    pub fn length(&self) -> f32 {
        if self.looping != Looping::Once && !self.released {
            return f32::INFINITY;
        }
        self.passes.iter().map(Pass::end).fold(0.0, f32::max)
    }

    pub fn is_done(&self) -> bool {
        if self.fade.is_some_and(|f| f <= 0.0) {
            return true;
        }
        let repeating = matches!(self.looping, Looping::Repeat { .. }) && !self.released;
        !repeating && self.passes.iter().all(Pass::is_done)
    }

    /// Fade out quickly (when a newer sound takes this one's place).
    pub fn stop(&mut self) {
        if self.fade.is_none() {
            self.fade = Some(1.0);
        }
    }

    /// End a looping sound: held layers fade out, repeats stop.
    pub fn release(&mut self) {
        self.released = true;
        self.passes.iter_mut().for_each(Pass::release);
    }

    /// Change the intensity while playing (see [`PlayOpts::intensity`]).
    pub fn set_intensity(&mut self, intensity: f32) {
        self.target = intensity.clamp(0.0, 2.0);
    }

    /// Change the pitch (semitones) while playing. Resonances keep the
    /// pitch they were struck at.
    pub fn set_pitch(&mut self, semitones: f32) {
        self.pitch = semitones;
    }

    /// Change the speed of repeats and pulses while playing.
    pub fn set_rate(&mut self, rate: f32) {
        let rate = rate.max(0.01);
        // The wait for the next repeat shrinks or grows with the new speed.
        if self.next_pass.is_finite() && self.next_pass > self.t {
            self.next_pass = self.t + (self.next_pass - self.t) * self.rate / rate;
        }
        self.rate = rate;
    }

    /// Change intensity, pitch and speed together, from play options.
    pub fn set_live(&mut self, opts: &PlayOpts) {
        self.set_intensity(opts.intensity);
        self.set_pitch(opts.pitch);
        self.set_rate(opts.rate);
    }

    /// Add the next `l.len()` samples into the buffers, and the reverb send
    /// (mono) into `rev`.
    pub fn render(&mut self, l: &mut [f32], r: &mut [f32], rev: &mut [f32]) {
        let n = l.len().min(r.len()).min(rev.len());
        let mut done = 0;
        while done < n {
            let m = (n - done).min(CHUNK);
            self.start_passes();
            let before = intensity_gain(self.intensity);
            self.intensity += (self.target - self.intensity) * self.smooth;
            let after = intensity_gain(self.intensity);
            let mut bl = [0.0f32; CHUNK];
            let mut br = [0.0f32; CHUNK];
            let live = Live {
                intensity: self.intensity,
                pitch: self.pitch,
                rate: self.rate,
            };
            for pass in &mut self.passes {
                pass.render(&mut bl[..m], &mut br[..m], self.dt, live);
            }
            self.passes.retain(|p| !p.is_done());
            for i in 0..m {
                let mut g = self.volume * (before + (after - before) * i as f32 / m as f32);
                if let Some(fade) = &mut self.fade {
                    *fade = (*fade - self.fade_step).max(0.0);
                    g *= *fade;
                }
                if let Some(rise) = &mut self.fade_in {
                    *rise = (*rise + self.fade_step).min(1.0);
                    g *= *rise;
                }
                let (a, b) = (bl[i] * g, br[i] * g);
                l[done + i] += a;
                r[done + i] += b;
                rev[done + i] += (a + b) * self.space;
            }
            self.t += m as f32 * self.dt;
            done += m;
        }
    }

    /// Repeating sounds: start a new, freshly varied pass when it's time.
    fn start_passes(&mut self) {
        let Some(sound) = &self.sound else { return };
        if self.released || self.t < self.next_pass {
            return;
        }
        let every = match self.looping {
            Looping::Repeat { every } => every.max(0.02),
            _ => return,
        };
        let opts = PlayOpts {
            seed: self.opts.seed ^ self.count.wrapping_mul(0x2545_f491),
            intensity: self.intensity,
            pitch: self.pitch,
            rate: self.rate,
            ..self.opts
        };
        if self.passes.len() >= MAX_REPEATS {
            self.passes.remove(0);
        }
        self.passes.push(Pass::new(sound, &opts, self.sr));
        self.count += 1;
        self.next_pass += every / self.rate;
    }
}

/// Plays one sound on its own, with its own reverb: for games and offline
/// rendering, where there is no shared engine.
pub struct SoundPlayer {
    voice: SfxVoice,
    reverb: Option<Reverb>,
    /// Seconds rendered after the voice finished (the reverb tail).
    tail: f32,
    quiet: f32,
    dt: f32,
    done: bool,
}

/// Longest reverb tail kept after a sound ends.
const MAX_TAIL: f32 = 4.0;

impl SoundPlayer {
    pub fn new(sound: &Sound, opts: &PlayOpts, sr: f32) -> Self {
        let voice = SfxVoice::new(sound, opts, sr);
        let reverb = (voice.space > 0.0).then(|| Reverb::new(sr));
        SoundPlayer {
            voice,
            reverb,
            tail: 0.0,
            quiet: 0.0,
            dt: 1.0 / sr,
            done: false,
        }
    }

    pub fn is_done(&self) -> bool {
        self.done
    }

    pub fn stop(&mut self) {
        self.voice.stop();
    }

    pub fn release(&mut self) {
        self.voice.release();
    }

    pub fn set_intensity(&mut self, intensity: f32) {
        self.voice.set_intensity(intensity);
    }

    /// Change intensity, pitch and speed while playing.
    pub fn set_live(&mut self, opts: &PlayOpts) {
        self.voice.set_live(opts);
    }

    /// Render into the buffers (overwriting them); returns false once the
    /// sound and its reverb tail are over.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        let n = l.len().min(r.len());
        l.fill(0.0);
        r.fill(0.0);
        if self.done {
            return false;
        }
        let mut rev = [0.0f32; CHUNK];
        let mut i = 0;
        while i < n {
            let m = (n - i).min(CHUNK);
            let (bl, br) = (&mut l[i..i + m], &mut r[i..i + m]);
            rev[..m].fill(0.0);
            if !self.voice.is_done() {
                self.voice.render(bl, br, &mut rev[..m]);
            }
            let mut peak = 0.0f32;
            for k in 0..m {
                let (rl, rr) = match &mut self.reverb {
                    Some(rv) => rv.process(rev[k]),
                    None => (0.0, 0.0),
                };
                bl[k] = soft_clip((bl[k] + rl * 2.5) * 0.9);
                br[k] = soft_clip((br[k] + rr * 2.5) * 0.9);
                peak = peak.max(bl[k].abs()).max(br[k].abs());
            }
            if self.voice.is_done() {
                let secs = m as f32 * self.dt;
                self.tail += secs;
                self.quiet = if peak < 1e-4 { self.quiet + secs } else { 0.0 };
                if self.reverb.is_none() || self.quiet > 0.2 || self.tail > MAX_TAIL {
                    self.done = true;
                    return i + m > 0;
                }
            }
            i += m;
        }
        true
    }
}

/// Render one play of a sound offline to interleaved stereo samples,
/// including its reverb tail. Looping sounds are released after
/// [`Sound::loop_preview`] seconds.
pub fn render_sound(sound: &Sound, opts: &PlayOpts, sample_rate: u32) -> Vec<f32> {
    let mut player = SoundPlayer::new(sound, opts, sample_rate as f32);
    let release_at = sound
        .loop_preview()
        .map(|s| (s * sample_rate as f32) as usize);
    let chunk = 512;
    let mut l = vec![0.0f32; chunk];
    let mut r = vec![0.0f32; chunk];
    let mut out = Vec::new();
    while !player.is_done() {
        if release_at.is_some_and(|at| out.len() / 2 >= at) {
            player.release();
        }
        player.process(&mut l, &mut r);
        for i in 0..chunk {
            out.push(l[i]);
            out.push(r[i]);
        }
    }
    // Trim the silent end of the last chunk.
    let last = out
        .iter()
        .rposition(|s| s.abs() > 1e-5)
        .map_or(0, |i| i + 1);
    out.truncate(last.next_multiple_of(2));
    out
}

/// Peak level of one layer over time, in `bins` equal slices of `seconds`,
/// played once as designed (no variation). For drawing waveforms.
pub fn layer_envelope(sound: &Sound, layer: &Layer, seconds: f32, bins: usize) -> Vec<f32> {
    const SR: f32 = 22050.0;
    let solo = Sound {
        layers: vec![Layer {
            mute: false,
            solo: false,
            start: 0.0,
            pan: 0.0,
            ..layer.clone()
        }],
        variation: 0.0,
        space: 0.0,
        volume: 1.0,
        looping: Looping::Once,
        ..sound.clone()
    };
    let mut voice = SfxVoice::new(&solo, &PlayOpts::default(), SR);
    let total = (seconds * SR) as usize;
    let per_bin = (total / bins.max(1)).max(1);
    let mut out = vec![0.0f32; bins];
    let mut l = vec![0.0f32; per_bin];
    let mut r = vec![0.0f32; per_bin];
    let mut rev = vec![0.0f32; per_bin];
    for bin in out.iter_mut() {
        if voice.is_done() {
            break;
        }
        l.fill(0.0);
        r.fill(0.0);
        voice.render(&mut l, &mut r, &mut rev);
        // Centered: the left channel carries the layer at its own gain.
        *bin = l.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestre_core::sfx::presets::{PRESETS, find};
    use orchestre_core::sfx::{Curve, GeneratorKind};

    const SR: u32 = 44100;

    fn peak(buf: &[f32]) -> f32 {
        buf.iter().fold(0.0f32, |m, s| m.max(s.abs()))
    }

    fn rms(buf: &[f32]) -> f32 {
        (buf.iter().map(|s| s * s).sum::<f32>() / buf.len().max(1) as f32).sqrt()
    }

    fn preset(name: &str) -> Sound {
        find(name)
            .unwrap_or_else(|| panic!("no preset {name}"))
            .sound()
    }

    #[test]
    fn every_preset_makes_sound_and_ends() {
        for p in PRESETS {
            let s = p.sound();
            let out = render_sound(&s, &PlayOpts::seeded(7), SR);
            assert!(out.iter().all(|x| x.is_finite()), "{}: non-finite", p.name);
            assert!(peak(&out) <= 1.0, "{}: clips", p.name);
            assert!(peak(&out) > 0.05, "{} is silent: {}", p.name, peak(&out));
            let secs = out.len() as f32 / 2.0 / SR as f32;
            let expected = s.loop_preview().unwrap_or(0.0) + s.length() * 1.5;
            assert!(secs <= expected + MAX_TAIL + 0.1, "{} runs {secs}s", p.name);
        }
    }

    /// One steady voice layer, the same on every play.
    fn klatt_voice(tweak: impl FnOnce(&mut VoiceParams)) -> Vec<f32> {
        let Generator::Voice(mut p) = GeneratorKind::Voice.default_generator() else {
            unreachable!()
        };
        p.shape = Shape::new(0.01, 0.6, 0.05);
        p.pitch = Curve::flat(100.0);
        p.openness = Curve::flat(0.4);
        p.frontness = Curve::flat(0.4);
        p.loudness = Curve::flat(1.0);
        p.roughness = 0.0;
        p.breath = 0.0;
        tweak(&mut p);
        let mut s = Sound::default();
        s.add_layer("voice", Generator::Voice(p));
        s.variation = 0.0;
        let out = render_sound(&s, &PlayOpts::seeded(1), SR);
        assert!(out.iter().all(|x| x.is_finite()), "non-finite");
        assert!(peak(&out) <= 1.0, "clips: {}", peak(&out));
        // The left channel, past the attack, before the release.
        out.chunks(2)
            .map(|f| f[0])
            .skip(SR as usize / 10)
            .take(SR as usize / 2)
            .collect()
    }

    /// Power at `f` Hz (Goertzel).
    fn power_at(x: &[f32], f: f32) -> f32 {
        let w = TAU * f / SR as f32;
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for &v in x {
            let s0 = v + 2.0 * w.cos() * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        (s1 * s1 + s2 * s2 - 2.0 * w.cos() * s1 * s2) / x.len() as f32
    }

    /// Energy-weighted mean frequency, from a coarse scan.
    fn centroid(x: &[f32]) -> f32 {
        let bands: Vec<f32> = (1..40).map(|k| k as f32 * 250.0).collect();
        let powers: Vec<f32> = bands.iter().map(|&f| power_at(x, f)).collect();
        bands.iter().zip(&powers).map(|(f, p)| f * p).sum::<f32>() / powers.iter().sum::<f32>()
    }

    #[test]
    fn an_f3_set_by_hand_is_where_the_sound_peaks() {
        // An r is a vowel with its third formant pulled down to ~1600 Hz.
        let uh = klatt_voice(|_| {});
        let r = klatt_voice(|p| p.klatt.formants[2] = Some(Curve::flat(1600.0)));
        let tilt = |x: &[f32]| power_at(x, 1600.0) / power_at(x, 2500.0);
        assert!(tilt(&r) > 4.0 * tilt(&uh), "{} vs {}", tilt(&r), tilt(&uh));
    }

    #[test]
    fn frication_alone_hisses_and_the_parallel_levels_shape_it() {
        let hiss = |levels: [f32; 7]| {
            klatt_voice(|p| {
                p.klatt.voicing = Curve::flat(0.0);
                p.klatt.frication = Curve::flat(1.0);
                p.klatt.parallel = levels;
            })
        };
        // s: F6 and the bypass. sh: F3 and F4, lower.
        let s = hiss([0.0, 0.0, 0.0, 0.1, 0.3, 1.0, 0.5]);
        let sh = hiss([0.0, 0.0, 1.0, 0.7, 0.2, 0.0, 0.0]);
        for (name, x) in [("s", &s), ("sh", &sh)] {
            assert!(rms(x) > 0.02, "{name} is too quiet: {}", rms(x));
            // No voice: nothing at the pitch.
            assert!(power_at(x, 100.0) < power_at(x, 3000.0), "{name} hums");
        }
        assert!(
            centroid(&s) > centroid(&sh) + 800.0,
            "s at {} Hz, sh at {} Hz",
            centroid(&s),
            centroid(&sh)
        );
    }

    #[test]
    fn a_whisper_is_breath_without_voice() {
        let whisper = klatt_voice(|p| {
            p.klatt.voicing = Curve::flat(0.0);
            p.klatt.aspiration = Curve::flat(1.0);
        });
        assert!(rms(&whisper) > 0.005, "{}", rms(&whisper));
        let voiced = klatt_voice(|_| {});
        // The pitch's harmonics stand out of a voice, not out of breath.
        let harmonic = |x: &[f32]| power_at(x, 300.0) / power_at(x, 350.0);
        assert!(harmonic(&voiced) > 10.0 * harmonic(&whisper));
    }

    #[test]
    fn the_nasal_zero_cuts_a_notch() {
        let open = klatt_voice(|p| p.openness = Curve::flat(0.1));
        let hum = klatt_voice(|p| {
            p.openness = Curve::flat(0.1);
            p.klatt.nasal_zero = Some(Curve::flat(1000.0));
        });
        // Harmonics of 100 Hz, relative to the fundamental.
        let at_zero = |x: &[f32]| power_at(x, 1000.0) / power_at(x, 200.0);
        assert!(
            at_zero(&hum) < 0.2 * at_zero(&open),
            "{} vs {}",
            at_zero(&hum),
            at_zero(&open)
        );
    }

    #[test]
    fn a_trill_shuts_briefly_where_a_flutter_dips_long() {
        let rasp = |trill: f32| {
            klatt_voice(|p| {
                p.rasp = 1.0;
                p.rasp_rate = 25.0;
                p.trill = trill;
            })
        };
        // Share of the time the level is nearly shut, from a 2 ms envelope.
        let shut = |x: &[f32]| {
            let env: Vec<f32> = x
                .chunks(88)
                .map(|c| c.iter().fold(0.0f32, |m, v| m.max(v.abs())))
                .collect();
            let top = env.iter().fold(0.0f32, |m, v| m.max(*v));
            env.iter().filter(|v| **v < 0.1 * top).count() as f32 / env.len() as f32
        };
        let (flutter, trill) = (shut(&rasp(0.0)), shut(&rasp(1.0)));
        assert!(trill > 0.02, "a trill still closes: {trill}");
        assert!(trill < 0.6 * flutter, "and briefly: {trill} vs {flutter}");
    }

    #[test]
    fn every_generator_default_makes_sound() {
        for kind in GeneratorKind::ALL {
            let mut s = Sound::default();
            s.add_layer("x", kind.default_generator());
            let out = render_sound(&s, &PlayOpts::default(), SR);
            assert!(
                peak(&out) > 0.02 && peak(&out) <= 1.0,
                "{kind:?}: {}",
                peak(&out)
            );
        }
    }

    #[test]
    fn same_seed_same_sound_new_seed_new_sound() {
        let s = preset("Pistol shot");
        let a = render_sound(&s, &PlayOpts::seeded(1), SR);
        let b = render_sound(&s, &PlayOpts::seeded(1), SR);
        let c = render_sound(&s, &PlayOpts::seeded(2), SR);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn no_variation_ignores_the_seed_for_tones() {
        // Tones have no noise in them, so without variation every play is identical.
        let mut s = preset("Coin");
        s.variation = 0.0;
        let a = render_sound(&s, &PlayOpts::seeded(1), SR);
        let b = render_sound(&s, &PlayOpts::seeded(99), SR);
        assert_eq!(a, b);
    }

    #[test]
    fn intensity_changes_loudness() {
        let s = preset("Punch");
        let level = |i: f32| {
            let opts = PlayOpts {
                intensity: i,
                ..PlayOpts::seeded(3)
            };
            rms(&render_sound(&s, &opts, SR))
        };
        let (soft, normal, hard) = (level(0.3), level(1.0), level(1.8));
        assert!(soft < normal * 0.6, "{soft} vs {normal}");
        assert!(hard > normal, "{hard} vs {normal}");
        assert_eq!(level(0.0), 0.0);
    }

    #[test]
    fn muting_a_layer_keeps_the_others_identical() {
        let s = preset("Laser");
        let mut only_first = s.clone();
        only_first.layers[1].mute = true;
        let mut solo_first = s.clone();
        solo_first.layers[0].solo = true;
        let opts = PlayOpts::seeded(5);
        assert_eq!(
            render_sound(&only_first, &opts, SR),
            render_sound(&solo_first, &opts, SR)
        );
    }

    fn run(v: &mut SfxVoice, secs: f32) -> Vec<f32> {
        let n = (secs * SR as f32) as usize;
        let (mut l, mut r, mut rev) = (vec![0.0; n], vec![0.0; n], vec![0.0; n]);
        v.render(&mut l, &mut r, &mut rev);
        l
    }

    #[test]
    fn stopped_voices_fade_out() {
        let s = preset("Distant boom");
        let mut v = SfxVoice::new(&s, &PlayOpts::default(), SR as f32);
        run(&mut v, 0.01);
        assert!(!v.is_done());
        v.stop();
        run(&mut v, 0.02);
        assert!(v.is_done());
    }

    #[test]
    fn sustained_sounds_hold_until_released() {
        let s = preset("Wind");
        assert_eq!(s.looping, Looping::Sustain);
        let mut v = SfxVoice::new(&s, &PlayOpts::default(), SR as f32);
        let early = rms(&run(&mut v, 1.0));
        let later = rms(&run(&mut v, 8.0));
        assert!(!v.is_done() && v.length().is_infinite());
        assert!(later > early * 0.3, "still going: {later} vs {early}");
        v.release();
        let decay = s
            .layers
            .iter()
            .filter_map(|l| l.generator.shape())
            .map(|s| s.decay)
            .fold(0.0, f32::max);
        run(&mut v, decay * 1.6 + 0.1);
        assert!(v.is_done());
    }

    #[test]
    fn repeating_sounds_repeat_until_released() {
        let s = preset("Heartbeat");
        let Looping::Repeat { every } = s.looping else {
            panic!("heartbeat should repeat");
        };
        let mut v = SfxVoice::new(&s, &PlayOpts::default(), SR as f32);
        let out = run(&mut v, every * 4.5);
        // Energy in each period: all of them beat.
        let per = (every * SR as f32) as usize;
        for k in 0..4 {
            assert!(rms(&out[k * per..(k + 1) * per]) > 0.01, "beat {k}");
        }
        assert!(!v.is_done());
        v.release();
        run(&mut v, s.length() + 0.1);
        assert!(v.is_done());
    }

    #[test]
    fn rate_speeds_up_repeats_live() {
        let s = preset("Heartbeat");
        let Looping::Repeat { every } = s.looping else {
            panic!()
        };
        let mut v = SfxVoice::new(&s, &PlayOpts::default(), SR as f32);
        run(&mut v, every * 4.0 + 0.01);
        let normal = v.count;
        v.set_rate(2.0);
        run(&mut v, every * 4.0);
        assert_eq!(v.count - normal, 8, "twice as many beats");
    }

    #[test]
    fn fast_sine_is_close() {
        for i in 0..1000 {
            let x = i as f32 / 1000.0;
            assert!((fast_sin(x) - (x * TAU).sin()).abs() < 0.002, "{x}");
        }
    }

    #[test]
    fn bubbles_ring_at_their_size() {
        let mut s = Sound::default();
        // 3 mm bubbles ring near 1 kHz.
        let g = Generator::Bubbles(BubbleParams {
            rate: 200.0,
            size_min: 3.0,
            size_max: 3.0,
            small: 0.0,
            rise: 0.0,
            shape: Shape::new(0.001, 0.5, 0.1),
        });
        s.add_layer("b", g);
        s.variation = 0.0;
        let out = render_sound(&s, &PlayOpts::default(), SR);
        assert!(peak(&out) > 0.05);
        // Count zero crossings on the left channel: ~2 per cycle.
        let left: Vec<f32> = out.iter().step_by(2).copied().collect();
        let n = left.len().min(SR as usize / 2);
        let crossings = left[..n]
            .windows(2)
            .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
            .count();
        let hz = crossings as f32 / (n as f32 / SR as f32);
        assert!((800.0..1200.0).contains(&hz), "{hz} Hz");
    }

    #[test]
    fn intensity_can_change_while_playing() {
        let s = preset("Engine");
        let mut quiet = SfxVoice::new(&s, &PlayOpts::default(), SR as f32);
        let mut loud = SfxVoice::new(&s, &PlayOpts::default(), SR as f32);
        run(&mut quiet, 0.5);
        run(&mut loud, 0.5);
        quiet.set_intensity(0.4);
        loud.set_intensity(1.8);
        let (a, b) = (rms(&run(&mut quiet, 1.0)), rms(&run(&mut loud, 1.0)));
        assert!(b > a * 1.5, "{b} vs {a}");
    }

    #[test]
    fn empty_sound_is_silent_and_done() {
        let s = Sound::default();
        assert!(render_sound(&s, &PlayOpts::default(), SR).is_empty());
        assert!(SfxVoice::new(&s, &PlayOpts::default(), SR as f32).is_done());
    }

    #[test]
    fn layer_envelope_follows_the_shape() {
        let s = preset("Explosion");
        let boom = s.layers.iter().find(|l| l.name == "Boom").unwrap();
        let env = layer_envelope(&s, boom, 2.0, 100);
        assert_eq!(env.len(), 100);
        assert!(env[1] > 0.1);
        assert!(env[99] < env[1] * 0.1);
    }
}
