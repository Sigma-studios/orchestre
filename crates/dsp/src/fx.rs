use crate::filter::OnePole;
use crate::util::{TAU, flush};

struct Comb {
    buf: Vec<f32>,
    pos: usize,
    store: f32,
}

impl Comb {
    fn new(len: usize) -> Self {
        Comb {
            buf: vec![0.0; len.max(1)],
            pos: 0,
            store: 0.0,
        }
    }

    #[inline]
    fn process(&mut self, x: f32, feedback: f32, damp: f32) -> f32 {
        let out = self.buf[self.pos];
        self.store = flush(out * (1.0 - damp) + self.store * damp);
        self.buf[self.pos] = x + self.store * feedback;
        self.pos += 1;
        if self.pos == self.buf.len() {
            self.pos = 0;
        }
        out
    }
}

struct Allpass {
    buf: Vec<f32>,
    pos: usize,
}

impl Allpass {
    fn new(len: usize) -> Self {
        Allpass {
            buf: vec![0.0; len.max(1)],
            pos: 0,
        }
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let b = self.buf[self.pos];
        self.buf[self.pos] = flush(x + b * 0.5);
        self.pos += 1;
        if self.pos == self.buf.len() {
            self.pos = 0;
        }
        b - x
    }
}

/// Freeverb-style stereo reverb, used as a shared send bus.
pub struct Reverb {
    combs: [Vec<Comb>; 2],
    allpasses: [Vec<Allpass>; 2],
    pub feedback: f32,
    pub damp: f32,
}

impl Reverb {
    pub fn new(sr: f32) -> Self {
        const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
        const ALLPASSES: [usize; 4] = [556, 441, 341, 225];
        const SPREAD: usize = 23;
        let scale = sr / 44100.0;
        let s = |n: usize| (n as f32 * scale) as usize;
        let make = |spread: usize| {
            (
                COMBS
                    .iter()
                    .map(|&n| Comb::new(s(n + spread)))
                    .collect::<Vec<_>>(),
                ALLPASSES
                    .iter()
                    .map(|&n| Allpass::new(s(n + spread)))
                    .collect::<Vec<_>>(),
            )
        };
        let (cl, al) = make(0);
        let (cr, ar) = make(SPREAD);
        Reverb {
            combs: [cl, cr],
            allpasses: [al, ar],
            feedback: 0.84,
            damp: 0.3,
        }
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> (f32, f32) {
        let x = input * 0.015;
        let mut out = [0.0f32; 2];
        for ((o, combs), allpasses) in out.iter_mut().zip(&mut self.combs).zip(&mut self.allpasses)
        {
            let mut acc = 0.0;
            for c in combs.iter_mut() {
                acc += c.process(x, self.feedback, self.damp);
            }
            for a in allpasses.iter_mut() {
                acc = a.process(acc);
            }
            *o = acc;
        }
        (out[0], out[1])
    }
}

/// Tempo-synced ping-pong echo, used as a shared send bus.
pub struct Delay {
    buf: [Vec<f32>; 2],
    pos: usize,
    delay: usize,
    pub feedback: f32,
    damp: [OnePole; 2],
}

impl Delay {
    pub fn new(sr: f32) -> Self {
        let len = (sr * 2.0) as usize;
        let mut damp = [OnePole::default(); 2];
        for d in &mut damp {
            d.set(4500.0, sr);
        }
        Delay {
            buf: [vec![0.0; len], vec![0.0; len]],
            pos: 0,
            delay: (sr * 0.3) as usize,
            feedback: 0.4,
            damp,
        }
    }

    pub fn set_time(&mut self, seconds: f32, sr: f32) {
        self.delay = ((seconds * sr) as usize).clamp(1, self.buf[0].len() - 1);
    }

    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let len = self.buf[0].len();
        let read = (self.pos + len - self.delay) % len;
        let dl = self.buf[0][read];
        let dr = self.buf[1][read];
        let mono = (l + r) * 0.5;
        self.buf[0][self.pos] = flush(mono + self.damp[1].low(dr) * self.feedback);
        self.buf[1][self.pos] = flush(self.damp[0].low(dl) * self.feedback);
        self.pos = (self.pos + 1) % len;
        (dl, dr)
    }
}

