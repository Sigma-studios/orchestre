//! Sound effects: short one-shot sounds made of layers. Each layer is one
//! synthesized generator (noise, tone, resonance or thump) placed in time,
//! so an explosion can be a boom, a crackle of debris and a rumbling tail.
//! Every parameter is a number, and each play varies a little on its own
//! (see [`Jitter`]), so repeated sounds never sound copy-pasted.

pub mod curve;
pub mod file;
pub mod presets;

pub use curve::Curve;

use serde::{Deserialize, Serialize};

use crate::instrument::Wave;
use crate::project::{Id, TRACK_COLORS};

/// Amplitude envelope: a linear fade in, a flat hold, then an exponential
/// decay that reaches silence (-60 dB) after `decay` seconds.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    pub attack: f32,
    pub hold: f32,
    pub decay: f32,
}

impl Shape {
    pub const fn new(attack: f32, hold: f32, decay: f32) -> Self {
        Shape {
            attack,
            hold,
            decay,
        }
    }

    pub fn length(&self) -> f32 {
        self.attack + self.hold + self.decay
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterMode {
    Low,
    Band,
    High,
}

impl FilterMode {
    pub const ALL: [FilterMode; 3] = [FilterMode::Low, FilterMode::Band, FilterMode::High];

    pub fn label(self) -> &'static str {
        match self {
            FilterMode::Low => "Low-pass (rumbly)",
            FilterMode::Band => "Band-pass (focused)",
            FilterMode::High => "High-pass (hissy)",
        }
    }
}

/// Filtered noise: whooshes, hiss, gunshot blasts, rumbles, footsteps. With
/// crackle it becomes sparse grains: fire, gravel, debris, rain.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct NoiseParams {
    pub shape: Shape,
    pub filter: FilterMode,
    /// Filter cutoff at the start and at the end of the layer, in Hz. It
    /// sweeps between them over the layer's length.
    pub cutoff_start: f32,
    pub cutoff_end: f32,
    /// 0..1.
    pub resonance: f32,
    /// 0 = steady noise, 1 = only random grains.
    pub crackle: f32,
    /// Grains per second.
    pub density: f32,
    /// Random, smooth drifting of the tone (octaves) and level: gusts of
    /// wind, flickering fire.
    #[serde(default)]
    pub wobble: f32,
    /// Drifts per second.
    #[serde(default = "one")]
    pub wobble_rate: f32,
}

fn one() -> f32 {
    1.0
}

fn rasp_rate() -> f32 {
    70.0
}

/// An oscillator with pitch slides, jumps and wobble: lasers, blips, coins,
/// power-ups, alarms, retro game sounds.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToneParams {
    pub shape: Shape,
    pub wave: Wave,
    /// Hz. The pitch slides from start to end over `glide` seconds.
    pub pitch_start: f32,
    pub pitch_end: f32,
    pub glide: f32,
    /// Semitones the pitch jumps by after `jump_time` seconds (0 = none).
    pub jump: f32,
    pub jump_time: f32,
    /// Wobble depth in semitones, and its speed in Hz.
    pub vibrato: f32,
    pub vibrato_rate: f32,
    /// Frequency modulation depth (0 = pure wave), and the modulator's
    /// frequency relative to the pitch.
    pub fm: f32,
    pub fm_ratio: f32,
    /// Low-pass cutoff, Hz.
    pub cutoff: f32,
    /// Retro lo-fi: fewer bits and a lower sample rate, 0..1.
    pub crush: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Material {
    Glass,
    Metal,
    Wood,
    Bell,
}

impl Material {
    pub const ALL: [Material; 4] = [
        Material::Glass,
        Material::Metal,
        Material::Wood,
        Material::Bell,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Material::Glass => "Glass",
            Material::Metal => "Metal",
            Material::Wood => "Wood",
            Material::Bell => "Bell",
        }
    }

    /// Frequencies of the object's resonances, relative to its pitch.
    pub fn ratios(self) -> [f32; 6] {
        match self {
            // The (n,0) modes of a wine glass or bowl, n = 2..7, from
            // French's theory: f ∝ (n²−1)/√(1+1/n²) (arXiv:1106.5657).
            Material::Glass => [1.0, 2.83, 5.42, 8.77, 12.87, 17.71],
            // Modes of a free bar (Euler–Bernoulli beam). They don't depend
            // on the material: wood differs from metal by its damping.
            Material::Metal | Material::Wood => [1.0, 2.756, 5.404, 8.933, 13.344, 18.638],
            // "True harmonic" bell tuning: hum, prime, tierce, quint,
            // nominal, superquint (keltektrust.org.uk/sob04.html).
            Material::Bell => [0.5, 1.0, 1.2, 1.5, 2.0, 3.0],
        }
    }

    /// Round objects split each resonance into two close ones that beat
    /// (Jundt et al. 2006 measured ~4 Hz on a wine glass): the split in Hz.
    pub fn beating(self) -> f32 {
        match self {
            Material::Glass => 4.0,
            Material::Bell => 1.5,
            Material::Metal | Material::Wood => 0.0,
        }
    }

    /// Internal loss factor η, from engineering tables (Irvine, "Damping
    /// properties of materials"): glass 0.6–2e-3, oak ~1e-2, steel
    /// 0.2–3e-4, brass < 1e-3.
    pub fn loss_factor(self) -> f32 {
        match self {
            Material::Glass => 1e-3,
            Material::Metal => 2e-4,
            Material::Wood => 1e-2,
            // Bell bronze: no figure found; between brass and steel.
            Material::Bell => 3e-4,
        }
    }

    /// Ring time (to −60 dB) of a resonance at `pitch` Hz from the loss
    /// factor: the amplitude time constant is 1/(π·f·η). Real objects lose
    /// more to radiation and their mounting, so this is an upper bound,
    /// capped to what a game sound wants.
    pub fn ring_time(self, pitch: f32) -> f32 {
        let tau = 1.0 / (std::f32::consts::PI * pitch.max(20.0) * self.loss_factor());
        (tau * 6.9078).clamp(0.03, 4.0)
    }
}

