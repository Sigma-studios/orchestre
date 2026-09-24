use crate::filter::OnePole;
use crate::util::flush;

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
