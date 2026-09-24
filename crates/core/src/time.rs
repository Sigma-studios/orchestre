use serde::{Deserialize, Serialize};

/// Musical time in ticks. Signed so that relative moves are easy to express.
pub type Tick = i64;

/// Ticks per quarter note. Divisible by 3 (triplets) and by 2^6.
pub const PPQ: Tick = 960;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeSig {
    pub num: u8,
    pub den: u8,
}

impl TimeSig {
    pub const PRESETS: [TimeSig; 5] = [
        TimeSig { num: 4, den: 4 },
        TimeSig { num: 3, den: 4 },
        TimeSig { num: 2, den: 4 },
        TimeSig { num: 6, den: 8 },
        TimeSig { num: 5, den: 4 },
    ];

    pub fn beat_ticks(self) -> Tick {
        PPQ * 4 / self.den as Tick
    }

    pub fn bar_ticks(self) -> Tick {
        self.beat_ticks() * self.num as Tick
    }

    pub fn label(self) -> String {
        format!("{}/{}", self.num, self.den)
    }
}

impl Default for TimeSig {
    fn default() -> Self {
        TimeSig { num: 4, den: 4 }
    }
}

/// Snap/grid resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Grid {
    Bar,
    Half,
    Quarter,
    Eighth,
    #[default]
    Sixteenth,
    ThirtySecond,
    EighthTriplet,
    SixteenthTriplet,
}

impl Grid {
    pub const ALL: [Grid; 8] = [
        Grid::Bar,
        Grid::Half,
        Grid::Quarter,
        Grid::Eighth,
        Grid::Sixteenth,
        Grid::ThirtySecond,
        Grid::EighthTriplet,
        Grid::SixteenthTriplet,
    ];

    pub fn ticks(self, ts: TimeSig) -> Tick {
        match self {
            Grid::Bar => ts.bar_ticks(),
            Grid::Half => PPQ * 2,
            Grid::Quarter => PPQ,
            Grid::Eighth => PPQ / 2,
            Grid::Sixteenth => PPQ / 4,
            Grid::ThirtySecond => PPQ / 8,
            Grid::EighthTriplet => PPQ / 3,
            Grid::SixteenthTriplet => PPQ / 6,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Grid::Bar => "1 bar",
            Grid::Half => "1/2",
            Grid::Quarter => "1/4",
            Grid::Eighth => "1/8",
            Grid::Sixteenth => "1/16",
            Grid::ThirtySecond => "1/32",
            Grid::EighthTriplet => "1/8 triplet",
            Grid::SixteenthTriplet => "1/16 triplet",
        }
    }
}

/// Round to the nearest multiple of `step`.
pub fn snap_round(t: Tick, step: Tick) -> Tick {
    if step <= 1 {
        return t;
    }
    (t as f64 / step as f64).round() as Tick * step
}

/// Round down to a multiple of `step`.
pub fn snap_floor(t: Tick, step: Tick) -> Tick {
    if step <= 1 {
        return t;
    }
    t.div_euclid(step) * step
}

pub fn ticks_to_seconds(t: f64, bpm: f64) -> f64 {
    t / PPQ as f64 * 60.0 / bpm
}

pub fn seconds_to_ticks(s: f64, bpm: f64) -> f64 {
    s * bpm / 60.0 * PPQ as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_lengths() {
        assert_eq!(TimeSig { num: 4, den: 4 }.bar_ticks(), 4 * PPQ);
        assert_eq!(TimeSig { num: 3, den: 4 }.bar_ticks(), 3 * PPQ);
        assert_eq!(TimeSig { num: 6, den: 8 }.bar_ticks(), 3 * PPQ);
        assert_eq!(TimeSig { num: 6, den: 8 }.beat_ticks(), PPQ / 2);
    }

    #[test]
    fn snapping() {
        let s = Grid::Sixteenth.ticks(TimeSig::default());
        assert_eq!(snap_round(250, s), 240);
        assert_eq!(snap_round(120, s), 240);
        assert_eq!(snap_round(119, s), 0);
        assert_eq!(snap_floor(479, s), 240);
        assert_eq!(snap_floor(-1, s), -240);
        assert_eq!(Grid::EighthTriplet.ticks(TimeSig::default()) * 3, PPQ);
    }

    #[test]
    fn seconds() {
        assert!((ticks_to_seconds(PPQ as f64, 120.0) - 0.5).abs() < 1e-9);
        assert!((seconds_to_ticks(0.5, 120.0) - PPQ as f64).abs() < 1e-9);
    }
}
