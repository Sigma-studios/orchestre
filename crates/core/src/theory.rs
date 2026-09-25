use serde::{Deserialize, Serialize};

/// Pitch class of each natural note C D E F G A B.
const NATURAL: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];

/// Default spelling of each pitch class as (letter, accidental), used when
/// no key says otherwise: C C# D Eb E F F# G Ab A Bb B.
const DEFAULT_SPELLING: [(usize, i32); 12] = [
    (0, 0),
    (0, 1),
    (1, 0),
    (2, -1),
    (2, 0),
    (3, 0),
    (3, 1),
    (4, 0),
    (5, -1),
    (5, 0),
    (6, -1),
    (6, 0),
];

/// How notes are named: English letters (C D E, middle C = C4) or French
/// solfège (Do Ré Mi, middle C = Do3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NoteNaming {
    #[default]
    English,
    French,
}

impl NoteNaming {
    pub const ALL: [NoteNaming; 2] = [NoteNaming::English, NoteNaming::French];

    pub fn label(self) -> &'static str {
        match self {
            NoteNaming::English => "English: C D E F G A B",
            NoteNaming::French => "French: Do Ré Mi Fa Sol La Si",
        }
    }

    fn letter(self, i: usize) -> &'static str {
        const EN: [&str; 7] = ["C", "D", "E", "F", "G", "A", "B"];
        const FR: [&str; 7] = ["Do", "Ré", "Mi", "Fa", "Sol", "La", "Si"];
        match self {
            NoteNaming::English => EN[i % 7],
            NoteNaming::French => FR[i % 7],
        }
    }

    /// Octave number of the octave starting at MIDI note `12 * (k + 1)`
    /// is `k` in English; French numbering starts one lower.
    pub fn octave(self, octave: i32) -> i32 {
        match self {
            NoteNaming::English => octave,
            NoteNaming::French => octave - 1,
        }
    }

    /// A note name from a letter index and an accidental in semitones.
    fn note(self, letter: usize, accidental: i32) -> String {
        let suffix = match accidental {
            -2 => "bb",
            -1 => "b",
            1 => "#",
            2 => "##",
            _ => "",
        };
        format!("{}{}", self.letter(letter), suffix)
    }
}

/// Name of a pitch with its octave, using the default spellings.
pub fn pitch_name(pitch: u8) -> String {
    pitch_name_in(pitch, NoteNaming::English)
}

/// Name of a pitch with its octave in the given naming, default spellings.
pub fn pitch_name_in(pitch: u8, naming: NoteNaming) -> String {
    let (letter, acc) = DEFAULT_SPELLING[pitch as usize % 12];
    let octave = naming.octave((pitch as i32 - acc) / 12 - 1);
    format!("{}{}", naming.note(letter, acc), octave)
}

pub fn is_black_key(pitch: u8) -> bool {
    matches!(pitch % 12, 1 | 3 | 6 | 8 | 10)
}

/// Whether a scale sounds major or minor (decides which keys it pairs with).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    Major,
    Minor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Scale {
    Major,
    Minor,
    MajorPentatonic,
    MinorPentatonic,
    Blues,
    HarmonicMinor,
    Dorian,
}

impl Scale {
    pub const ALL: [Scale; 7] = [
        Scale::Major,
        Scale::Minor,
        Scale::MajorPentatonic,
        Scale::MinorPentatonic,
        Scale::Blues,
        Scale::HarmonicMinor,
        Scale::Dorian,
    ];

