use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Wave {
    Sine,
    Triangle,
    Saw,
    Square,
    /// 25% pulse.
    Pulse,
}

impl Wave {
    pub const ALL: [Wave; 5] = [
        Wave::Sine,
        Wave::Triangle,
        Wave::Saw,
        Wave::Square,
        Wave::Pulse,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Wave::Sine => "Sine",
            Wave::Triangle => "Triangle",
            Wave::Saw => "Saw",
            Wave::Square => "Square",
            Wave::Pulse => "Pulse",
        }
    }
}

/// Envelope times are in seconds, sustain is a 0..1 level.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Adsr {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
}

impl Adsr {
    pub const fn new(attack: f32, decay: f32, sustain: f32, release: f32) -> Self {
        Adsr {
            attack,
            decay,
            sustain,
            release,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SynthPreset {
    Lead,
    Bass,
    SubBass,
    Pad,
    Pluck,
    Bell,
    Supersaw,
    Chiptune,
    Bass808,
    WobbleBass,
    Brass,
    Flute,
    Strings,
    ArpPluck,
}

impl SynthPreset {
    pub const ALL: [SynthPreset; 14] = [
        SynthPreset::Lead,
        SynthPreset::Bass,
        SynthPreset::SubBass,
        SynthPreset::Pad,
        SynthPreset::Pluck,
        SynthPreset::Bell,
        SynthPreset::Supersaw,
        SynthPreset::Chiptune,
        SynthPreset::Bass808,
        SynthPreset::WobbleBass,
        SynthPreset::Brass,
        SynthPreset::Flute,
        SynthPreset::Strings,
        SynthPreset::ArpPluck,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SynthPreset::Lead => "Lead",
            SynthPreset::Bass => "Bass",
            SynthPreset::SubBass => "Sub Bass",
            SynthPreset::Pad => "Pad",
            SynthPreset::Pluck => "Pluck",
            SynthPreset::Bell => "Bell Keys",
            SynthPreset::Supersaw => "Supersaw",
            SynthPreset::Chiptune => "Chiptune",
            SynthPreset::Bass808 => "808 Bass",
            SynthPreset::WobbleBass => "Wobble Bass",
            SynthPreset::Brass => "Brass",
            SynthPreset::Flute => "Flute",
            SynthPreset::Strings => "Strings",
            SynthPreset::ArpPluck => "Arp Pluck",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            SynthPreset::Lead => "Bright, singing melody sound",
            SynthPreset::Bass => "Punchy bass line",
            SynthPreset::SubBass => "Deep, round low end",
            SynthPreset::Pad => "Soft, slow background chords",
            SynthPreset::Pluck => "Short, plucked notes",
            SynthPreset::Bell => "Shimmering bell-like keys",
            SynthPreset::Supersaw => "Huge, wide chords",
            SynthPreset::Chiptune => "Retro video game sound",
            SynthPreset::Bass808 => "Booming hip-hop bass that bends down",
            SynthPreset::WobbleBass => "Growling bass that wobbles",
            SynthPreset::Brass => "Bold horn-section stabs",
            SynthPreset::Flute => "Soft, breathy whistle",
            SynthPreset::Strings => "Lush string section",
            SynthPreset::ArpPluck => "Bouncy plucks for fast patterns",
        }
    }

    pub fn params(self) -> SynthParams {
        let base = SynthParams::default();
        match self {
            SynthPreset::Lead => SynthParams {
                preset: self,
                osc1: Wave::Saw,
                osc2: Wave::Square,
                osc2_semitones: 0,
                osc2_detune: 7.0,
                osc_mix: 0.4,
                cutoff: 3200.0,
                resonance: 0.25,
                filter_env: 1.5,
                lfo_rate: 5.5,
                lfo_pitch: 0.12,
                mono: true,
                glide: 0.06,
                amp: Adsr::new(0.01, 0.2, 0.8, 0.15),
                ..base
            },
            SynthPreset::Bass => SynthParams {
                preset: self,
                osc1: Wave::Saw,
                osc2: Wave::Square,
                osc2_semitones: -12,
                osc_mix: 0.5,
                cutoff: 450.0,
                resonance: 0.35,
                filter_env: 3.0,
                filter_adsr: Adsr::new(0.002, 0.18, 0.1, 0.1),
                amp: Adsr::new(0.003, 0.3, 0.7, 0.08),
                mono: true,
                glide: 0.03,
                ..base
            },
            SynthPreset::SubBass => SynthParams {
                preset: self,
                osc1: Wave::Sine,
                osc2: Wave::Triangle,
                osc_mix: 0.2,
                cutoff: 900.0,
                resonance: 0.0,
                filter_env: 0.0,
                amp: Adsr::new(0.005, 0.2, 0.9, 0.12),
                mono: true,
                glide: 0.02,
                ..base
            },
            SynthPreset::Pad => SynthParams {
                preset: self,
                osc1: Wave::Saw,
                osc2: Wave::Saw,
                osc2_detune: 12.0,
                osc_mix: 0.5,
                unison: 3,
                unison_spread: 18.0,
                width: 0.8,
                cutoff: 1400.0,
                resonance: 0.1,
                filter_env: 0.8,
                filter_adsr: Adsr::new(0.8, 1.5, 0.6, 1.2),
                lfo_rate: 0.3,
                lfo_cutoff: 0.4,
                amp: Adsr::new(0.6, 1.0, 0.85, 1.5),
                gain: 0.6,
                ..base
            },
            SynthPreset::Pluck => SynthParams {
                preset: self,
                osc1: Wave::Saw,
                osc2: Wave::Pulse,
                osc2_semitones: 12,
                osc_mix: 0.3,
                cutoff: 600.0,
                resonance: 0.2,
                filter_env: 4.0,
                filter_adsr: Adsr::new(0.001, 0.22, 0.0, 0.2),
                amp: Adsr::new(0.001, 0.45, 0.0, 0.3),
                width: 0.4,
                ..base
            },
            SynthPreset::Bell => SynthParams {
                preset: self,
                osc1: Wave::Sine,
                osc2: Wave::Sine,
                osc2_semitones: 19,
                osc2_detune: 3.0,
                osc_mix: 0.1,
                fm: 2.2,
                cutoff: 8000.0,
                filter_env: 0.0,
                amp: Adsr::new(0.002, 1.6, 0.0, 1.0),
                width: 0.3,
                ..base
            },
            SynthPreset::Supersaw => SynthParams {
                preset: self,
                osc1: Wave::Saw,
                osc2: Wave::Saw,
                osc2_semitones: 12,
                osc_mix: 0.25,
                unison: 5,
                unison_spread: 25.0,
                width: 1.0,
                cutoff: 5000.0,
                resonance: 0.1,
                filter_env: 0.5,
                amp: Adsr::new(0.02, 0.4, 0.8, 0.4),
                gain: 0.5,
                ..base
            },
            SynthPreset::Chiptune => SynthParams {
                preset: self,
                osc1: Wave::Pulse,
                osc2: Wave::Square,
                osc2_semitones: 12,
                osc_mix: 0.15,
                cutoff: 12000.0,
                filter_env: 0.0,
                lfo_rate: 7.0,
                lfo_pitch: 0.15,
                amp: Adsr::new(0.001, 0.1, 0.7, 0.05),
                gain: 0.55,
                ..base
            },
            SynthPreset::Bass808 => SynthParams {
                preset: self,
                osc1: Wave::Sine,
                osc2: Wave::Triangle,
                osc_mix: 0.15,
                cutoff: 1200.0,
                resonance: 0.0,
                filter_env: 0.0,
                amp: Adsr::new(0.002, 2.5, 0.0, 0.3),
                bend: 7.0,
                bend_time: 0.06,
                mono: true,
                glide: 0.05,
                octave: -1,
                gain: 0.95,
                ..base
            },
            SynthPreset::WobbleBass => SynthParams {
                preset: self,
                osc1: Wave::Saw,
                osc2: Wave::Square,
                osc2_semitones: -12,
                osc2_detune: 8.0,
                osc_mix: 0.5,
                cutoff: 280.0,
                resonance: 0.6,
                filter_env: 0.5,
                lfo_rate: 2.75,
                lfo_cutoff: 2.8,
                amp: Adsr::new(0.005, 0.2, 0.9, 0.1),
                mono: true,
                glide: 0.04,
                gain: 0.75,
                ..base
            },
            SynthPreset::Brass => SynthParams {
                preset: self,
                osc1: Wave::Saw,
                osc2: Wave::Saw,
                osc2_detune: 9.0,
                osc_mix: 0.5,
                cutoff: 700.0,
                resonance: 0.15,
                filter_env: 2.2,
                filter_adsr: Adsr::new(0.06, 0.35, 0.45, 0.2),
                amp: Adsr::new(0.03, 0.3, 0.8, 0.15),
                lfo_rate: 5.0,
                lfo_pitch: 0.06,
                width: 0.4,
                gain: 0.85,
                ..base
            },
            SynthPreset::Flute => SynthParams {
                preset: self,
                osc1: Wave::Sine,
                osc2: Wave::Triangle,
                osc2_semitones: 12,
                osc_mix: 0.15,
                noise: 0.12,
                cutoff: 3500.0,
                resonance: 0.0,
                filter_env: 0.3,
                amp: Adsr::new(0.07, 0.2, 0.85, 0.15),
                lfo_rate: 5.2,
                lfo_pitch: 0.12,
                gain: 0.75,
                ..base
            },
            SynthPreset::Strings => SynthParams {
                preset: self,
                osc1: Wave::Saw,
                osc2: Wave::Saw,
                osc2_detune: 7.0,
                osc_mix: 0.5,
                unison: 3,
                unison_spread: 14.0,
                width: 0.9,
                cutoff: 2400.0,
                resonance: 0.05,
                filter_env: 0.4,
                filter_adsr: Adsr::new(0.4, 1.0, 0.7, 0.8),
                amp: Adsr::new(0.35, 0.5, 0.9, 0.9),
                lfo_rate: 4.8,
                lfo_pitch: 0.05,
                chorus: 0.7,
                gain: 0.55,
                ..base
            },
            SynthPreset::ArpPluck => SynthParams {
                preset: self,
                osc1: Wave::Square,
                osc2: Wave::Saw,
                osc2_semitones: 12,
                osc2_detune: 5.0,
                osc_mix: 0.35,
                cutoff: 800.0,
                resonance: 0.3,
                filter_env: 3.5,
                filter_adsr: Adsr::new(0.001, 0.12, 0.0, 0.1),
                amp: Adsr::new(0.001, 0.22, 0.0, 0.12),
                width: 0.6,
                chorus: 0.3,
                ..base
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SynthParams {
    pub preset: SynthPreset,
    pub osc1: Wave,
    pub osc2: Wave,
    pub osc2_semitones: i8,
    /// Cents.
    pub osc2_detune: f32,
    /// 0 = only osc1, 1 = only osc2.
    pub osc_mix: f32,
    /// Phase modulation of osc1 by osc2 (0 = off).
    pub fm: f32,
    /// Square sub-oscillator one octave below, 0..1.
    pub sub: f32,
    pub noise: f32,
    /// Number of stacked detuned copies of each oscillator (1..=5).
    pub unison: u8,
    /// Cents between outermost unison voices.
    pub unison_spread: f32,
    /// Stereo spread of unison voices, 0..1.
    pub width: f32,
    /// Hz.
    pub cutoff: f32,
    /// 0..1.
    pub resonance: f32,
    /// Filter envelope depth in octaves.
    pub filter_env: f32,
    pub filter_adsr: Adsr,
    pub amp: Adsr,
    pub lfo_rate: f32,
    /// Vibrato depth in semitones.
    pub lfo_pitch: f32,
    /// Filter wobble depth in octaves.
    pub lfo_cutoff: f32,
    pub mono: bool,
    /// Portamento time in seconds (mono only).
    pub glide: f32,
    pub octave: i8,
    pub gain: f32,
    /// Pitch starts this many semitones up and falls to the note (808 bass).
    #[serde(default)]
    pub bend: f32,
    /// Seconds for the bend to settle.
    #[serde(default)]
    pub bend_time: f32,
    /// Stereo chorus amount, 0..1.
    #[serde(default)]
    pub chorus: f32,
}

impl Default for SynthParams {
    fn default() -> Self {
        SynthParams {
            preset: SynthPreset::Lead,
            osc1: Wave::Saw,
            osc2: Wave::Square,
            osc2_semitones: 0,
            osc2_detune: 0.0,
            osc_mix: 0.0,
            fm: 0.0,
            sub: 0.0,
            noise: 0.0,
            unison: 1,
            unison_spread: 0.0,
            width: 0.2,
            cutoff: 2000.0,
            resonance: 0.2,
            filter_env: 1.0,
            filter_adsr: Adsr::new(0.005, 0.3, 0.3, 0.2),
            amp: Adsr::new(0.005, 0.2, 0.8, 0.2),
            lfo_rate: 5.0,
            lfo_pitch: 0.0,
            lfo_cutoff: 0.0,
            mono: false,
            glide: 0.0,
            octave: 0,
            gain: 0.7,
            bend: 0.0,
            bend_time: 0.0,
            chorus: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DrumPiece {
    Kick,
    Snare,
    Clap,
    Rim,
    ClosedHat,
    OpenHat,
    LowTom,
    MidTom,
    HighTom,
    Crash,
    Cowbell,
    Tambourine,
    Shaker,
    Claves,
    HighConga,
    LowConga,
    Bongo,
}

impl DrumPiece {
    /// Number of core kit pieces (home-row keys); the rest is percussion
    /// (black-key row).
    pub const KIT_LEN: usize = 10;

    /// Index = the "pitch" used by notes on a drum track. New pieces are
    /// only ever appended, so saved songs keep their meaning.
    pub const ALL: [DrumPiece; 17] = [
        DrumPiece::Kick,
        DrumPiece::Snare,
        DrumPiece::Clap,
        DrumPiece::Rim,
        DrumPiece::ClosedHat,
        DrumPiece::OpenHat,
        DrumPiece::LowTom,
        DrumPiece::MidTom,
        DrumPiece::HighTom,
        DrumPiece::Crash,
        DrumPiece::Cowbell,
        DrumPiece::Tambourine,
        DrumPiece::Shaker,
        DrumPiece::Claves,
        DrumPiece::HighConga,
        DrumPiece::LowConga,
        DrumPiece::Bongo,
    ];

    pub fn from_pitch(p: u8) -> Option<DrumPiece> {
        Self::ALL.get(p as usize).copied()
    }

    pub fn label(self) -> &'static str {
        match self {
            DrumPiece::Kick => "Kick",
            DrumPiece::Snare => "Snare",
            DrumPiece::Clap => "Clap",
            DrumPiece::Rim => "Rim",
            DrumPiece::ClosedHat => "Hi-hat",
            DrumPiece::OpenHat => "Open hat",
            DrumPiece::LowTom => "Low tom",
            DrumPiece::MidTom => "Mid tom",
            DrumPiece::HighTom => "High tom",
            DrumPiece::Crash => "Crash",
            DrumPiece::Cowbell => "Cowbell",
            DrumPiece::Tambourine => "Tambourine",
            DrumPiece::Shaker => "Shaker",
            DrumPiece::Claves => "Claves",
            DrumPiece::HighConga => "High conga",
            DrumPiece::LowConga => "Low conga",
            DrumPiece::Bongo => "Bongo",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrumKit {
    Punchy,
    Tight,
    Boomy808,
}

impl DrumKit {
    pub const ALL: [DrumKit; 3] = [DrumKit::Punchy, DrumKit::Tight, DrumKit::Boomy808];

    pub fn label(self) -> &'static str {
        match self {
            DrumKit::Punchy => "Punchy",
            DrumKit::Tight => "Tight",
            DrumKit::Boomy808 => "Boomy 808",
        }
    }

    pub fn params(self) -> DrumParams {
        match self {
            DrumKit::Punchy => DrumParams {
                kit: self,
                ..DrumParams::default()
            },
            DrumKit::Tight => DrumParams {
                kit: self,
                tune: 3.0,
                decay: 0.6,
                punch: 0.7,
                snappy: 0.8,
                ..DrumParams::default()
            },
            DrumKit::Boomy808 => DrumParams {
                kit: self,
                tune: -3.0,
                decay: 2.2,
                punch: 0.2,
                snappy: 0.4,
                ..DrumParams::default()
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrumParams {
    pub kit: DrumKit,
    /// Semitones.
    pub tune: f32,
    /// Decay-time multiplier.
    pub decay: f32,
    /// Kick click / attack amount, 0..1.
    pub punch: f32,
    /// Snare noise amount, 0..1.
    pub snappy: f32,
    /// Volume of each core kit piece.
    pub levels: [f32; 10],
    /// Volume of each percussion piece (added later; defaults for old songs).
    #[serde(default = "default_perc_levels")]
    pub perc_levels: [f32; 7],
    pub gain: f32,
}

fn default_perc_levels() -> [f32; 7] {
    [1.0; 7]
}

impl DrumParams {
    /// Volume of the drum piece with index `i` (see [`DrumPiece::ALL`]).
    pub fn level(&self, i: usize) -> f32 {
        if i < DrumPiece::KIT_LEN {
            self.levels[i]
        } else {
            self.perc_levels
                .get(i - DrumPiece::KIT_LEN)
                .copied()
                .unwrap_or(1.0)
        }
    }

    pub fn level_mut(&mut self, i: usize) -> &mut f32 {
        if i < DrumPiece::KIT_LEN {
            &mut self.levels[i]
        } else {
            &mut self.perc_levels[(i - DrumPiece::KIT_LEN).min(6)]
        }
    }
}

impl Default for DrumParams {
    fn default() -> Self {
        DrumParams {
            kit: DrumKit::Punchy,
            tune: 0.0,
            decay: 1.0,
            punch: 0.5,
            snappy: 0.6,
            levels: [1.0; 10],
            perc_levels: default_perc_levels(),
            gain: 0.8,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PianoParams {
    /// Tone color, 0..1.
    pub brightness: f32,
    /// Sustain length multiplier.
    pub decay: f32,
    /// Hammer noise, 0..1.
    pub hardness: f32,
    /// String detune ("honky-tonk"), 0..1.
    pub detune: f32,
    /// Keep ringing after the note ends, like holding the sustain pedal.
    pub pedal: bool,
    pub gain: f32,
}

impl Default for PianoParams {
    fn default() -> Self {
        PianoParams {
            brightness: 0.5,
            decay: 1.0,
            hardness: 0.4,
            detune: 0.15,
            pedal: false,
            gain: 0.8,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MalletKind {
    Marimba,
    Vibraphone,
    Xylophone,
    Glockenspiel,
}

impl MalletKind {
    pub const ALL: [MalletKind; 4] = [
        MalletKind::Marimba,
        MalletKind::Vibraphone,
        MalletKind::Xylophone,
        MalletKind::Glockenspiel,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MalletKind::Marimba => "Marimba",
            MalletKind::Vibraphone => "Vibraphone",
            MalletKind::Xylophone => "Xylophone",
            MalletKind::Glockenspiel => "Glockenspiel",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            MalletKind::Marimba => "Warm wooden bars",
            MalletKind::Vibraphone => "Dreamy metal bars with vibrato",
            MalletKind::Xylophone => "Bright, dry wooden bars",
            MalletKind::Glockenspiel => "Tiny sparkling bells",
        }
    }

    pub fn params(self) -> MalletParams {
        let base = MalletParams {
            kind: self,
            hardness: 0.5,
            decay: 1.0,
            tremolo: 0.0,
            damper: false,
            gain: 0.8,
        };
        match self {
            MalletKind::Vibraphone => MalletParams {
                tremolo: 0.5,
                damper: true,
                hardness: 0.35,
                ..base
            },
            MalletKind::Xylophone => MalletParams {
                hardness: 0.75,
                ..base
            },
            MalletKind::Glockenspiel => MalletParams {
                hardness: 0.6,
                gain: 0.6,
                ..base
            },
            MalletKind::Marimba => base,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MalletParams {
    pub kind: MalletKind,
    /// Mallet hardness: brighter attack, 0..1.
    pub hardness: f32,
    /// Ring length multiplier.
    pub decay: f32,
    /// Vibraphone-style tremolo depth, 0..1.
    pub tremolo: f32,
    /// Stop ringing when the note ends.
    pub damper: bool,
    pub gain: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluckKind {
    Guitar,
    Harp,
    BassGuitar,
    Koto,
}

impl PluckKind {
    pub const ALL: [PluckKind; 4] = [
        PluckKind::Guitar,
        PluckKind::Harp,
        PluckKind::BassGuitar,
        PluckKind::Koto,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PluckKind::Guitar => "Guitar",
            PluckKind::Harp => "Harp",
            PluckKind::BassGuitar => "Bass Guitar",
            PluckKind::Koto => "Koto",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            PluckKind::Guitar => "Acoustic guitar strings",
            PluckKind::Harp => "Soft, ringing harp",
            PluckKind::BassGuitar => "Round electric bass",
            PluckKind::Koto => "Twangy Japanese zither",
        }
    }

    pub fn params(self) -> PluckParams {
        match self {
            PluckKind::Guitar => PluckParams {
                kind: self,
                brightness: 0.55,
                decay: 1.0,
                position: 0.2,
                body: 0.5,
                ring: false,
                gain: 0.8,
            },
            PluckKind::Harp => PluckParams {
                kind: self,
                brightness: 0.4,
                decay: 1.3,
                position: 0.45,
                body: 0.2,
                ring: true,
                gain: 0.8,
            },
            PluckKind::BassGuitar => PluckParams {
                kind: self,
                brightness: 0.3,
                decay: 0.9,
                position: 0.15,
                body: 0.6,
                ring: false,
                gain: 0.9,
            },
            PluckKind::Koto => PluckParams {
                kind: self,
                brightness: 0.8,
                decay: 0.8,
                position: 0.1,
                body: 0.3,
                ring: true,
                gain: 0.75,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PluckParams {
    pub kind: PluckKind,
    /// String brightness, 0..1.
    pub brightness: f32,
    /// Ring length multiplier.
    pub decay: f32,
    /// Where the string is plucked, 0 (near the end: thin) .. 1 (middle: round).
    pub position: f32,
    /// Wooden body resonance, 0..1.
    pub body: f32,
    /// Keep ringing after the note ends (otherwise the string is muted).
    pub ring: bool,
    pub gain: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrganPreset {
    Jazz,
    Rock,
    Church,
}

impl OrganPreset {
    pub const ALL: [OrganPreset; 3] = [OrganPreset::Jazz, OrganPreset::Rock, OrganPreset::Church];

    pub fn label(self) -> &'static str {
        match self {
            OrganPreset::Jazz => "Jazz Organ",
            OrganPreset::Rock => "Rock Organ",
            OrganPreset::Church => "Church Organ",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            OrganPreset::Jazz => "Smooth tonewheel organ",
            OrganPreset::Rock => "Gritty organ with a spinning speaker",
            OrganPreset::Church => "Big, full pipe-like organ",
        }
    }

    pub fn params(self) -> OrganParams {
        // Drawbars: 16', 5 1/3', 8', 4', 2 2/3', 2', 1 3/5', 1 1/3', 1'.
        match self {
            OrganPreset::Jazz => OrganParams {
                preset: self,
                drawbars: [1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                percussion: true,
                click: 0.4,
                rotary: 0.25,
                gain: 0.7,
            },
            OrganPreset::Rock => OrganParams {
                preset: self,
                drawbars: [1.0, 1.0, 1.0, 1.0, 0.5, 0.4, 0.0, 0.0, 0.3],
                percussion: false,
                click: 0.6,
                rotary: 0.9,
                gain: 0.6,
            },
            OrganPreset::Church => OrganParams {
                preset: self,
                drawbars: [0.9, 0.5, 1.0, 0.8, 0.5, 0.7, 0.3, 0.3, 0.6],
                percussion: false,
                click: 0.0,
                rotary: 0.0,
                gain: 0.55,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct OrganParams {
    pub preset: OrganPreset,
    /// Levels of the nine harmonics, 0..1.
    pub drawbars: [f32; 9],
    /// Short percussive "ping" on the 3rd harmonic.
    pub percussion: bool,
    /// Key click, 0..1.
    pub click: f32,
    /// Rotating speaker: 0 = off, 1 = fast.
    pub rotary: f32,
    pub gain: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EPianoParams {
    /// Bark / brightness (FM depth), 0..1.
    pub tone: f32,
    /// Metallic tine "ping" on the attack, 0..1.
    pub bell: f32,
    /// Ring length multiplier.
    pub decay: f32,
    /// Stereo tremolo, 0..1.
    pub tremolo: f32,
    pub gain: f32,
}

impl Default for EPianoParams {
    fn default() -> Self {
        EPianoParams {
            tone: 0.45,
            bell: 0.35,
            decay: 1.0,
            tremolo: 0.3,
            gain: 0.8,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChoirParams {
    /// Vowel: 0 = "aah", 0.5 = "ooh", 1 = "eeh".
    pub vowel: f32,
    /// Breath noise, 0..1.
    pub breath: f32,
    /// Seconds.
    pub attack: f32,
    /// Seconds.
    pub release: f32,
    /// Number of voices per note and their detune, 0..1.
    pub ensemble: f32,
    /// Vibrato depth, 0..1.
    pub vibrato: f32,
    pub gain: f32,
}

impl Default for ChoirParams {
    fn default() -> Self {
        ChoirParams {
            vowel: 0.0,
            breath: 0.15,
            attack: 0.25,
            release: 0.5,
            ensemble: 0.6,
            vibrato: 0.4,
            gain: 0.8,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Instrument {
    Synth(SynthParams),
    Drums(DrumParams),
    Piano(PianoParams),
    EPiano(EPianoParams),
    Organ(OrganParams),
    Mallets(MalletParams),
    Pluck(PluckParams),
    Choir(ChoirParams),
}

impl Instrument {
    pub fn is_drums(&self) -> bool {
        matches!(self, Instrument::Drums(_))
    }

    /// The menu entry this instrument corresponds to.
    pub fn choice(&self) -> InstrumentChoice {
        match self {
            Instrument::Synth(p) => InstrumentChoice::Synth(p.preset),
            Instrument::Drums(_) => InstrumentChoice::Drums,
            Instrument::Piano(_) => InstrumentChoice::Piano,
            Instrument::EPiano(_) => InstrumentChoice::EPiano,
            Instrument::Organ(p) => InstrumentChoice::Organ(p.preset),
            Instrument::Mallets(p) => InstrumentChoice::Mallet(p.kind),
            Instrument::Pluck(p) => InstrumentChoice::Pluck(p.kind),
            Instrument::Choir(_) => InstrumentChoice::Choir,
        }
    }

    pub fn label(&self) -> &'static str {
        self.choice().label()
    }

    /// Rough time the instrument keeps sounding after a note ends.
    pub fn tail_seconds(&self) -> f32 {
        match self {
            Instrument::Synth(p) => p.amp.release,
            Instrument::Drums(p) => 0.15 * p.decay,
            Instrument::Piano(p) => {
                if p.pedal {
                    2.0 * p.decay
                } else {
                    0.25
                }
            }
            Instrument::EPiano(_) => 0.2,
            Instrument::Organ(_) => 0.05,
            Instrument::Mallets(p) => {
                if p.damper {
                    0.2
                } else {
                    1.0 * p.decay
                }
            }
            Instrument::Pluck(p) => {
                if p.ring {
                    1.5 * p.decay
                } else {
                    0.1
                }
            }
            Instrument::Choir(p) => p.release,
        }
    }
}

/// Groups of the "add instrument" menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Drums,
    Keys,
    Plucked,
    Mallets,
    Bass,
    Leads,
    Pads,
}

impl Category {
    pub const ALL: [Category; 7] = [
        Category::Drums,
        Category::Keys,
        Category::Plucked,
        Category::Mallets,
        Category::Bass,
        Category::Leads,
        Category::Pads,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Drums => "Drums & percussion",
            Category::Keys => "Keys",
            Category::Plucked => "Plucked",
            Category::Mallets => "Mallets",
            Category::Bass => "Bass",
            Category::Leads => "Leads",
            Category::Pads => "Pads & choir",
        }
    }
}

/// Entries of the "add instrument" menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstrumentChoice {
    Synth(SynthPreset),
    Drums,
    Piano,
    EPiano,
    Organ(OrganPreset),
    Mallet(MalletKind),
    Pluck(PluckKind),
    Choir,
}

impl InstrumentChoice {
    /// Every instrument, grouped by category (in [`Category::ALL`] order).
    pub fn all() -> Vec<InstrumentChoice> {
        use InstrumentChoice as C;
        let mut v = vec![C::Drums, C::Piano, C::EPiano];
        v.extend(OrganPreset::ALL.map(C::Organ));
        v.extend(PluckKind::ALL.map(C::Pluck));
        v.extend(MalletKind::ALL.map(C::Mallet));
        v.extend(SynthPreset::ALL.map(C::Synth));
        v.push(C::Choir);
        Category::ALL
            .iter()
            .flat_map(|&cat| v.iter().copied().filter(move |c| c.category() == cat))
            .collect()
    }

    pub fn category(self) -> Category {
        use SynthPreset as S;
        match self {
            InstrumentChoice::Drums => Category::Drums,
            InstrumentChoice::Piano | InstrumentChoice::EPiano | InstrumentChoice::Organ(_) => {
                Category::Keys
            }
            InstrumentChoice::Mallet(_) => Category::Mallets,
            InstrumentChoice::Pluck(PluckKind::BassGuitar) => Category::Bass,
            InstrumentChoice::Pluck(_) => Category::Plucked,
            InstrumentChoice::Choir => Category::Pads,
            InstrumentChoice::Synth(p) => match p {
                S::Bell => Category::Keys,
                S::Pluck | S::ArpPluck => Category::Plucked,
                S::Bass | S::SubBass | S::Bass808 | S::WobbleBass => Category::Bass,
                S::Lead | S::Chiptune | S::Brass | S::Flute => Category::Leads,
                S::Pad | S::Supersaw | S::Strings => Category::Pads,
            },
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            InstrumentChoice::Synth(p) => p.label(),
            InstrumentChoice::Drums => "Drums",
            InstrumentChoice::Piano => "Piano",
            InstrumentChoice::EPiano => "Electric Piano",
            InstrumentChoice::Organ(p) => p.label(),
            InstrumentChoice::Mallet(k) => k.label(),
            InstrumentChoice::Pluck(k) => k.label(),
            InstrumentChoice::Choir => "Choir",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            InstrumentChoice::Synth(p) => p.description(),
            InstrumentChoice::Drums => "Drum kit and percussion",
            InstrumentChoice::Piano => "Acoustic-style piano",
            InstrumentChoice::EPiano => "Warm, bell-like electric piano",
            InstrumentChoice::Organ(p) => p.description(),
            InstrumentChoice::Mallet(k) => k.description(),
            InstrumentChoice::Pluck(k) => k.description(),
            InstrumentChoice::Choir => "Voices singing \"aah\" and \"ooh\"",
        }
    }

    pub fn instrument(self) -> Instrument {
        match self {
            InstrumentChoice::Synth(p) => Instrument::Synth(p.params()),
            InstrumentChoice::Drums => Instrument::Drums(DrumParams::default()),
            InstrumentChoice::Piano => Instrument::Piano(PianoParams::default()),
            InstrumentChoice::EPiano => Instrument::EPiano(EPianoParams::default()),
            InstrumentChoice::Organ(p) => Instrument::Organ(p.params()),
            InstrumentChoice::Mallet(k) => Instrument::Mallets(k.params()),
            InstrumentChoice::Pluck(k) => Instrument::Pluck(k.params()),
            InstrumentChoice::Choir => Instrument::Choir(ChoirParams::default()),
        }
    }

    /// A short phrase showing off the instrument, for previews.
    pub fn preview_notes(self) -> Vec<PreviewNote> {
        let n = |start: f32, len: f32, pitch: u8| PreviewNote {
            start,
            len,
            pitch,
            vel: 0.8,
        };
        match self.category() {
            Category::Drums => {
                // A bar of a simple beat, then a percussion fill.
                let e = 0.25;
                let mut v = Vec::new();
                for i in 0..8 {
                    v.push(n(i as f32 * e, 0.1, 4));
                }
                for t in [0.0, 1.0, 1.25] {
                    v.push(n(t, 0.1, 0));
                }
                for t in [0.5, 1.5] {
                    v.push(n(t, 0.1, 1));
                }
                for (i, p) in [10, 14, 15, 16].into_iter().enumerate() {
                    v.push(n(2.0 + i as f32 * e, 0.1, p));
                }
                v
            }
            Category::Bass => {
                let r = 36;
                vec![
                    n(0.0, 0.35, r),
                    n(0.4, 0.15, r),
                    n(0.6, 0.35, r + 7),
                    n(1.0, 0.6, r + 12),
                ]
            }
            Category::Pads => vec![
                n(0.0, 1.6, 60),
                n(0.0, 1.6, 64),
                n(0.0, 1.6, 67),
                n(0.4, 1.2, 72),
            ],
            _ => {
                let r = match self {
                    InstrumentChoice::Mallet(MalletKind::Glockenspiel) => 84,
                    InstrumentChoice::Mallet(MalletKind::Xylophone) => 72,
                    InstrumentChoice::Synth(SynthPreset::Flute) => 72,
                    _ => 60,
                };
                let mut v = vec![
                    n(0.0, 0.2, r),
                    n(0.18, 0.2, r + 4),
                    n(0.36, 0.2, r + 7),
                    n(0.54, 0.3, r + 12),
                ];
                // Chords for keys and plucked instruments, a held note for leads.
                if self.category() == Category::Leads {
                    v.push(n(0.9, 0.7, r + 7));
                } else {
                    v.extend([n(0.9, 0.9, r), n(0.9, 0.9, r + 4), n(0.9, 0.9, r + 7)]);
                }
                v
            }
        }
    }
}

/// A note of a preview phrase, timed in seconds.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PreviewNote {
    pub start: f32,
    pub len: f32,
    pub pitch: u8,
    pub vel: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_roundtrip_and_are_grouped() {
        let all = InstrumentChoice::all();
        // Drums, piano + e-piano, organs, plucked, mallets, synths, choir.
        assert_eq!(all.len(), 1 + 2 + 3 + 4 + 4 + 14 + 1);
        for c in &all {
            assert_eq!(c.instrument().choice(), *c, "{c:?}");
            assert!(!c.preview_notes().is_empty());
        }
        // Grouped: categories appear in order, each exactly once as a run.
        let cats: Vec<Category> = all.iter().map(|c| c.category()).collect();
        let mut runs = cats.clone();
        runs.dedup();
        assert_eq!(runs, Category::ALL.to_vec());
    }

    #[test]
    fn old_drum_settings_still_load() {
        // Saved before percussion existed: no `perc_levels`.
        let json = r#"{"kit":"Punchy","tune":0.0,"decay":1.0,"punch":0.5,"snappy":0.6,
            "levels":[1,1,1,1,1,1,1,1,1,0.5],"gain":0.8}"#;
        let p: DrumParams = serde_json::from_str(json).unwrap();
        assert_eq!(p.level(9), 0.5);
        assert_eq!(p.level(16), 1.0);
    }

    #[test]
    fn old_synth_settings_still_load() {
        let mut v = serde_json::to_value(SynthParams::default()).unwrap();
        for k in ["bend", "bend_time", "chorus"] {
            v.as_object_mut().unwrap().remove(k);
        }
        let p: SynthParams = serde_json::from_value(v).unwrap();
        assert_eq!(p.chorus, 0.0);
    }
}