impl Material {}

/// A struck object ringing: glass, metal, wood, bells, UI pings. Extra hits
/// scatter smaller strikes after the first, for shards, rattles and debris.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModalParams {
    pub material: Material,
    /// Hz.
    pub pitch: f32,
    /// Seconds for the lowest resonance to die out.
    pub decay: f32,
    /// How much faster the high resonances die: 0 = all ring as long,
    /// 1 = ring time falls in proportion to frequency (damping ∝ f, as in
    /// van den Doel's modal models).
    pub damping: f32,
    /// Soft mallet (0) to hard hit (1): brighter, with more click.
    pub hardness: f32,
    /// Extra strikes after the first one.
    pub hits: u8,
    /// Seconds over which the extra strikes are spread.
    pub spread: f32,
    /// Random pitch range of the extra strikes, ± semitones.
    pub scatter: f32,
    /// Irregular shapes: each strike's resonances move at random by up to
    /// ±40% (shards, links, junk), so it stops sounding like a bell, 0..1.
    #[serde(default)]
    pub randomize: f32,
    /// How many resonances ring. The first six follow the material; more
    /// add dense, irregular modes above them, as blades, plates and rungs
    /// have: metallic rather than tuned. 1..40.
    #[serde(default = "six")]
    pub modes: u8,
}

fn six() -> u8 {
    6
}

pub const MAX_MODES: u8 = 40;

pub const MAX_HITS: u8 = 16;

/// Formants (resonances of the throat and mouth) of an adult voice, in Hz,
/// at the four corners of the vowel space: `[closed, open][back, front]`.
/// Every vowel lies between: "oo" is closed and back, "ee" closed and
/// front, "ah" open and back.
pub const VOWEL_CORNERS: [[[f32; 3]; 2]; 2] = [
    [[300.0, 870.0, 2240.0], [270.0, 2290.0, 3010.0]],
    [[730.0, 1090.0, 2440.0], [660.0, 1720.0, 2410.0]],
];

/// Named vowels as (openness, frontness), for labels and presets.
pub const VOWEL_NAMES: [(&str, f32, f32); 7] = [
    ("oo", 0.0, 0.0),
    ("oh", 0.6, 0.0),
    ("ah", 1.0, 0.1),
    ("uh", 0.8, 0.25),
    ("ae", 1.0, 1.0),
    ("eh", 0.6, 1.0),
    ("ee", 0.0, 1.0),
];

/// The nearest named vowel.
pub fn vowel_label(openness: f32, frontness: f32) -> &'static str {
    VOWEL_NAMES
        .iter()
        .min_by(|a, b| {
            let d = |v: &(&str, f32, f32)| (v.1 - openness).powi(2) + (v.2 - frontness).powi(2);
            d(a).total_cmp(&d(b))
        })
        .map_or("", |v| v.0)
}

/// Formants F1..F3 of a vowel, in Hz, for an adult voice.
pub fn vowel_formants(openness: f32, frontness: f32) -> [f32; 3] {
    let (o, f) = (openness.clamp(0.0, 1.0), frontness.clamp(0.0, 1.0));
    let c = &VOWEL_CORNERS;
    std::array::from_fn(|k| {
        let closed = c[0][0][k] + (c[0][1][k] - c[0][0][k]) * f;
        let open = c[1][0][k] + (c[1][1][k] - c[1][0][k]) * f;
        closed + (open - closed) * o
    })
}

/// Whose vocal tract shapes the voice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Tract {
    /// Human vowels (Peterson & Barney).
    #[default]
    Human,
    /// A dog's long muzzle: F1 high and close to F2, formants bunched
    /// ~600–950 Hz apart (Malinois barks, Vet. Sci. 2026).
    Dog,
}