    /// Each degree as (semitones above the tonic, letter steps above the
    /// tonic's letter). Letters give correct spellings: in A harmonic minor
    /// the 7th is G#, not Ab.
    fn degrees(self) -> &'static [(u8, u8)] {
        match self {
            Scale::Major => &[(0, 0), (2, 1), (4, 2), (5, 3), (7, 4), (9, 5), (11, 6)],
            Scale::Minor => &[(0, 0), (2, 1), (3, 2), (5, 3), (7, 4), (8, 5), (10, 6)],
            Scale::MajorPentatonic => &[(0, 0), (2, 1), (4, 2), (7, 4), (9, 5)],
            Scale::MinorPentatonic => &[(0, 0), (3, 2), (5, 3), (7, 4), (10, 6)],
            // Minor pentatonic plus the "blue note", the flat 5th.
            Scale::Blues => &[(0, 0), (3, 2), (5, 3), (6, 4), (7, 4), (10, 6)],
            Scale::HarmonicMinor => &[(0, 0), (2, 1), (3, 2), (5, 3), (7, 4), (8, 5), (11, 6)],
            Scale::Dorian => &[(0, 0), (2, 1), (3, 2), (5, 3), (7, 4), (9, 5), (10, 6)],
        }
    }

    pub fn intervals(self) -> impl Iterator<Item = u8> {
        self.degrees().iter().map(|d| d.0)
    }

    pub fn note_count(self) -> usize {
        self.degrees().len()
    }

    pub fn family(self) -> Family {
        match self {
            Scale::Major | Scale::MajorPentatonic => Family::Major,
            _ => Family::Minor,
        }
    }

    pub fn label(self) -> &'static str {
        self.label_in(NoteNaming::English)
    }

    pub fn label_in(self, naming: NoteNaming) -> &'static str {
        match (naming, self) {
            (NoteNaming::English, Scale::Major) => "Major",
            (NoteNaming::English, Scale::Minor) => "Minor",
            (NoteNaming::English, Scale::MajorPentatonic) => "Major Pentatonic",
            (NoteNaming::English, Scale::MinorPentatonic) => "Minor Pentatonic",
            (NoteNaming::English, Scale::Blues) => "Blues",
            (NoteNaming::English, Scale::HarmonicMinor) => "Harmonic Minor",
            (NoteNaming::English, Scale::Dorian) => "Dorian",
            (NoteNaming::French, Scale::Major) => "majeur",
            (NoteNaming::French, Scale::Minor) => "mineur",
            (NoteNaming::French, Scale::MajorPentatonic) => "pentatonique majeure",
            (NoteNaming::French, Scale::MinorPentatonic) => "pentatonique mineure",
            (NoteNaming::French, Scale::Blues) => "blues",
            (NoteNaming::French, Scale::HarmonicMinor) => "mineur harmonique",
            (NoteNaming::French, Scale::Dorian) => "dorien",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Scale::Major => "All 7 notes of the key",
            Scale::Minor => "All 7 notes of the key",
            Scale::MajorPentatonic => "5 notes: hard to play a wrong note",
            Scale::MinorPentatonic => "5 notes: the classic riff and solo scale",
            Scale::Blues => "Minor pentatonic plus the \"blue note\"",
            Scale::HarmonicMinor => "Minor with a raised 7th: dramatic, classical",
            Scale::Dorian => "Minor with a bright 6th: funk, lo-fi, soul",
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
    pub const fn new(root: u8, scale: Scale) -> Key {
        Key { root, scale }
    }

    /// The most common keys in popular music, offered as presets
    /// (roughly by popularity within major and minor).
    pub const PRESETS: [Key; 18] = [
        Key::new(0, Scale::Major),
        Key::new(7, Scale::Major),
        Key::new(2, Scale::Major),
        Key::new(9, Scale::Major),
        Key::new(4, Scale::Major),
        Key::new(5, Scale::Major),
        Key::new(1, Scale::Major),
        Key::new(10, Scale::Major),
        Key::new(3, Scale::Major),
        Key::new(9, Scale::Minor),
        Key::new(4, Scale::Minor),
        Key::new(11, Scale::Minor),
        Key::new(2, Scale::Minor),
        Key::new(6, Scale::Minor),
        Key::new(1, Scale::Minor),
        Key::new(7, Scale::Minor),
        Key::new(0, Scale::Minor),
        Key::new(5, Scale::Minor),
    ];

    /// The tonic as (letter, accidental). Each key is written with the
    /// fewest accidentals, the way musicians name it (Db major, C# minor).
    fn tonic(self) -> (usize, i32) {
        const MAJOR: [(usize, i32); 12] = [
            (0, 0),
            (1, -1),
            (1, 0),
            (2, -1),
            (2, 0),
            (3, 0),
            (3, 1),
            (4, 0),
            (5, -1),
            (5, 0),
            (6, -1),
            (6, 0),
        ];
        const MINOR: [(usize, i32); 12] = [
            (0, 0),
            (0, 1),
            (1, 0),
            (2, -1),
            (2, 0),
            (3, 0),
            (3, 1),
            (4, 0),
            (4, 1),
            (5, 0),
            (6, -1),
            (6, 0),
        ];
        let table = match self.scale.family() {
            Family::Major => MAJOR,
            Family::Minor => MINOR,
        };
        table[self.root as usize % 12]
    }

    pub fn tonic_name(self) -> String {
        self.tonic_name_in(NoteNaming::English)
    }

    pub fn tonic_name_in(self, naming: NoteNaming) -> String {
        let (letter, acc) = self.tonic();
        naming.note(letter, acc)
    }

    pub fn label(self) -> String {
        self.label_in(NoteNaming::English)
    }

    /// "A Minor" in English, "La mineur" in French.
    pub fn label_in(self, naming: NoteNaming) -> String {
        format!(
            "{} {}",
            self.tonic_name_in(naming),
            self.scale.label_in(naming)
        )
    }

    /// The key as a preset (tonic + major/minor feel), ignoring the scale variant.
    pub fn base(self) -> Key {
        let scale = match self.scale.family() {
            Family::Major => Scale::Major,
            Family::Minor => Scale::Minor,
        };
        Key::new(self.root, scale)
    }

    pub fn contains(self, pitch: u8) -> bool {
        let pc = (pitch as i32 - self.root as i32).rem_euclid(12) as u8;
        self.scale.intervals().any(|i| i == pc)
    }

    pub fn is_tonic(self, pitch: u8) -> bool {
        pitch % 12 == self.root % 12
    }

    /// Name of `pitch` (without octave) spelled for this key: scale notes
    /// get their proper letters; other notes follow the key's sharps/flats.
    pub fn spell(self, pitch: u8) -> String {
        self.spell_in(pitch, NoteNaming::English)
    }

    pub fn spell_in(self, pitch: u8, naming: NoteNaming) -> String {
        let (letter, acc) = self.spell_parts(pitch);
        naming.note(letter, acc)
    }

    /// Name with octave number, e.g. "G#4". The octave follows the letter,
    /// so the C above B is written B#3 where the key calls for it.
    pub fn spell_with_octave(self, pitch: u8) -> String {
        self.spell_with_octave_in(pitch, NoteNaming::English)
    }

    pub fn spell_with_octave_in(self, pitch: u8, naming: NoteNaming) -> String {
        let (letter, acc) = self.spell_parts(pitch);
        let octave = naming.octave((pitch as i32 - acc) / 12 - 1);
        format!("{}{}", naming.note(letter, acc), octave)
    }

    /// (letter, accidental) of `pitch` spelled for this key.
    fn spell_parts(self, pitch: u8) -> (usize, i32) {
        let tonic_letter = self.tonic().0;
        let pc = (pitch as i32).rem_euclid(12);
        let interval = (pc - self.root as i32).rem_euclid(12) as u8;
        if let Some(&(_, steps)) = self.scale.degrees().iter().find(|d| d.0 == interval) {
            let letter = (tonic_letter + steps as usize) % 7;
            return (letter, accidental_of(letter, pc));
        }
        // Not in the scale: spell with the key's preferred accidental.
        let flats = self.scale.degrees().iter().any(|&(i, steps)| {
            let letter = (tonic_letter + steps as usize) % 7;
            let pc = (self.root as i32 + i as i32).rem_euclid(12);
            accidental_of(letter, pc) < 0
        });
        let letter = (0..7)
            .filter(|&l| {
                let a = accidental_of(l, pc);
                if flats {
                    a == 0 || a == -1
                } else {
                    a == 0 || a == 1
                }
            })
            .min_by_key(|&l| accidental_of(l, pc).abs())
            .unwrap_or(0);
        (letter, accidental_of(letter, pc))
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

    /// Where `pitch` goes when a song moves from `self` to `to`: the same
    /// scale degree when both scales have the same number of notes (so a
    /// melody keeps its shape, and major ↔ minor works), otherwise shifted
    /// by the distance between tonics and snapped into the new key.
    pub fn transpose_to(self, to: Key, pitch: u8) -> u8 {
        // Shift by the smallest interval between the two tonics.
        let shift = (to.root as i32 - self.root as i32 + 6).rem_euclid(12) - 6;
        let shifted = (pitch as i32 + shift).clamp(0, 127) as u8;
        if self.scale.note_count() != to.scale.note_count() || !self.contains(pitch) {
            return to.nearest(shifted);
        }
        let interval = (pitch as i32 - self.root as i32).rem_euclid(12) as u8;
        let degree = self
            .scale
            .intervals()
            .position(|i| i == interval)
            .unwrap_or(0);
        let new_interval = to.scale.intervals().nth(degree).unwrap_or(0) as i32;
        // The new pitch with that interval, in the octave nearest the shifted pitch.
        let base = shifted as i32 - (shifted as i32 - to.root as i32).rem_euclid(12);
        let candidates = [base - 12, base, base + 12].map(|b| b + new_interval);
        let best = candidates
            .into_iter()
            .min_by_key(|&c| (c - shifted as i32).abs())
            .unwrap_or(shifted as i32);
        best.clamp(0, 127) as u8
    }
}

/// Accidental (in semitones, -2..=2) needed for `letter` to sound as `pc`.
fn accidental_of(letter: usize, pc: i32) -> i32 {
    (pc - NATURAL[letter] + 6).rem_euclid(12) - 6
}

#[cfg(test)]
mod tests {
    use super::*;

    const C_MAJ: Key = Key::new(0, Scale::Major);
    const A_MIN: Key = Key::new(9, Scale::Minor);
    const C_MIN: Key = Key::new(0, Scale::Minor);

    fn names(key: Key) -> Vec<String> {
        (0..12)
            .map(|i| key.root + i)
            .filter(|&p| key.contains(p))
            .map(|p| key.spell(p))
            .collect()
    }

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
        let pent = Key::new(9, Scale::MinorPentatonic);
        // A C D E G: one step up from E is G.
        assert_eq!(pent.step(64, 1), 67);
    }

    #[test]
    fn spelling_follows_the_key() {
        assert_eq!(
            names(Key::new(4, Scale::Major)),
            ["E", "F#", "G#", "A", "B", "C#", "D#"]
        );
        assert_eq!(
            names(Key::new(5, Scale::Major)),
            ["F", "G", "A", "Bb", "C", "D", "E"]
        );
        assert_eq!(
            names(Key::new(1, Scale::Major)),
            ["Db", "Eb", "F", "Gb", "Ab", "Bb", "C"]
        );
        assert_eq!(
            names(Key::new(1, Scale::Minor)),
            ["C#", "D#", "E", "F#", "G#", "A", "B"]
        );
        assert_eq!(
            names(Key::new(9, Scale::HarmonicMinor)),
            ["A", "B", "C", "D", "E", "F", "G#"]
        );
        assert_eq!(
            names(Key::new(9, Scale::Blues)),
            ["A", "C", "D", "Eb", "E", "G"]
        );
        assert_eq!(
            names(Key::new(2, Scale::Dorian)),
            ["D", "E", "F", "G", "A", "B", "C"]
        );
        // Out-of-key notes follow the key's accidentals.
        assert_eq!(Key::new(5, Scale::Major).spell(61), "Db");
        assert_eq!(Key::new(4, Scale::Major).spell(70), "A#");
        // The octave number follows the letter.
        assert_eq!(
            Key::new(1, Scale::HarmonicMinor).spell_with_octave(60),
            "B#3"
        );
        assert_eq!(C_MAJ.spell_with_octave(60), "C4");
    }

    #[test]
    fn labels_use_conventional_names() {
        assert_eq!(Key::new(10, Scale::Major).label(), "Bb Major");
        assert_eq!(Key::new(1, Scale::Major).label(), "Db Major");
        assert_eq!(Key::new(1, Scale::Minor).label(), "C# Minor");
        assert_eq!(
            Key::new(9, Scale::MinorPentatonic).label(),
            "A Minor Pentatonic"
        );
        assert_eq!(pitch_name(60), "C4");
    }

    #[test]
    fn french_naming() {
        let fr = NoteNaming::French;
        // Middle C is Do3 in French numbering.
        assert_eq!(pitch_name_in(60, fr), "Do3");
        assert_eq!(pitch_name_in(70, fr), "Sib3");
        assert_eq!(Key::new(9, Scale::Minor).label_in(fr), "La mineur");
        assert_eq!(Key::new(1, Scale::Major).label_in(fr), "Réb majeur");
        assert_eq!(Key::new(1, Scale::Minor).label_in(fr), "Do# mineur");
        assert_eq!(
            Key::new(9, Scale::MinorPentatonic).label_in(fr),
            "La pentatonique mineure"
        );
        assert_eq!(
            Key::new(4, Scale::Major).spell_with_octave_in(68, fr),
            "Sol#3"
        );
        assert_eq!(
            Key::new(1, Scale::HarmonicMinor).spell_with_octave_in(60, fr),
            "Si#2"
        );
    }

    #[test]
    fn transposing_keeps_melodies_in_key() {
        // C major → G major: the smallest move is down a fourth.
        let g = Key::new(7, Scale::Major);
        assert_eq!(C_MAJ.transpose_to(g, 60), 55); // C4 → G3
        assert_eq!(C_MAJ.transpose_to(g, 65), 60); // F → C, not F#
        // C major → D major: up a tone.
        assert_eq!(C_MAJ.transpose_to(Key::new(2, Scale::Major), 64), 66); // E → F#
        // C major → C minor: same degree, so E becomes Eb.
        assert_eq!(C_MAJ.transpose_to(C_MIN, 64), 63);
        // Into a 5-note scale, notes snap into it.
        let pent = Key::new(0, Scale::MajorPentatonic);
        assert!(pent.contains(C_MAJ.transpose_to(pent, 65)));
        for p in 36..96u8 {
            if C_MAJ.contains(p) {
                assert!(g.contains(C_MAJ.transpose_to(g, p)));
                assert!(C_MIN.contains(C_MAJ.transpose_to(C_MIN, p)));
            }
        }
    }
}
