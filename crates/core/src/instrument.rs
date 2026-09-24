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
}

impl SynthPreset {
    pub const ALL: [SynthPreset; 8] = [
        SynthPreset::Lead,
        SynthPreset::Bass,
        SynthPreset::SubBass,
        SynthPreset::Pad,
        SynthPreset::Pluck,
        SynthPreset::Bell,
        SynthPreset::Supersaw,
        SynthPreset::Chiptune,
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
}

impl DrumPiece {
    /// Index = the "pitch" used by notes on a drum track.
    pub const ALL: [DrumPiece; 10] = [
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
    pub levels: [f32; 10],
    pub gain: f32,
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

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Instrument {
    Synth(SynthParams),
    Drums(DrumParams),
    Piano(PianoParams),
}

impl Instrument {
    pub fn is_drums(&self) -> bool {
        matches!(self, Instrument::Drums(_))
    }

    pub fn label(&self) -> &'static str {
        match self {
            Instrument::Synth(p) => p.preset.label(),
            Instrument::Drums(_) => "Drums",
            Instrument::Piano(_) => "Piano",
        }
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
        }
    }
}

/// Entries of the "add instrument" menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstrumentChoice {
    Synth(SynthPreset),
    Drums,
    Piano,
}

impl InstrumentChoice {
    pub fn all() -> Vec<InstrumentChoice> {
        let mut v = vec![InstrumentChoice::Drums, InstrumentChoice::Piano];
        v.extend(SynthPreset::ALL.iter().map(|&p| InstrumentChoice::Synth(p)));
        v
    }

    pub fn label(self) -> &'static str {
        match self {
            InstrumentChoice::Synth(p) => p.label(),
            InstrumentChoice::Drums => "Drums",
            InstrumentChoice::Piano => "Piano",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            InstrumentChoice::Synth(p) => p.description(),
            InstrumentChoice::Drums => "Synthesized drum kit",
            InstrumentChoice::Piano => "Acoustic-style piano",
        }
    }

    pub fn instrument(self) -> Instrument {
        match self {
            InstrumentChoice::Synth(p) => Instrument::Synth(p.params()),
            InstrumentChoice::Drums => Instrument::Drums(DrumParams::default()),
            InstrumentChoice::Piano => Instrument::Piano(PianoParams::default()),
        }
    }
}