impl Tract {
    pub const ALL: [Tract; 2] = [Tract::Human, Tract::Dog];

    pub fn label(self) -> &'static str {
        match self {
            Tract::Human => "Human",
            Tract::Dog => "Dog",
        }
    }
}

/// Formants F1..F4 of a dog's tract, mouth nearly closed and wide open.
/// Open: the measured bark ranges (F1 980–1430, F2 1660–1980, F3 2280–2790,
/// F4 2890–4040 Hz); closed: by ear.
pub const DOG_FORMANTS: [[f32; 4]; 2] = [
    [600.0, 1450.0, 2250.0, 3000.0],
    [1250.0, 1850.0, 2550.0, 3450.0],
];

/// A throat and mouth (formant synthesis): grunts, yells, groans, animal
/// calls, monsters. The curves run over the layer's length.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoiceParams {
    pub shape: Shape,
    /// Hz.
    pub pitch: Curve,
    /// Mouth, 0 = closed (oo, ee) … 1 = open (ah, ae).
    pub openness: Curve,
    /// Tongue, 0 = back (oo, oh) … 1 = front (ee, eh).
    pub frontness: Curve,
    /// Voice quality, 0 = pressed and tense … 0.5 = normal … 1 = lax and
    /// breathy.
    pub quality: Curve,
    /// Level, 0..1, on top of the volume envelope: swells and surges.
    pub loudness: Curve,
    /// Size of the throat: below 1 is a child or a small animal, above 1
    /// a big creature.
    pub size: f32,
    /// Unsteady pitch and level from one vibration to the next (jitter and
    /// shimmer): rough, hoarse, 0..1.
    pub roughness: f32,
    /// Every other vibration weaker and longer: a growl an octave below,
    /// 0..1.
    pub subharmonics: f32,
    /// A second, independent pitch at `split_ratio` times the first: the
    /// rasp of screams, crows and roars, 0..1.
    pub split: f32,
    pub split_ratio: f32,
    /// Breath noise, which pulses with the voice, 0..1.
    pub breath: f32,
    /// Fast amplitude flutter in the 30–150 Hz "roughness" band that makes
    /// screams alarming (Arnal et al. 2015), 0..1, and its rate in Hz.
    #[serde(default)]
    pub rasp: f32,
    #[serde(default = "rasp_rate")]
    pub rasp_rate: f32,
    /// Shape of the rasp: 0 = a smooth flutter … 1 = sharp closures, a
    /// tongue or lips tapping shut (a rolled r, "brrr").
    #[serde(default)]
    pub trill: f32,
    /// Wobble depth in semitones, and its speed in Hz.
    pub vibrato: f32,
    pub vibrato_rate: f32,
    /// Whose throat: a person's vowels, or an animal's formant pattern.
    #[serde(default)]
    pub tract: Tract,
    /// The rest of Klatt's synthesizer: formants set by hand, hisses,
    /// nasality. All off unless set.
    #[serde(default)]
    pub klatt: KlattParams,
}

/// The parts of Klatt's (1980) cascade/parallel formant synthesizer beyond
/// the vowel knobs, under his parameter names: formants and bandwidths set
/// directly (an r is a low F3), the amounts of voicing, breath and hiss over
/// time, the parallel branch that shapes a hiss into s, sh or f, the voice
/// bar under a v or z, and the nasal pole and zero of an m or n.
///
/// Everything is off by default, and a voice that sets none of it sounds
/// exactly as it did before any of it existed.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KlattParams {
    /// F1–F5 in Hz. Unset, each follows mouth, tongue and throat.
    pub formants: [Option<Curve>; 5],
    /// B1–B5 in Hz. Unset, each follows its formant.
    pub bandwidths: [Option<Curve>; 5],
    /// AV: how much of the sound is the vocal folds, 0..1. 0 leaves only
    /// breath and hiss: a whisper, or a consonant with no voice.
    pub voicing: Curve,
    /// AH: breath through the whole tract (h), 0..1, on top of `breath`.
    pub aspiration: Curve,
    /// AF: turbulence at a narrowing of the mouth, 0..1, shaped by the
    /// parallel formants at the `parallel` levels.
    pub frication: Curve,
    /// A1–A6 then AB: how much of the frication each parallel formant
    /// passes, and how much goes straight through (bypass), 0..1. High
    /// formants and the bypass make an s; F3 and F4 a sh; the bypass
    /// alone an f.
    pub parallel: [f32; 7],
    /// B1P–B6P in Hz: the parallel formants' own bandwidths (Klatt 1988),
    /// much wider than the vowel's, since a hiss's peaks are broad.
    pub parallel_widths: [f32; 6],
    /// F6 in Hz, the parallel branch's highest formant: where an s hisses.
    pub f6: f32,
    /// AVS: a soft hum at the pitch, under a voiced hiss (v, z), 0..1.
    pub voice_bar: Curve,
    /// FNP in Hz: the nasal cavity's resonance.
    pub nasal_pole: f32,
    /// FNZ in Hz over time: its anti-resonance. Unset, the nose is shut.
    /// Near FNP it barely changes a vowel; far above it (800–1500 Hz), with
    /// the mouth closed, it is the murmur of an m or n.
    pub nasal_zero: Option<Curve>,
}