/// Stereo chorus: two slowly modulated short delays, in quadrature.
pub struct Chorus {
    buf: [Vec<f32>; 2],
    pos: usize,
    phase: f32,
    sr: f32,
}

impl Chorus {
    pub fn new(sr: f32) -> Self {
        let len = (sr * 0.04) as usize + 2;
        Chorus {
            buf: [vec![0.0; len], vec![0.0; len]],
            pos: 0,
            phase: 0.0,
            sr,
        }
    }

    /// Process a block in place; `amount` 0..1.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32], amount: f32) {
        let len = self.buf[0].len();
        let (wet, dry) = (amount * 0.6, 1.0 - amount * 0.25);
        let base = 0.012 * self.sr;
        let depth = 0.004 * self.sr;
        for i in 0..l.len() {
            self.buf[0][self.pos] = l[i];
            self.buf[1][self.pos] = r[i];
            let mut out = [0.0f32; 2];
            for (ch, o) in out.iter_mut().enumerate() {
                let lfo = (self.phase * TAU + ch as f32 * std::f32::consts::FRAC_PI_2).sin();
                let d = base + depth * lfo;
                let read = self.pos as f32 - d + len as f32;
                let i0 = read.floor() as usize % len;
                let frac = read.fract();
                let b = &self.buf[ch];
                *o = b[i0] * (1.0 - frac) + b[(i0 + 1) % len] * frac;
            }
            // Cross-feed the wet signal for width.
            l[i] = l[i] * dry + out[1] * wet;
            r[i] = r[i] * dry + out[0] * wet;
            self.pos = (self.pos + 1) % len;
            self.phase = (self.phase + 0.8 / self.sr).fract();
        }
    }
}

/// String-ensemble chorus, after the Solina / ARP string machines: three
/// delay taps 120° apart, each swept by a slow (~0.6 Hz) and a shallow fast
/// (~6 Hz) LFO. (Rates from Haible's triple-chorus notes and synth forums;
/// the delay range is a guess in the BBD region, 3–12 ms.)
pub struct Ensemble {
    buf: [Vec<f32>; 2],
    pos: usize,
    slow: f32,
    fast: f32,
    sr: f32,
}

impl Ensemble {
    pub fn new(sr: f32) -> Self {
        let len = (sr * 0.03) as usize + 2;
        Ensemble {
            buf: [vec![0.0; len], vec![0.0; len]],
            pos: 0,
            slow: 0.0,
            fast: 0.0,
            sr,
        }
    }

    /// Process a block in place; `amount` 0..1.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32], amount: f32) {
        let len = self.buf[0].len();
        let (wet, dry) = (amount * 0.55, 1.0 - amount * 0.1);
        let base = 0.007 * self.sr;
        let slow_depth = 0.0025 * self.sr;
        let fast_depth = 0.0003 * self.sr;
        let third = std::f32::consts::TAU / 3.0;
        for i in 0..l.len() {
            self.buf[0][self.pos] = l[i];
            self.buf[1][self.pos] = r[i];
            let mut taps = [0.0f32; 3];
            for (k, t) in taps.iter_mut().enumerate() {
                let phase = k as f32 * third;
                let d = base
                    + slow_depth * (self.slow * TAU + phase).sin()
                    + fast_depth * (self.fast * TAU + phase).sin();
                let read = self.pos as f32 - d + len as f32;
                let i0 = read.floor() as usize % len;
                let frac = read.fract();
                // Taps alternate between the channels for width.
                let b = &self.buf[k % 2];
                *t = b[i0] * (1.0 - frac) + b[(i0 + 1) % len] * frac;
            }
            l[i] = l[i] * dry + (taps[0] + taps[2] * 0.5) * wet;
            r[i] = r[i] * dry + (taps[1] + taps[2] * 0.5) * wet;
            self.pos = (self.pos + 1) % len;
            self.slow = (self.slow + 0.6 / self.sr).fract();
            self.fast = (self.fast + 6.0 / self.sr).fract();
        }
    }
}
