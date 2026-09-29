//! Arpeggiator: turns held notes into a rhythmic pattern of single notes.

use orchestre_core::{ArpParams, ArpPattern};

use crate::util::Rng;

const MAX_HELD: usize = 16;
/// Pattern length: held notes times octaves.
const MAX_STEPS: usize = MAX_HELD * 4;

#[derive(Clone, Copy, Default)]
struct Held {
    pitch: u8,
    vel: f32,
    /// Tick at which the note is released; infinite for live notes.
    off_at: f64,
    order: u64,
}

/// What the arpeggiator wants the instrument to do.
pub enum ArpEvent {
    On { pitch: u8, vel: f32 },
    Off { pitch: u8 },
}

pub struct Arp {
    held: [Held; MAX_HELD],
    len: usize,
    counter: u64,
    step: usize,
    /// Pitch sounding now, and the tick it stops.
    sounding: Option<(u8, f64)>,
    /// Tick of the next step; `None` until notes are held.
    next: Option<f64>,
    rng: Rng,
}

impl Default for Arp {
    fn default() -> Self {
        Arp {
            held: [Held::default(); MAX_HELD],
            len: 0,
            counter: 0,
            step: 0,
            sounding: None,
            next: None,
            rng: Rng::new(7),
        }
    }
}

impl Arp {
    pub fn note_on(&mut self, pitch: u8, vel: f32, off_at: f64) {
        self.remove(|h| h.pitch == pitch);
        if self.len == MAX_HELD {
            self.held.copy_within(1.., 0);
            self.len -= 1;
        }
        self.counter += 1;
        self.held[self.len] = Held {
            pitch,
            vel,
            off_at,
            order: self.counter,
        };
        self.len += 1;
    }

    pub fn live_off(&mut self, pitch: u8) {
        self.remove(|h| h.pitch == pitch && h.off_at.is_infinite());
    }

    pub fn release_due(&mut self, tick: f64) {
        self.remove(|h| h.off_at <= tick);
    }

    /// Forget every held note (the sounding one is stopped by the next `process`).
    pub fn all_off(&mut self) {
        self.len = 0;
    }

    pub fn is_idle(&self) -> bool {
        self.len == 0 && self.sounding.is_none()
    }

    fn remove(&mut self, pred: impl Fn(&Held) -> bool) {
        let mut j = 0;
        for i in 0..self.len {
            if !pred(&self.held[i]) {
                self.held[j] = self.held[i];
                j += 1;
            }
        }
        self.len = j;
    }

    /// The pattern's notes, in playing order.
    fn sequence(&self, p: &ArpParams, out: &mut [(u8, f32); MAX_STEPS]) -> usize {
        let mut notes = self.held;
        let notes = &mut notes[..self.len];
        match p.pattern {
            ArpPattern::Played => notes.sort_by_key(|h| h.order),
            _ => notes.sort_by_key(|h| h.pitch),
        }
        let mut n = 0;
        for oct in 0..p.octaves.clamp(1, 4) {
            for h in notes.iter() {
                let pitch = h.pitch as u32 + 12 * oct as u32;
                if pitch <= 127 && n < MAX_STEPS {
                    out[n] = (pitch as u8, h.vel);
                    n += 1;
                }
            }
        }
        match p.pattern {
            ArpPattern::Down => out[..n].reverse(),
            ArpPattern::UpDown if n > 2 => {
                // Bounce without repeating the top and bottom notes.
                for i in (1..n - 1).rev() {
                    if n < MAX_STEPS {
                        out[n] = out[i];
                        n += 1;
                    }
                }
            }
            _ => {}
        }
        n
    }

    /// Advance over the ticks `[from, to)`; `step` is the step length in
    /// ticks. Events are pushed to `out`.
    pub fn process(
        &mut self,
        p: &ArpParams,
        from: f64,
        to: f64,
        step: f64,
        out: &mut impl FnMut(ArpEvent),
    ) {
        if let Some((pitch, end)) = self.sounding
            && (end < to || self.len == 0)
        {
            out(ArpEvent::Off { pitch });
            self.sounding = None;
        }
        if self.len == 0 {
            self.next = None;
            self.step = 0;
            return;
        }
        let step = step.max(1.0);
        // First step, or the transport jumped (seek, loop): restart on the
        // next step boundary.
        let next = match self.next {
            Some(t) if t >= from - step && t < to + step => t,
            _ => (from / step).ceil() * step,
        };
        if next >= to {
            self.next = Some(next);
            return;
        }
        let mut seq = [(0u8, 0.0f32); MAX_STEPS];
        let n = self.sequence(p, &mut seq);
        if n == 0 {
            return;
        }
        let i = match p.pattern {
            ArpPattern::Random => (self.rng.noise().abs() * n as f32) as usize % n,
            _ => self.step % n,
        };
        self.step = self.step.wrapping_add(1);
        let (pitch, vel) = seq[i];
        if let Some((prev, _)) = self.sounding.take() {
            out(ArpEvent::Off { pitch: prev });
        }
        out(ArpEvent::On { pitch, vel });
        self.sounding = Some((pitch, next + step * p.gate.clamp(0.05, 1.0) as f64));
        self.next = Some(next + step);
    }
}