impl Default for KlattParams {
    fn default() -> Self {
        KlattParams {
            formants: [None; 5],
            bandwidths: [None; 5],
            voicing: Curve::flat(1.0),
            aspiration: Curve::flat(0.0),
            frication: Curve::flat(0.0),
            parallel: [0.0, 0.0, 0.3, 0.5, 0.6, 0.6, 0.3],
            parallel_widths: [100.0, 200.0, 350.0, 500.0, 700.0, 1000.0],
            f6: 4900.0,
            voice_bar: Curve::flat(0.0),
            nasal_pole: 250.0,
            nasal_zero: None,
        }
    }
}

impl KlattParams {
    /// Whether any hiss reaches the parallel branch.
    pub fn frication_on(&self) -> bool {
        self.frication.max() > 0.0
    }
}

/// Water, as clouds of bubbles (van den Doel 2005): each bubble rings at
/// f ≈ 3/r, dies faster the higher it is, and rises in pitch as it goes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BubbleParams {
    pub shape: Shape,
    /// Bubbles per second.
    pub rate: f32,
    /// Smallest and largest bubble radius, millimetres.
    pub size_min: f32,
    pub size_max: f32,
    /// How much more common small bubbles are (van den Doel's γ; ~5 or more
    /// sounds like rain).
    pub small: f32,
    /// Pitch rise: ~0.1 for drops in water, up to ~10 for blowing through
    /// a straw.
    pub rise: f32,
}

/// A combustion engine: cylinders firing in turn at the crank's speed,
/// through a resonant exhaust pipe. The firing rate is rpm/60 × cylinders
/// (two-stroke) or half that (four-stroke); the pipe's resonance stays put
/// while the rpm changes, as on a real engine.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EngineParams {
    pub shape: Shape,
    /// Crankshaft speed as designed.
    pub rpm: f32,
    pub cylinders: u8,
    /// Two-strokes fire every turn of the crank, four-strokes every other.
    pub two_stroke: bool,
    /// Uneven firing (V-twins, cross-plane V8s): 0 = even, 1 = very lopey.
    pub uneven: f32,
    /// Exhaust pipe length, metres: its resonance colours the note.
    pub exhaust: f32,
    /// Throttle: 0 = coasting, 1 = full load (louder, brighter, rougher).
    pub load: f32,
    /// Firing-to-firing variation, 0..1.
    pub roughness: f32,
    /// Intake hiss and valve ticks, rising with rpm, 0..1.
    pub mechanics: f32,
}

/// A train of short bursts, each ringing a resonance: creaks, ratchets,
/// ticking, engines, gunfire bursts, rattles.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PulseParams {
    pub shape: Shape,
    /// Pulses per second at the start and end; slides over `glide` seconds.
    pub rate_start: f32,
    pub rate_end: f32,
    pub glide: f32,
    /// Random timing and level of each pulse, 0..1.
    pub irregular: f32,
    /// Seconds each pulse lasts before ringing.
    pub burst: f32,
    /// Hz of the resonance the pulses ring.
    pub tone: f32,
    /// 0 = a dull knock, 1 = a long ring.
    pub resonance: f32,
    /// A wooden body ringing along (six broad resonances below `tone`, as in
    /// Farnell's creaking door): creaks, old furniture, 0..1.
    #[serde(default)]
    pub body: f32,
    /// What each pulse is made of: 0 = a clean click (stick-slip friction),
    /// 1 = a burst of noise.
    #[serde(default = "one")]
    pub grit: f32,
}

/// A low body hit whose pitch drops, like a kick drum: punches, landings,
/// door slams, the boom of an explosion.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThumpParams {
    pub shape: Shape,
    /// Hz. The pitch falls from start to end, most of the way in `drop` seconds.
    pub pitch_start: f32,
    pub pitch_end: f32,
    pub drop: f32,
    /// Sharp transient at the very start, 0..1.
    pub click: f32,
    /// Noise mixed into the body (dull thuds, punches), 0..1.
    pub noise: f32,
    /// Low-pass on the click and noise, Hz.
    pub cutoff: f32,
    /// A blast wave at the start (Friedlander shape: a sharp overpressure,
    /// then a suction): gunshots, explosions, 0..1.
    #[serde(default)]
    pub blast: f32,
}

