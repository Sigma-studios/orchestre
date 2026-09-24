use serde::{Deserialize, Serialize};

pub const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
];

pub fn pitch_name(pitch: u8) -> String {
    let octave = pitch as i32 / 12 - 1;
    format!("{}{}", NOTE_NAMES[pitch as usize % 12], octave)
}

pub fn is_black_key(pitch: u8) -> bool {
    matches!(pitch % 12, 1 | 3 | 6 | 8 | 10)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Scale {
    Major,
    Minor,
}

impl Scale {
    pub fn intervals(self) -> [u8; 7] {
        match self {
            Scale::Major => [0, 2, 4, 5, 7, 9, 11],
            Scale::Minor => [0, 2, 3, 5, 7, 8, 10],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Scale::Major => "Major",
            Scale::Minor => "Minor",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Key {
    /// Pitch class of the tonic, 0 = C.
    pub root: u8,
    pub scale: Scale,
}

impl Key {
    /// The most common keys in popular music, offered as presets.
    pub const PRESETS: [Key; 15] = [
        Key {
            root: 0,
            scale: Scale::Major,
        },
        Key {
            root: 7,
            scale: Scale::Major,
        },
        Key {
            root: 2,
            scale: Scale::Major,
        },
        Key {
            root: 9,
            scale: Scale::Major,
        },
        Key {
            root: 4,
            scale: Scale::Major,
        },
        Key {
            root: 5,
            scale: Scale::Major,
        },
        Key {
            root: 10,
            scale: Scale::Major,
        },
        Key {
            root: 3,
            scale: Scale::Major,
        },
        Key {
            root: 9,
            scale: Scale::Minor,
        },
        Key {
            root: 4,
            scale: Scale::Minor,
        },
        Key {
            root: 11,
            scale: Scale::Minor,
        },
        Key {
            root: 2,
            scale: Scale::Minor,
        },
        Key {
            root: 7,
            scale: Scale::Minor,
        },
        Key {
            root: 0,
            scale: Scale::Minor,
        },
        Key {
            root: 5,
            scale: Scale::Minor,
        },
    ];

    pub fn label(self) -> String {
        format!("{} {}", NOTE_NAMES[self.root as usize], self.scale.label())
    }

    pub fn contains(self, pitch: u8) -> bool {
        let pc = (pitch as i32 - self.root as i32).rem_euclid(12) as u8;
        self.scale.intervals().contains(&pc)
    }

    /// Nearest in-key pitch; ties go down.
    pub fn nearest(self, pitch: u8) -> u8 {
        for d in 0..12i32 {
            for cand in [pitch as i32 - d, pitch as i32 + d] {
                if (0..=127).contains(&cand) && self.contains(cand as u8) {
                    return cand as u8;
                }
            }
        }
        pitch
    }

    /// Move `pitch` by `steps` scale degrees. Out-of-key pitches are first
    /// snapped to the nearest in-key pitch. Clamped to the MIDI range.
    pub fn step(self, pitch: u8, steps: i32) -> u8 {
        let mut p = self.nearest(pitch) as i32;
        let dir = steps.signum();
        for _ in 0..steps.abs() {
            let mut next = p + dir;
            while (0..=127).contains(&next) && !self.contains(next as u8) {
                next += dir;
            }
            if !(0..=127).contains(&next) {
                break;
            }
            p = next;
        }
        p as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const C_MAJ: Key = Key {
        root: 0,
        scale: Scale::Major,
    };
    const A_MIN: Key = Key {
        root: 9,
        scale: Scale::Minor,
    };
    const C_MIN: Key = Key {
        root: 0,
        scale: Scale::Minor,
    };

    #[test]
    fn membership() {
        assert!(C_MAJ.contains(60));
        assert!(!C_MAJ.contains(61));
        assert!(C_MAJ.contains(71));
        // Relative minor has same notes.
        for p in 0..128u8 {
            assert_eq!(C_MAJ.contains(p), A_MIN.contains(p));
        }
        assert!(C_MIN.contains(63)); // Eb
        assert!(!C_MIN.contains(64)); // E
    }

    #[test]
    fn nearest_and_step() {
        assert_eq!(C_MAJ.nearest(61), 60);
        assert_eq!(C_MAJ.nearest(66), 65);
        assert_eq!(C_MAJ.step(60, 1), 62);
        assert_eq!(C_MAJ.step(64, 1), 65);
        assert_eq!(C_MAJ.step(60, -1), 59);
        assert_eq!(C_MAJ.step(60, 7), 72);
        assert_eq!(C_MAJ.step(127, 3), 127);
    }

    #[test]
    fn names() {
        assert_eq!(pitch_name(60), "C4");
        assert_eq!(
            Key {
                root: 10,
                scale: Scale::Major
            }
            .label(),
            "Bb Major"
        );
    }
}