// Voices carry five curves inline (a few hundred bytes): fine for the few
// layers of a sound, and it keeps generators `Copy`, with nothing to allocate
// on the audio thread.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Generator {
    Noise(NoiseParams),
    Tone(ToneParams),
    Modal(ModalParams),
    Thump(ThumpParams),
    Voice(VoiceParams),
    Pulses(PulseParams),
    Bubbles(BubbleParams),
    Engine(EngineParams),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratorKind {
    Noise,
    Tone,
    Modal,
    Thump,
    Voice,
    Pulses,
    Bubbles,
    Engine,
}

impl GeneratorKind {
    pub const ALL: [GeneratorKind; 8] = [
        GeneratorKind::Noise,
        GeneratorKind::Tone,
        GeneratorKind::Modal,
        GeneratorKind::Thump,
        GeneratorKind::Voice,
        GeneratorKind::Pulses,
        GeneratorKind::Bubbles,
        GeneratorKind::Engine,
    ];

    pub fn label(self) -> &'static str {
        match self {
            GeneratorKind::Noise => "Noise",
            GeneratorKind::Tone => "Tone",
            GeneratorKind::Modal => "Resonance",
            GeneratorKind::Thump => "Thump",
            GeneratorKind::Voice => "Voice",
            GeneratorKind::Pulses => "Pulses",
            GeneratorKind::Bubbles => "Bubbles",
            GeneratorKind::Engine => "Engine",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            GeneratorKind::Noise => "Whooshes, hiss, blasts, rumble, crackle",
            GeneratorKind::Tone => "Lasers, blips, coins, alarms",
            GeneratorKind::Modal => "Glass, metal, wood, bells",
            GeneratorKind::Thump => "Punches, landings, booms",
            GeneratorKind::Voice => "Grunts, yells, animals, monsters",
            GeneratorKind::Pulses => "Creaks, ratchets, engines, ticking",
            GeneratorKind::Bubbles => "Water: splashes, streams, drips, swimming",
            GeneratorKind::Engine => "Cars, motorbikes, boats, generators",
        }
    }

    /// A reasonable starting point for a new layer.
    pub fn default_generator(self) -> Generator {
        match self {
            GeneratorKind::Noise => Generator::Noise(NoiseParams {
                shape: Shape::new(0.005, 0.02, 0.3),
                filter: FilterMode::Band,
                cutoff_start: 2000.0,
                cutoff_end: 800.0,
                resonance: 0.2,
                crackle: 0.0,
                density: 200.0,
                wobble: 0.0,
                wobble_rate: 1.0,
            }),
            GeneratorKind::Tone => Generator::Tone(ToneParams {
                shape: Shape::new(0.002, 0.05, 0.2),
                wave: Wave::Square,
                pitch_start: 880.0,
                pitch_end: 440.0,
                glide: 0.2,
                jump: 0.0,
                jump_time: 0.1,
                vibrato: 0.0,
                vibrato_rate: 8.0,
                fm: 0.0,
                fm_ratio: 2.0,
                cutoff: 8000.0,
                crush: 0.0,
            }),
            GeneratorKind::Modal => Generator::Modal(ModalParams {
                material: Material::Metal,
                pitch: 600.0,
                decay: 0.8,
                damping: 0.4,
                hardness: 0.6,
                hits: 0,
                spread: 0.2,
                scatter: 5.0,
                randomize: 0.0,
                modes: 6,
            }),
            GeneratorKind::Thump => Generator::Thump(ThumpParams {
                shape: Shape::new(0.001, 0.0, 0.3),
                pitch_start: 160.0,
                pitch_end: 50.0,
                drop: 0.04,
                click: 0.5,
                noise: 0.2,
                cutoff: 3000.0,
                blast: 0.0,
            }),
            GeneratorKind::Voice => Generator::Voice(VoiceParams {
                shape: Shape::new(0.02, 0.2, 0.2),
                pitch: Curve::new(&[(0.0, 140.0), (0.3, 160.0), (1.0, 115.0)]),
                openness: Curve::new(&[(0.0, 0.8), (1.0, 0.5)]),
                frontness: Curve::flat(0.25),
                quality: Curve::flat(0.4),
                loudness: Curve::new(&[(0.0, 0.8), (0.3, 1.0), (1.0, 0.7)]),
                size: 1.0,
                roughness: 0.2,
                subharmonics: 0.0,
                split: 0.0,
                split_ratio: 1.41,
                breath: 0.1,
                rasp: 0.0,
                rasp_rate: 70.0,
                trill: 0.0,
                vibrato: 0.0,
                vibrato_rate: 5.0,
                tract: Tract::Human,
                klatt: KlattParams::default(),
            }),
            GeneratorKind::Pulses => Generator::Pulses(PulseParams {
                shape: Shape::new(0.01, 0.5, 0.2),
                rate_start: 20.0,
                rate_end: 20.0,
                glide: 0.5,
                irregular: 0.3,
                burst: 0.002,
                tone: 800.0,
                resonance: 0.5,
                body: 0.0,
                grit: 1.0,
            }),
            GeneratorKind::Bubbles => Generator::Bubbles(BubbleParams {
                shape: Shape::new(0.01, 0.4, 0.3),
                rate: 60.0,
                size_min: 1.0,
                size_max: 6.0,
                small: 2.0,
                rise: 0.3,
            }),
            GeneratorKind::Engine => Generator::Engine(EngineParams {
                shape: Shape::new(0.2, 1.0, 0.5),
                rpm: 850.0,
                cylinders: 4,
                two_stroke: false,
                uneven: 0.0,
                exhaust: 2.0,
                load: 0.3,
                roughness: 0.25,
                mechanics: 0.3,
            }),
        }
    }
}

impl Generator {
    pub fn kind(&self) -> GeneratorKind {
        match self {
            Generator::Noise(_) => GeneratorKind::Noise,
            Generator::Tone(_) => GeneratorKind::Tone,
            Generator::Modal(_) => GeneratorKind::Modal,
            Generator::Thump(_) => GeneratorKind::Thump,
            Generator::Voice(_) => GeneratorKind::Voice,
            Generator::Pulses(_) => GeneratorKind::Pulses,
            Generator::Bubbles(_) => GeneratorKind::Bubbles,
            Generator::Engine(_) => GeneratorKind::Engine,
        }
    }

    /// Seconds from the layer's start until it is silent.
    pub fn length(&self) -> f32 {
        match self {
            Generator::Noise(p) => p.shape.length(),
            Generator::Tone(p) => p.shape.length(),
            Generator::Modal(p) => {
                let spread = if p.hits > 0 { p.spread } else { 0.0 };
                spread + p.decay
            }
            Generator::Thump(p) => p.shape.length(),
            Generator::Voice(p) => p.shape.length(),
            Generator::Pulses(p) => p.shape.length(),
            // The last bubbles to start still have to ring out.
            Generator::Bubbles(p) => p.shape.length() + 0.1,
            Generator::Engine(p) => p.shape.length(),
        }
    }

    /// The volume envelope, for generators that have one.
    pub fn shape(&self) -> Option<&Shape> {
        match self {
            Generator::Noise(p) => Some(&p.shape),
            Generator::Tone(p) => Some(&p.shape),
            Generator::Modal(_) => None,
            Generator::Thump(p) => Some(&p.shape),
            Generator::Voice(p) => Some(&p.shape),
            Generator::Pulses(p) => Some(&p.shape),
            Generator::Bubbles(p) => Some(&p.shape),
            Generator::Engine(p) => Some(&p.shape),
        }
    }
}

/// How much a layer changes, at random, each time the sound plays. Every
/// value is a ± range; 0 turns that kind of variation off.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Jitter {
    /// Semitones.
    pub pitch: f32,
    /// Decibels.
    pub volume: f32,
    /// Seconds added to (or removed from) the layer's start.
    pub timing: f32,
    /// Brightness, as a fraction of an octave on filters and hardness.
    pub tone: f32,
    /// Fraction of the layer's length.
    pub length: f32,
}

impl Jitter {
    pub const NONE: Jitter = Jitter {
        pitch: 0.0,
        volume: 0.0,
        timing: 0.0,
        tone: 0.0,
        length: 0.0,
    };

    /// A little of everything: enough that repeats don't sound identical.
    pub const SUBTLE: Jitter = Jitter {
        pitch: 0.6,
        volume: 1.5,
        timing: 0.0,
        tone: 0.15,
        length: 0.1,
    };
}

impl Default for Jitter {
    fn default() -> Self {
        Jitter::SUBTLE
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: Id,
    pub name: String,
    pub color: [u8; 3],
    /// Seconds after the start of the sound.
    pub start: f32,
    /// Linear gain, 0..1.5.
    pub gain: f32,
    /// -1 (left) .. 1 (right).
    pub pan: f32,
    /// Saturation, 0..1.
    pub drive: f32,
    /// Semitones the layer's pitch rises per unit of intensity above 1 (and
    /// falls below it), live while it plays: an engine revving.
    #[serde(default)]
    pub follow: f32,
    pub mute: bool,
    pub solo: bool,
    pub jitter: Jitter,
    pub generator: Generator,
}

impl Layer {
    pub fn end(&self) -> f32 {
        self.start + self.generator.length()
    }
}

/// How a sound plays over time.
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub enum Looping {
    /// Once, then it ends.
    #[default]
    Once,
    /// Layers hold after fading in, until the sound is stopped; then they
    /// fade out. Wind, fire, engines, hums.
    Sustain,
    /// The whole sound starts again every `every` seconds, varying each
    /// time, until stopped. Heartbeats, alarms, machine guns, ticking.
    Repeat { every: f32 },
}

impl Looping {
    pub fn label(self) -> &'static str {
        match self {
            Looping::Once => "Once",
            Looping::Sustain => "Hold until stopped",
            Looping::Repeat { .. } => "Repeat until stopped",
        }
    }
}

/// What a sound's control moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlTarget {
    /// The pitch of every layer (a voice's pitch, an insect's wing beat).
    Pitch,
    /// How fast it goes: repeats and pulses (an engine's rpm, a gun's rate
    /// of fire, a heart rate).
    Rate,
    /// How hard it plays (rain's heaviness, a fire's size).
    Intensity,
    /// How far a voice's pitch rises and falls around its middle (a howl's
    /// swoop).
    Swing,
}

impl ControlTarget {
    pub const ALL: [ControlTarget; 4] = [
        ControlTarget::Pitch,
        ControlTarget::Rate,
        ControlTarget::Intensity,
        ControlTarget::Swing,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ControlTarget::Pitch => "Pitch",
            ControlTarget::Rate => "Speed",
            ControlTarget::Intensity => "Intensity",
            ControlTarget::Swing => "Pitch range",
        }
    }
}

/// A setting a game can change on this sound, in its own words and units:
/// "rpm", "rate of fire", "pitch". `base` is the value that plays the sound
/// as designed; other values scale pitch, speed or intensity in proportion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Control {
    pub name: String,
    pub target: ControlTarget,
    pub unit: String,
    pub base: f32,
    pub min: f32,
    pub max: f32,
}

impl Control {
    pub fn new(
        name: &str,
        target: ControlTarget,
        unit: &str,
        base: f32,
        min: f32,
        max: f32,
    ) -> Self {
        Control {
            name: name.into(),
            target,
            unit: unit.into(),
            base,
            min,
            max,
        }
    }

    /// Apply a value of this control to play options.
    pub fn apply(&self, value: f32, opts: &mut PlayOpts) {
        let ratio = value.clamp(self.min, self.max) / self.base.max(1e-6);
        match self.target {
            ControlTarget::Pitch => opts.pitch += 12.0 * ratio.max(1e-6).log2(),
            ControlTarget::Rate => opts.rate *= ratio.max(0.0),
            ControlTarget::Intensity => opts.intensity *= ratio.max(0.0),
            ControlTarget::Swing => opts.swing *= ratio.max(0.0),
        }
    }
}

/// A sound effect: one `.orsfx` file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sound {
    pub name: String,
    pub layers: Vec<Layer>,
    pub next_id: Id,
    /// Linear gain, 0..1.5.
    pub volume: f32,
    /// Scales every layer's jitter: 0 plays the same sound every time.
    pub variation: f32,
    /// Reverb amount, 0..1.
    pub space: f32,
    /// Most copies that play at once in a game; the oldest one stops first.
    pub max_voices: u8,
    #[serde(default)]
    pub looping: Looping,
    /// Named settings a game can change, even while the sound plays.
    #[serde(default)]
    pub controls: Vec<Control>,
}

impl Default for Sound {
    fn default() -> Self {
        Sound {
            name: "New sound".into(),
            layers: Vec::new(),
            next_id: 1,
            volume: 0.8,
            variation: 1.0,
            space: 0.0,
            max_voices: 4,
            looping: Looping::Once,
            controls: Vec::new(),
        }
    }
}

impl Sound {
    /// Play options with named control values applied (unknown names are
    /// ignored).
    pub fn with_controls<'a>(
        &self,
        mut opts: PlayOpts,
        values: impl IntoIterator<Item = (&'a str, f32)>,
    ) -> PlayOpts {
        for (name, value) in values {
            if let Some(c) = self
                .controls
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case(name))
            {
                c.apply(value, &mut opts);
            }
        }
        opts
    }

    pub fn new_id(&mut self) -> Id {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn layer(&self, id: Id) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    pub fn layer_mut(&mut self, id: Id) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    /// Add a layer with a fresh id and color; returns its id.
    pub fn add_layer(&mut self, name: &str, generator: Generator) -> Id {
        let id = self.new_id();
        let color = TRACK_COLORS[self.layers.len() % TRACK_COLORS.len()];
        self.layers.push(Layer {
            id,
            name: name.to_string(),
            color,
            start: 0.0,
            gain: 0.8,
            pan: 0.0,
            drive: 0.0,
            follow: 0.0,
            mute: false,
            solo: false,
            jitter: Jitter::default(),
            generator,
        });
        id
    }

    /// Copy a layer (e.g. from another sound) in, with a fresh id and color.
    pub fn insert_layer(&mut self, layer: &Layer) -> Id {
        let id = self.new_id();
        let color = TRACK_COLORS[self.layers.len() % TRACK_COLORS.len()];
        self.layers.push(Layer {
            id,
            color,
            solo: false,
            ..layer.clone()
        });
        id
    }

    /// Whether a layer is heard, given mute and solo.
    pub fn audible(&self, layer: &Layer) -> bool {
        let any_solo = self.layers.iter().any(|l| l.solo);
        !layer.mute && (!any_solo || layer.solo)
    }

    /// For looping sounds: how long to let it loop when it has to end on
    /// its own (exports, previews) before stopping it. `None` for one-shots.
    pub fn loop_preview(&self) -> Option<f32> {
        match self.looping {
            Looping::Once => None,
            Looping::Sustain => {
                let held = self
                    .layers
                    .iter()
                    .filter(|l| self.audible(l))
                    .filter_map(|l| l.generator.shape().map(|s| l.start + s.attack + s.hold))
                    .fold(0.0, f32::max);
                Some(held.max(2.0))
            }
            Looping::Repeat { every } => Some((every.max(0.02) * 3.0).max(1.0)),
        }
    }

    /// Seconds until the last audible layer is silent (without reverb, and
    /// before random variation). For looping sounds, one pass.
    pub fn length(&self) -> f32 {
        self.layers
            .iter()
            .filter(|l| self.audible(l))
            .map(Layer::end)
            .fold(0.0, f32::max)
    }
}

/// Options for one play of a sound.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayOpts {
    /// Picks the random variation: the same seed always sounds the same.
    pub seed: u32,
    /// How hard the sound is played: 1 is as designed, lower is softer,
    /// darker and shorter, higher is louder, brighter and longer. 0..2.
    pub intensity: f32,
    /// Semitones added to every layer.
    pub pitch: f32,
    /// Linear gain.
    pub volume: f32,
    /// Speed of repeats and pulses: 2 plays them twice as fast.
    #[serde(default = "one")]
    pub rate: f32,
    /// How far voices' pitch curves swing around their middle: 0 = flat,
    /// 1 = as designed, 2 = twice as far.
    #[serde(default = "one")]
    pub swing: f32,
}

impl Default for PlayOpts {
    fn default() -> Self {
        PlayOpts {
            seed: 1,
            intensity: 1.0,
            pitch: 0.0,
            volume: 1.0,
            rate: 1.0,
            swing: 1.0,
        }
    }
}

impl PlayOpts {
    pub fn seeded(seed: u32) -> Self {
        PlayOpts {
            seed,
            ..PlayOpts::default()
        }
    }
}

/// Gain for a play [`intensity`](PlayOpts::intensity): quieter below 1,
/// a little louder above.
pub fn intensity_gain(intensity: f32) -> f32 {
    let i = intensity.clamp(0.0, 2.0);
    if i <= 1.0 {
        i.powf(1.3)
    } else {
        1.0 + (i - 1.0) * 0.6
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_follows_audible_layers() {
        let mut s = Sound::default();
        assert_eq!(s.length(), 0.0);
        let a = s.add_layer("a", GeneratorKind::Thump.default_generator());
        let b = s.add_layer("b", GeneratorKind::Noise.default_generator());
        s.layer_mut(b).unwrap().start = 1.0;
        let end_b = s.layer(b).unwrap().end();
        assert!((s.length() - end_b).abs() < 1e-6);
        s.layer_mut(b).unwrap().mute = true;
        assert!((s.length() - s.layer(a).unwrap().end()).abs() < 1e-6);
        s.layer_mut(b).unwrap().mute = false;
        s.layer_mut(a).unwrap().solo = true;
        assert!(!s.audible(s.layer(b).unwrap()));
        assert!((s.length() - s.layer(a).unwrap().end()).abs() < 1e-6);
    }

    #[test]
    fn controls_scale_pitch_speed_and_intensity() {
        let s = Sound {
            controls: vec![
                Control::new("RPM", ControlTarget::Rate, "rpm", 800.0, 400.0, 6000.0),
                Control::new("Pitch", ControlTarget::Pitch, "Hz", 200.0, 100.0, 400.0),
                Control::new("Size", ControlTarget::Intensity, "", 1.0, 0.0, 2.0),
            ],
            ..Sound::default()
        };
        let o = s.with_controls(
            PlayOpts::default(),
            [
                ("rpm", 1600.0),
                ("pitch", 400.0),
                ("size", 0.5),
                ("nope", 3.0),
            ],
        );
        assert!((o.rate - 2.0).abs() < 1e-5);
        assert!((o.pitch - 12.0).abs() < 1e-4);
        assert!((o.intensity - 0.5).abs() < 1e-5);
        // Values outside the range are clamped.
        let o = s.with_controls(PlayOpts::default(), [("rpm", 100000.0)]);
        assert!((o.rate - 7.5).abs() < 1e-5);
    }

    #[test]
    fn inserted_layers_get_fresh_ids() {
        let mut s = Sound::default();
        let a = s.add_layer("a", GeneratorKind::Tone.default_generator());
        let copy = s.layer(a).unwrap().clone();
        let b = s.insert_layer(&copy);
        assert_ne!(a, b);
        assert_eq!(s.layers.len(), 2);
        assert_eq!(s.layer(b).unwrap().generator, copy.generator);
    }
}
