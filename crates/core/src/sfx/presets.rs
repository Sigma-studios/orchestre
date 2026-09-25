//! Built-in sound effects, to use as they are while developing a game and
//! to tune or replace later. Each one is an ordinary [`Sound`].

use super::{
    BubbleParams, Control, ControlTarget, Curve, EngineParams, FilterMode, Generator, Layer,
    Looping, Material, ModalParams, NoiseParams, PulseParams, Shape, Sound, ThumpParams,
    ToneParams, Tract, VoiceParams,
};
use crate::instrument::Wave;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SfxCategory {
    Impacts,
    Weapons,
    SciFi,
    Magic,
    Explosions,
    Footsteps,
    Movement,
    Breaking,
    Doors,
    Machines,
    Nature,
    Voices,
    Creatures,
    Interface,
    Feedback,
    Pickups,
}

impl SfxCategory {
    pub const ALL: [SfxCategory; 16] = [
        SfxCategory::Impacts,
        SfxCategory::Weapons,
        SfxCategory::SciFi,
        SfxCategory::Magic,
        SfxCategory::Explosions,
        SfxCategory::Footsteps,
        SfxCategory::Movement,
        SfxCategory::Breaking,
        SfxCategory::Doors,
        SfxCategory::Machines,
        SfxCategory::Nature,
        SfxCategory::Voices,
        SfxCategory::Creatures,
        SfxCategory::Interface,
        SfxCategory::Feedback,
        SfxCategory::Pickups,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SfxCategory::Impacts => "Impacts & hits",
            SfxCategory::Weapons => "Weapons",
            SfxCategory::SciFi => "Sci-fi",
            SfxCategory::Magic => "Magic",
            SfxCategory::Movement => "Movement & cloth",
            SfxCategory::Machines => "Machines",
            SfxCategory::Nature => "Nature & weather",
            SfxCategory::Voices => "Human voices",
            SfxCategory::Creatures => "Animals & creatures",
            SfxCategory::Feedback => "Game feedback",
            SfxCategory::Explosions => "Explosions",
            SfxCategory::Footsteps => "Footsteps",
            SfxCategory::Breaking => "Breaking & debris",
            SfxCategory::Doors => "Doors",
            SfxCategory::Interface => "Interface",
            SfxCategory::Pickups => "Pickups & retro",
        }
    }
}

pub struct Preset {
    pub category: SfxCategory,
    pub name: &'static str,
    pub description: &'static str,
    build: fn(&mut Sound),
}

impl Preset {
    pub fn sound(&self) -> Sound {
        let mut s = Sound {
            name: self.name.to_string(),
            ..Sound::default()
        };
        (self.build)(&mut s);
        s
    }
}

pub fn find(name: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.name == name)
}

fn add<'a>(s: &'a mut Sound, name: &str, start: f32, gain: f32, g: Generator) -> &'a mut Layer {
    let id = s.add_layer(name, g);
    let l = s.layer_mut(id).unwrap();
    l.start = start;
    l.gain = gain;
    l
}

const fn sh(attack: f32, hold: f32, decay: f32) -> Shape {
    Shape::new(attack, hold, decay)
}

fn np(filter: FilterMode, from: f32, to: f32, resonance: f32, shape: Shape) -> NoiseParams {
    NoiseParams {
        shape,
        filter,
        cutoff_start: from,
        cutoff_end: to,
        resonance,
        crackle: 0.0,
        density: 200.0,
        wobble: 0.0,
        wobble_rate: 1.0,
    }
}

fn noise(filter: FilterMode, from: f32, to: f32, resonance: f32, shape: Shape) -> Generator {
    Generator::Noise(np(filter, from, to, resonance, shape))
}

fn bubbles(
    rate: f32,
    size_min: f32,
    size_max: f32,
    small: f32,
    rise: f32,
    shape: Shape,
) -> Generator {
    Generator::Bubbles(BubbleParams {
        shape,
        rate,
        size_min,
        size_max,
        small,
        rise,
    })
}

/// Noise whose tone and level drift at random: wind, fire, water.
fn drifting(
    filter: FilterMode,
    at: f32,
    resonance: f32,
    wobble: f32,
    rate: f32,
    shape: Shape,
) -> Generator {
    Generator::Noise(NoiseParams {
        wobble,
        wobble_rate: rate,
        ..np(filter, at, at, resonance, shape)
    })
}

/// A voice with a normal quality and steady contours; set the rest with
/// struct update syntax.
fn voice(shape: Shape) -> VoiceParams {
    VoiceParams {
        shape,
        pitch: flat(150.0),
        openness: flat(0.6),
        frontness: flat(0.3),
        quality: flat(0.45),
        loudness: flat(1.0),
        size: 1.0,
        roughness: 0.1,
        subharmonics: 0.0,
        split: 0.0,
        split_ratio: 1.41,
        breath: 0.08,
        rasp: 0.0,
        rasp_rate: 70.0,
        vibrato: 0.0,
        vibrato_rate: 5.0,
        tract: Tract::Human,
    }
}

/// A curve through (time 0..1, value) points.
fn c(points: &[(f32, f32)]) -> Curve {
    Curve::new(points)
}

fn flat(v: f32) -> Curve {
    Curve::flat(v)
}

fn pulses(rate: f32, tone: f32, resonance: f32, burst: f32, shape: Shape) -> PulseParams {
    PulseParams {
        shape,
        rate_start: rate,
        rate_end: rate,
        glide: shape.length(),
        irregular: 0.3,
        burst,
        tone,
        resonance,
        body: 0.0,
        grit: 1.0,
    }
}

fn crackle(
    filter: FilterMode,
    from: f32,
    to: f32,
    density: f32,
    amount: f32,
    shape: Shape,
) -> Generator {
    Generator::Noise(NoiseParams {
        shape,
        filter,
        cutoff_start: from,
        cutoff_end: to,
        resonance: 0.2,
        crackle: amount,
        density,
        wobble: 0.0,
        wobble_rate: 1.0,
    })
}

fn tone(wave: Wave, from: f32, to: f32, glide: f32, shape: Shape) -> ToneParams {
    ToneParams {
        shape,
        wave,
        pitch_start: from,
        pitch_end: to,
        glide,
        jump: 0.0,
        jump_time: 0.1,
        vibrato: 0.0,
        vibrato_rate: 8.0,
        fm: 0.0,
        fm_ratio: 2.0,
        cutoff: 12000.0,
        crush: 0.0,
    }
}

fn modal(material: Material, pitch: f32, decay: f32, damping: f32, hardness: f32) -> ModalParams {
    ModalParams {
        material,
        pitch,
        decay,
        damping,
        hardness,
        hits: 0,
        spread: 0.2,
        scatter: 5.0,
        randomize: 0.0,
        modes: 6,
    }
}

fn thump_p(
    from: f32,
    to: f32,
    drop: f32,
    click: f32,
    noise: f32,
    cutoff: f32,
    shape: Shape,
) -> ThumpParams {
    ThumpParams {
        shape,
        pitch_start: from,
        pitch_end: to,
        drop,
        click,
        noise,
        cutoff,
        blast: 0.0,
    }
}

fn thump(
    from: f32,
    to: f32,
    drop: f32,
    click: f32,
    noise: f32,
    cutoff: f32,
    shape: Shape,
) -> Generator {
    Generator::Thump(thump_p(from, to, drop, click, noise, cutoff, shape))
}

use FilterMode::{Band, High, Low};
use SfxCategory::*;

pub const PRESETS: &[Preset] = &[
    // Impacts & hits
    Preset {
        category: Impacts,
        name: "Punch",
        description: "A solid hit on a body",
        build: |s| {
            add(
                s,
                "Body",
                0.0,
                1.0,
                thump(180.0, 60.0, 0.03, 0.6, 0.5, 2500.0, sh(0.001, 0.0, 0.18)),
            )
            .drive = 0.3;
            add(
                s,
                "Slap",
                0.0,
                0.4,
                noise(Band, 1500.0, 600.0, 0.3, sh(0.001, 0.005, 0.06)),
            );
            s.max_voices = 6;
        },
    },
    Preset {
        category: Impacts,
        name: "Heavy landing",
        description: "Something heavy hits the ground",
        build: |s| {
            add(
                s,
                "Thud",
                0.0,
                1.0,
                thump(110.0, 40.0, 0.06, 0.3, 0.6, 900.0, sh(0.002, 0.01, 0.35)),
            )
            .drive = 0.2;
            add(
                s,
                "Dust",
                0.0,
                0.5,
                noise(Low, 800.0, 200.0, 0.1, sh(0.002, 0.02, 0.25)),
            );
        },
    },
    Preset {
        category: Impacts,
        name: "Wood knock",
        description: "A knuckle on a wooden surface",
        build: |s| {
            add(
                s,
                "Wood",
                0.0,
                0.9,
                Generator::Modal(modal(Material::Wood, 420.0, 0.18, 0.7, 0.7)),
            )
            .jitter
            .pitch = 1.0;
            add(
                s,
                "Knock",
                0.0,
                0.4,
                thump(200.0, 120.0, 0.01, 0.2, 0.1, 2000.0, sh(0.001, 0.0, 0.06)),
            );
        },
    },
    Preset {
        category: Impacts,
        name: "Metal clang",
        description: "A hard hit on a metal bar or pipe",
        build: |s| {
            add(
                s,
                "Ring",
                0.0,
                0.9,
                Generator::Modal(modal(Material::Metal, 520.0, 1.6, 0.35, 0.85)),
            );
            add(
                s,
                "Click",
                0.0,
                0.3,
                noise(High, 6000.0, 3000.0, 0.1, sh(0.001, 0.0, 0.05)),
            );
            s.space = 0.2;
        },
    },
    Preset {
        category: Impacts,
        name: "Box drop",
        description: "A wooden box dropped on the floor",
        build: |s| {
            add(
                s,
                "Thud",
                0.0,
                0.9,
                thump(140.0, 70.0, 0.02, 0.4, 0.4, 1500.0, sh(0.001, 0.0, 0.2)),
            );
            add(
                s,
                "Wood",
                0.0,
                0.6,
                Generator::Modal(modal(Material::Wood, 180.0, 0.25, 0.8, 0.4)),
            );
        },
    },
    // Weapons
    Preset {
        category: Weapons,
        name: "Pistol shot",
        description: "A handgun firing",
        build: |s| {
            // Three events (Hacıhabiboğlu): the action, the muzzle blast
            // (a Friedlander wave) and its tail, plus a low boom for weight.
            add(
                s,
                "Action",
                0.0,
                0.3,
                noise(High, 4000.0, 4000.0, 0.2, sh(0.0005, 0.0, 0.01)),
            );
            add(
                s,
                "Blast",
                0.0,
                0.9,
                Generator::Thump(ThumpParams {
                    blast: 0.8,
                    ..thump_p(220.0, 60.0, 0.015, 1.0, 0.3, 5000.0, sh(0.0005, 0.0, 0.12))
                }),
            )
            .drive = 0.5;
            add(
                s,
                "Boom",
                0.0,
                1.0,
                thump(70.0, 35.0, 0.06, 0.0, 0.6, 350.0, sh(0.001, 0.04, 0.4)),
            )
            .drive = 0.3;
            add(
                s,
                "Crack",
                0.0,
                0.9,
                noise(Low, 7000.0, 1200.0, 0.1, sh(0.0005, 0.005, 0.18)),
            );
            add(
                s,
                "Tail",
                0.01,
                0.35,
                noise(Low, 1500.0, 300.0, 0.0, sh(0.005, 0.02, 0.5)),
            );
            s.space = 0.25;
            s.max_voices = 6;
        },
    },
    Preset {
        category: Weapons,
        name: "Shotgun",
        description: "A big, wide blast",
        build: |s| {
            add(
                s,
                "Action",
                0.0,
                0.3,
                noise(High, 3500.0, 3500.0, 0.2, sh(0.0005, 0.0, 0.012)),
            );
            add(
                s,
                "Blast",
                0.0,
                1.0,
                Generator::Thump(ThumpParams {
                    blast: 1.0,
                    ..thump_p(150.0, 45.0, 0.03, 1.0, 0.6, 3000.0, sh(0.0005, 0.01, 0.25))
                }),
            )
            .drive = 0.7;
            add(
                s,
                "Boom",
                0.0,
                1.0,
                thump(70.0, 35.0, 0.06, 0.0, 0.6, 350.0, sh(0.001, 0.04, 0.45)),
            )
            .drive = 0.4;
            add(
                s,
                "Crack",
                0.0,
                1.0,
                noise(Low, 5000.0, 600.0, 0.1, sh(0.0005, 0.02, 0.35)),
            );
            add(
                s,
                "Rumble",
                0.02,
                0.5,
                noise(Low, 900.0, 150.0, 0.0, sh(0.01, 0.05, 0.9)),
            );
            s.space = 0.3;
        },
    },
    Preset {
        category: Weapons,
        name: "Sword swing",
        description: "A blade cutting through the air",
        build: |s| {
            // A narrow, whistling band rather than broad noise.
            let l = add(
                s,
                "Whoosh",
                0.0,
                0.8,
                noise(Band, 500.0, 1600.0, 0.85, sh(0.12, 0.0, 0.18)),
            );
            l.jitter.pitch = 2.0;
            add(
                s,
                "Edge",
                0.04,
                0.15,
                noise(Band, 1500.0, 3500.0, 0.9, sh(0.08, 0.0, 0.1)),
            );
            s.max_voices = 6;
        },
    },
    Preset {
        category: Weapons,
        name: "Sword clash",
        description: "Two blades hitting",
        build: |s| {
            // A blade has dozens of crowded, irregular modes: a noisy metallic
            // "shing", not a tuned ring.
            add(
                s,
                "Hit",
                0.0,
                0.5,
                noise(High, 8000.0, 5000.0, 0.1, sh(0.0005, 0.002, 0.02)),
            );
            add(
                s,
                "Blade",
                0.0,
                0.7,
                Generator::Modal(ModalParams {
                    modes: 32,
                    randomize: 0.8,
                    ..modal(Material::Metal, 1500.0, 0.5, 0.8, 1.0)
                }),
            );
            add(
                s,
                "Blade 2",
                0.0,
                0.4,
                Generator::Modal(ModalParams {
                    modes: 24,
                    randomize: 0.8,
                    ..modal(Material::Metal, 2300.0, 0.35, 0.8, 1.0)
                }),
            )
            .pan = 0.3;
            add(
                s,
                "Shing",
                0.0,
                0.4,
                noise(Band, 6500.0, 6000.0, 0.6, sh(0.001, 0.03, 0.4)),
            );
            s.space = 0.2;
        },
    },
    Preset {
        category: SciFi,
        name: "Laser",
        description: "The classic pew",
        build: |s| {
            let l = add(
                s,
                "Pew",
                0.0,
                0.7,
                Generator::Tone(ToneParams {
                    fm: 0.8,
                    fm_ratio: 1.5,
                    cutoff: 7000.0,
                    ..tone(Wave::Square, 1400.0, 180.0, 0.2, sh(0.001, 0.03, 0.2))
                }),
            );
            l.jitter.pitch = 1.5;
            add(
                s,
                "Shine",
                0.0,
                0.3,
                Generator::Tone(tone(Wave::Sine, 2800.0, 400.0, 0.12, sh(0.001, 0.0, 0.12))),
            );
            s.max_voices = 8;
        },
    },
    Preset {
        category: SciFi,
        name: "Electric zap",
        description: "A crackling spark of electricity",
        build: |s| {
            // Arcs: spiky, snapping bursts over a sizzle, not a steady buzz.
            add(
                s,
                "Arc",
                0.0,
                0.7,
                Generator::Noise(NoiseParams {
                    crackle: 1.0,
                    density: 3000.0,
                    wobble: 1.0,
                    wobble_rate: 30.0,
                    ..np(High, 3000.0, 3000.0, 0.2, sh(0.001, 0.25, 0.1))
                }),
            )
            .drive = 0.5;
            add(
                s,
                "Snaps",
                0.0,
                0.5,
                Generator::Pulses(PulseParams {
                    irregular: 1.0,
                    ..pulses(120.0, 2500.0, 0.2, 0.0005, sh(0.001, 0.25, 0.1))
                }),
            );
            add(
                s,
                "Hum",
                0.0,
                0.12,
                Generator::Tone(ToneParams {
                    cutoff: 2000.0,
                    ..tone(Wave::Saw, 120.0, 120.0, 0.1, sh(0.001, 0.25, 0.1))
                }),
            );
        },
    },
    Preset {
        category: SciFi,
        name: "Teleport",
        description: "Shimmering, rising, gone",
        build: |s| {
            add(
                s,
                "Rise",
                0.0,
                0.35,
                Generator::Tone(ToneParams {
                    vibrato: 0.8,
                    vibrato_rate: 14.0,
                    ..tone(Wave::Sine, 300.0, 2400.0, 0.6, sh(0.05, 0.3, 0.4))
                }),
            );
            add(
                s,
                "Sparkle",
                0.0,
                0.25,
                Generator::Tone(tone(Wave::Triangle, 600.0, 4800.0, 0.6, sh(0.05, 0.3, 0.4))),
            )
            .pan = 0.3;
            add(
                s,
                "Air",
                0.0,
                0.2,
                noise(Band, 1000.0, 6000.0, 0.5, sh(0.1, 0.3, 0.4)),
            );
            s.space = 0.5;
        },
    },
    Preset {
        category: SciFi,
        name: "Power down",
        description: "A machine winding down",
        build: |s| {
            add(
                s,
                "Whine",
                0.0,
                0.6,
                Generator::Tone(ToneParams {
                    vibrato: 0.3,
                    vibrato_rate: 6.0,
                    cutoff: 3000.0,
                    ..tone(Wave::Saw, 800.0, 60.0, 0.9, sh(0.001, 0.6, 0.4))
                }),
            );
            s.space = 0.2;
        },
    },
    // Explosions
    Preset {
        category: Explosions,
        name: "Explosion",
        description: "A big blast with flying debris",
        build: |s| {
            // A sharp crack and blast wave first, then the boom and debris.
            add(
                s,
                "Crack",
                0.0,
                0.8,
                noise(High, 9000.0, 3000.0, 0.1, sh(0.0003, 0.005, 0.06)),
            );
            add(
                s,
                "Boom",
                0.0,
                1.0,
                Generator::Thump(ThumpParams {
                    blast: 0.8,
                    ..thump_p(90.0, 30.0, 0.1, 0.8, 0.7, 1200.0, sh(0.001, 0.05, 0.9))
                }),
            )
            .drive = 0.6;
            add(
                s,
                "Blast",
                0.0,
                0.9,
                noise(Low, 3000.0, 150.0, 0.1, sh(0.001, 0.1, 1.6)),
            );
            add(
                s,
                "Debris",
                0.05,
                0.5,
                crackle(Band, 2500.0, 900.0, 60.0, 0.9, sh(0.02, 0.3, 1.0)),
            );
            s.space = 0.35;
            s.max_voices = 3;
        },
    },
    Preset {
        category: Explosions,
        name: "Small blast",
        description: "A grenade or a barrel",
        build: |s| {
            add(
                s,
                "Boom",
                0.0,
                1.0,
                thump(130.0, 40.0, 0.05, 0.9, 0.6, 1800.0, sh(0.001, 0.02, 0.45)),
            )
            .drive = 0.5;
            add(
                s,
                "Blast",
                0.0,
                0.8,
                noise(Low, 5000.0, 300.0, 0.1, sh(0.001, 0.03, 0.7)),
            );
            add(
                s,
                "Debris",
                0.03,
                0.4,
                crackle(Band, 3000.0, 1200.0, 80.0, 0.9, sh(0.01, 0.15, 0.5)),
            );
            s.space = 0.3;
        },
    },
    Preset {
        category: Explosions,
        name: "Distant boom",
        description: "Far away thunder or artillery",
        build: |s| {
            add(
                s,
                "Boom",
                0.0,
                0.4,
                thump(60.0, 28.0, 0.15, 0.0, 0.5, 400.0, sh(0.03, 0.1, 1.8)),
            );
            add(
                s,
                "Rumble",
                0.0,
                0.35,
                noise(Low, 400.0, 80.0, 0.0, sh(0.05, 0.2, 2.5)),
            );
            s.space = 0.6;
            s.max_voices = 2;
        },
    },
    // Footsteps
    Preset {
        category: Footsteps,
        name: "Step on a hard floor",
        description: "Heel and toe on wood or tiles",
        build: |s| {
            // Heel then toe: soft, weighty thuds with a light tap, not an impact.
            add(
                s,
                "Heel",
                0.0,
                0.8,
                thump(110.0, 80.0, 0.01, 0.1, 0.7, 900.0, sh(0.002, 0.0, 0.06)),
            );
            add(
                s,
                "Heel tap",
                0.0,
                0.35,
                noise(Band, 1500.0, 1500.0, 0.2, sh(0.001, 0.003, 0.03)),
            );
            let toe = add(
                s,
                "Toe",
                0.06,
                0.5,
                thump(130.0, 100.0, 0.01, 0.1, 0.8, 1200.0, sh(0.002, 0.0, 0.04)),
            );
            toe.jitter.timing = 0.015;
            toe.jitter.volume = 3.0;
            add(
                s,
                "Scuff",
                0.06,
                0.2,
                noise(Band, 2500.0, 1500.0, 0.1, sh(0.005, 0.01, 0.04)),
            )
            .jitter
            .timing = 0.015;
            s.max_voices = 6;
        },
    },
    Preset {
        category: Footsteps,
        name: "Step on grass",
        description: "A soft, rustling step",
        build: |s| {
            // A soft, smooth swish of blades and a muffled thud.
            add(
                s,
                "Swish",
                0.0,
                0.5,
                drifting(Band, 3500.0, 0.1, 0.3, 10.0, sh(0.01, 0.05, 0.12)),
            )
            .jitter
            .tone = 0.3;
            add(
                s,
                "Thud",
                0.0,
                0.6,
                thump(80.0, 60.0, 0.01, 0.0, 0.9, 500.0, sh(0.003, 0.01, 0.1)),
            );
            add(
                s,
                "Toe",
                0.07,
                0.3,
                drifting(Band, 3000.0, 0.1, 0.3, 10.0, sh(0.01, 0.03, 0.08)),
            )
            .jitter
            .timing = 0.015;
            s.max_voices = 6;
        },
    },
    Preset {
        category: Footsteps,
        name: "Step on gravel",
        description: "Crunchy stones underfoot",
        build: |s| {
            // Two short crunches (heel, toe) under the foot's weight.
            add(
                s,
                "Heel crunch",
                0.0,
                0.7,
                crackle(Band, 2500.0, 1800.0, 250.0, 1.0, sh(0.002, 0.03, 0.07)),
            )
            .jitter
            .tone = 0.3;
            add(
                s,
                "Thud",
                0.0,
                0.6,
                thump(90.0, 60.0, 0.01, 0.0, 0.8, 600.0, sh(0.002, 0.0, 0.08)),
            );
            add(
                s,
                "Toe crunch",
                0.08,
                0.5,
                crackle(Band, 2800.0, 2000.0, 250.0, 1.0, sh(0.002, 0.02, 0.05)),
            )
            .jitter
            .timing = 0.015;
            s.max_voices = 6;
        },
    },
    Preset {
        category: Breaking,
        name: "Glass break",
        description: "A window or bottle shattering",
        build: |s| {
            // A bright, noisy crash with many tiny, fast-dying shards: glass
            // breaking is mostly noise, with no tones that last.
            add(
                s,
                "Crack",
                0.0,
                0.4,
                thump(300.0, 150.0, 0.01, 1.0, 0.3, 6000.0, sh(0.0005, 0.0, 0.05)),
            );
            add(
                s,
                "Crash",
                0.0,
                0.7,
                noise(High, 5000.0, 3000.0, 0.1, sh(0.0005, 0.03, 0.4)),
            );
            add(
                s,
                "Shards",
                0.0,
                0.4,
                Generator::Modal(ModalParams {
                    hits: 16,
                    spread: 0.8,
                    scatter: 12.0,
                    randomize: 1.0,
                    modes: 16,
                    ..modal(Material::Glass, 4500.0, 0.03, 1.0, 1.0)
                }),
            );
            add(
                s,
                "Tinkles",
                0.0,
                0.4,
                crackle(High, 7000.0, 6000.0, 300.0, 1.0, sh(0.001, 0.2, 0.6)),
            );
            add(
                s,
                "Sparkle",
                0.05,
                0.2,
                crackle(High, 9000.0, 9000.0, 150.0, 1.0, sh(0.001, 0.3, 0.6)),
            );
            s.space = 0.25;
        },
    },
    Preset {
        category: Breaking,
        name: "Crate smash",
        description: "Wood splintering apart",
        build: |s| {
            add(
                s,
                "Hit",
                0.0,
                0.9,
                thump(150.0, 60.0, 0.02, 0.8, 0.6, 2500.0, sh(0.001, 0.01, 0.2)),
            );
            add(
                s,
                "Planks",
                0.0,
                0.7,
                Generator::Modal(ModalParams {
                    hits: 8,
                    spread: 0.4,
                    scatter: 6.0,
                    randomize: 0.5,
                    ..modal(Material::Wood, 260.0, 0.07, 1.0, 0.9)
                }),
            );
            add(
                s,
                "Splinters",
                0.0,
                0.5,
                crackle(Band, 2000.0, 700.0, 120.0, 0.9, sh(0.001, 0.1, 0.35)),
            );
        },
    },
    Preset {
        category: Doors,
        name: "Door slam",
        description: "A heavy door shutting",
        build: |s| {
            // A heavy, noisy thud with the panel's slap and a rattle; no
            // pure resonances (they read as metal or rubber).
            add(
                s,
                "Slam",
                0.0,
                1.0,
                thump(90.0, 50.0, 0.03, 0.9, 0.8, 2500.0, sh(0.001, 0.02, 0.3)),
            )
            .drive = 0.3;
            add(
                s,
                "Panel",
                0.0,
                0.6,
                noise(Band, 350.0, 300.0, 0.3, sh(0.001, 0.02, 0.2)),
            );
            add(
                s,
                "Latch",
                0.01,
                0.3,
                noise(Band, 3000.0, 3000.0, 0.4, sh(0.0003, 0.002, 0.015)),
            );
            add(
                s,
                "Rattle",
                0.02,
                0.2,
                crackle(Band, 1500.0, 1500.0, 80.0, 1.0, sh(0.01, 0.05, 0.15)),
            );
            s.space = 0.35;
            s.max_voices = 2;
        },
    },
    Preset {
        category: Doors,
        name: "Door knock",
        description: "Three knocks on a door",
        build: |s| {
            // Knuckles on a solid door: a deep, weighty thump with a short,
            // hollow wooden ring.
            for (i, t) in [0.0, 0.18, 0.36].into_iter().enumerate() {
                let k = add(
                    s,
                    &format!("Knock {}", i + 1),
                    t,
                    1.0,
                    thump(150.0, 100.0, 0.008, 0.9, 0.4, 2000.0, sh(0.0005, 0.0, 0.09)),
                );
                k.jitter.timing = 0.02;
                add(
                    s,
                    &format!("Wood {}", i + 1),
                    t,
                    0.4,
                    Generator::Modal(ModalParams {
                        randomize: 0.2,
                        ..modal(Material::Wood, 180.0, 0.08, 0.8, 1.0)
                    }),
                );
                add(
                    s,
                    &format!("Snap {}", i + 1),
                    t,
                    0.2,
                    noise(Band, 1800.0, 1800.0, 0.3, sh(0.0003, 0.002, 0.01)),
                );
            }
            s.space = 0.25;
            s.max_voices = 1;
        },
    },
    Preset {
        category: Doors,
        name: "Latch click",
        description: "A door handle or a lock",
        build: |s| {
            add(
                s,
                "Click",
                0.0,
                0.6,
                noise(Band, 3500.0, 3500.0, 0.3, sh(0.0003, 0.002, 0.012)),
            );
            add(
                s,
                "Click body",
                0.0,
                0.4,
                thump(
                    500.0,
                    350.0,
                    0.005,
                    1.0,
                    0.3,
                    4000.0,
                    sh(0.0005, 0.0, 0.015),
                ),
            );
            add(
                s,
                "Clack",
                0.07,
                0.6,
                noise(Band, 2500.0, 2500.0, 0.3, sh(0.0003, 0.002, 0.015)),
            )
            .jitter
            .timing = 0.015;
            add(
                s,
                "Clack body",
                0.07,
                0.5,
                thump(350.0, 250.0, 0.005, 1.0, 0.3, 3500.0, sh(0.0005, 0.0, 0.02)),
            );
            add(
                s,
                "Steel",
                0.0,
                0.12,
                Generator::Modal(ModalParams {
                    randomize: 0.5,
                    ..modal(Material::Metal, 4200.0, 0.04, 1.0, 1.0)
                }),
            );
        },
    },
    Preset {
        category: Interface,
        name: "Click",
        description: "A button press",
        build: |s| {
            add(
                s,
                "Tick",
                0.0,
                0.5,
                noise(Band, 3000.0, 3000.0, 0.3, sh(0.0005, 0.002, 0.015)),
            );
            add(
                s,
                "Tone",
                0.0,
                0.5,
                Generator::Tone(tone(
                    Wave::Sine,
                    1800.0,
                    1200.0,
                    0.01,
                    sh(0.0005, 0.005, 0.03),
                )),
            );
            s.variation = 0.3;
            s.max_voices = 8;
        },
    },
    Preset {
        category: Interface,
        name: "Blip",
        description: "Hovering over a menu item",
        build: |s| {
            add(
                s,
                "Blip",
                0.0,
                0.6,
                Generator::Tone(tone(
                    Wave::Sine,
                    1200.0,
                    1200.0,
                    0.01,
                    sh(0.002, 0.02, 0.08),
                )),
            );
            s.variation = 0.2;
            s.max_voices = 8;
        },
    },
    Preset {
        category: Interface,
        name: "Confirm",
        description: "Two rising notes: OK!",
        build: |s| {
            add(
                s,
                "Notes",
                0.0,
                0.5,
                Generator::Tone(ToneParams {
                    jump: 7.0,
                    jump_time: 0.08,
                    cutoff: 4000.0,
                    ..tone(Wave::Square, 660.0, 660.0, 0.01, sh(0.002, 0.14, 0.12))
                }),
            );
            add(
                s,
                "Shine",
                0.0,
                0.25,
                Generator::Tone(ToneParams {
                    jump: 7.0,
                    jump_time: 0.08,
                    ..tone(Wave::Sine, 1320.0, 1320.0, 0.01, sh(0.002, 0.14, 0.12))
                }),
            );
            s.variation = 0.0;
        },
    },
    Preset {
        category: Interface,
        name: "Error",
        description: "Two low buzzes: not allowed",
        build: |s| {
            for (i, t) in [0.0, 0.14].into_iter().enumerate() {
                add(
                    s,
                    &format!("Buzz {}", i + 1),
                    t,
                    0.5,
                    Generator::Tone(ToneParams {
                        cutoff: 2500.0,
                        ..tone(Wave::Square, 220.0, 200.0, 0.1, sh(0.002, 0.09, 0.03))
                    }),
                );
            }
            s.variation = 0.0;
        },
    },
    Preset {
        category: Interface,
        name: "Menu whoosh",
        description: "A panel sliding open",
        build: |s| {
            add(
                s,
                "Air",
                0.0,
                0.6,
                noise(Band, 800.0, 3000.0, 0.5, sh(0.03, 0.0, 0.12)),
            );
            add(
                s,
                "Rise",
                0.0,
                0.3,
                Generator::Tone(tone(Wave::Sine, 400.0, 900.0, 0.1, sh(0.01, 0.02, 0.1))),
            );
            s.variation = 0.4;
        },
    },
    // Pickups & retro
    Preset {
        category: Pickups,
        name: "Coin",
        description: "Collecting a coin",
        build: |s| {
            add(
                s,
                "Coin",
                0.0,
                0.5,
                Generator::Tone(ToneParams {
                    jump: 5.0,
                    jump_time: 0.07,
                    cutoff: 9000.0,
                    ..tone(Wave::Square, 988.0, 988.0, 0.01, sh(0.001, 0.08, 0.35))
                }),
            );
            s.variation = 0.2;
            s.max_voices = 6;
        },
    },
    Preset {
        category: Pickups,
        name: "Power-up",
        description: "A rising, trilling jingle",
        build: |s| {
            add(
                s,
                "Rise",
                0.0,
                0.5,
                Generator::Tone(ToneParams {
                    vibrato: 1.5,
                    vibrato_rate: 16.0,
                    cutoff: 6000.0,
                    ..tone(Wave::Square, 260.0, 1040.0, 0.5, sh(0.001, 0.4, 0.2))
                }),
            );
            add(
                s,
                "Octave",
                0.0,
                0.2,
                Generator::Tone(ToneParams {
                    vibrato: 1.5,
                    vibrato_rate: 16.0,
                    ..tone(Wave::Triangle, 520.0, 2080.0, 0.5, sh(0.001, 0.4, 0.2))
                }),
            );
            s.variation = 0.2;
        },
    },
    Preset {
        category: Pickups,
        name: "Jump",
        description: "A retro platformer jump",
        build: |s| {
            add(
                s,
                "Jump",
                0.0,
                0.5,
                Generator::Tone(ToneParams {
                    cutoff: 5000.0,
                    crush: 0.3,
                    ..tone(Wave::Square, 250.0, 600.0, 0.12, sh(0.001, 0.05, 0.12))
                }),
            );
            s.variation = 0.4;
            s.max_voices = 4;
        },
    },
    Preset {
        category: Pickups,
        name: "Hurt",
        description: "The player takes damage",
        build: |s| {
            add(
                s,
                "Ouch",
                0.0,
                0.6,
                Generator::Tone(ToneParams {
                    fm: 2.0,
                    fm_ratio: 1.41,
                    crush: 0.4,
                    ..tone(Wave::Saw, 500.0, 150.0, 0.18, sh(0.001, 0.06, 0.15))
                }),
            );
            add(
                s,
                "Hit",
                0.0,
                0.5,
                noise(Band, 1500.0, 500.0, 0.2, sh(0.001, 0.01, 0.08)),
            );
        },
    },
    // Weapons, continued
    Preset {
        category: Weapons,
        name: "Reload",
        description: "Magazine out, magazine in, slide",
        build: |s| {
            // Steel parts: a hard tick, then many crowded, irregular modes
            // high up that die in ~50 ms (a small part rings like a plate,
            // not a bell), with a little mass behind.
            let click = |s: &mut Sound, name: &str, at: f32, f: f32, body: f32, gain: f32| {
                add(
                    s,
                    &format!("{name} tick"),
                    at,
                    gain * 0.5,
                    noise(High, 8000.0, 8000.0, 0.1, sh(0.0002, 0.0, 0.004)),
                );
                add(
                    s,
                    &format!("{name} steel"),
                    at,
                    gain,
                    Generator::Modal(ModalParams {
                        modes: 24,
                        randomize: 0.8,
                        ..modal(Material::Metal, f, 0.05, 0.5, 1.0)
                    }),
                );
                add(
                    s,
                    &format!("{name} body"),
                    at,
                    gain * 0.5,
                    thump(
                        body,
                        body * 0.7,
                        0.006,
                        0.6,
                        0.3,
                        3000.0,
                        sh(0.0005, 0.0, 0.03),
                    ),
                );
            };
            click(s, "Mag out", 0.0, 2800.0, 260.0, 0.6);
            click(s, "Mag in", 0.35, 2200.0, 170.0, 1.0);
            add(
                s,
                "Slide scrape",
                0.52,
                0.25,
                noise(Band, 3000.0, 2500.0, 0.3, sh(0.005, 0.03, 0.03)),
            );
            click(s, "Slide back", 0.55, 3300.0, 300.0, 0.6);
            click(s, "Slide forward", 0.66, 2600.0, 250.0, 1.0);
            s.max_voices = 1;
        },
    },
    Preset {
        category: Weapons,
        name: "Empty click",
        description: "Pulling the trigger with no ammo",
        build: |s| {
            // The trigger, then the hammer falling on nothing.
            add(
                s,
                "Trigger",
                0.0,
                0.5,
                noise(Band, 3500.0, 3500.0, 0.5, sh(0.0003, 0.001, 0.008)),
            );
            add(
                s,
                "Trigger body",
                0.0,
                0.4,
                thump(
                    400.0,
                    300.0,
                    0.005,
                    0.8,
                    0.2,
                    4000.0,
                    sh(0.0005, 0.0, 0.015),
                ),
            );
            add(
                s,
                "Hammer",
                0.02,
                0.8,
                thump(300.0, 220.0, 0.005, 1.0, 0.5, 4000.0, sh(0.0005, 0.0, 0.02)),
            );
            add(
                s,
                "Hammer click",
                0.02,
                0.4,
                noise(Band, 2500.0, 2500.0, 0.4, sh(0.0003, 0.002, 0.01)),
            );
            add(
                s,
                "Steel",
                0.02,
                0.15,
                Generator::Modal(ModalParams {
                    randomize: 0.4,
                    ..modal(Material::Metal, 3800.0, 0.03, 1.0, 1.0)
                }),
            );
            s.max_voices = 2;
        },
    },
    Preset {
        category: Weapons,
        name: "Bow shot",
        description: "A bowstring releasing an arrow",
        build: |s| {
            add(
                s,
                "String",
                0.0,
                0.6,
                Generator::Tone(ToneParams {
                    vibrato: 0.3,
                    vibrato_rate: 25.0,
                    cutoff: 2500.0,
                    ..tone(Wave::Triangle, 150.0, 140.0, 0.05, sh(0.001, 0.02, 0.25))
                }),
            );
            add(
                s,
                "Snap",
                0.0,
                0.3,
                thump(200.0, 100.0, 0.01, 0.5, 0.2, 3000.0, sh(0.001, 0.0, 0.05)),
            );
            add(
                s,
                "Flight",
                0.01,
                0.5,
                noise(Band, 900.0, 2500.0, 0.5, sh(0.03, 0.0, 0.15)),
            );
            s.max_voices = 4;
        },
    },
    Preset {
        category: Weapons,
        name: "Arrow hit",
        description: "An arrow sinking into wood",
        build: |s| {
            add(
                s,
                "Thunk",
                0.0,
                0.9,
                thump(180.0, 80.0, 0.01, 0.8, 0.3, 2500.0, sh(0.001, 0.0, 0.08)),
            );
            add(
                s,
                "Wood",
                0.0,
                0.6,
                Generator::Modal(modal(Material::Wood, 300.0, 0.15, 0.6, 0.9)),
            );
            add(
                s,
                "Quiver",
                0.0,
                0.3,
                Generator::Tone(ToneParams {
                    vibrato: 2.0,
                    vibrato_rate: 18.0,
                    ..tone(Wave::Triangle, 90.0, 85.0, 0.3, sh(0.001, 0.05, 0.25))
                }),
            );
            s.max_voices = 4;
        },
    },
    Preset {
        category: Weapons,
        name: "Bullet impact",
        description: "A bullet hitting a hard surface",
        build: |s| {
            add(
                s,
                "Crack",
                0.0,
                0.6,
                noise(High, 7000.0, 3000.0, 0.1, sh(0.0005, 0.003, 0.04)),
            );
            add(
                s,
                "Ping",
                0.0,
                0.4,
                Generator::Modal(modal(Material::Metal, 1700.0, 0.15, 0.5, 1.0)),
            );
            add(
                s,
                "Chips",
                0.0,
                0.4,
                crackle(Band, 2500.0, 1500.0, 200.0, 0.9, sh(0.001, 0.02, 0.1)),
            );
            s.max_voices = 8;
        },
    },
    Preset {
        category: Weapons,
        name: "Ricochet",
        description: "A bullet bouncing off: pyeww",
        build: |s| {
            add(
                s,
                "Whine",
                0.0,
                0.4,
                Generator::Tone(ToneParams {
                    vibrato: 0.6,
                    vibrato_rate: 30.0,
                    ..tone(Wave::Sine, 3200.0, 1300.0, 0.35, sh(0.001, 0.15, 0.25))
                }),
            );
            add(
                s,
                "Hit",
                0.0,
                0.4,
                noise(High, 6000.0, 4000.0, 0.1, sh(0.0005, 0.0, 0.03)),
            );
            s.space = 0.3;
        },
    },
    Preset {
        category: Weapons,
        name: "Machine gun",
        description: "Rapid fire until stopped",
        build: |s| {
            // Three events (Hacıhabiboğlu): the action, the muzzle blast
            // (a Friedlander wave) and its tail.
            add(
                s,
                "Action",
                0.0,
                0.35,
                Generator::Modal(modal(Material::Metal, 2500.0, 0.03, 0.5, 1.0)),
            );
            add(
                s,
                "Blast",
                0.0,
                0.9,
                Generator::Thump(ThumpParams {
                    blast: 0.8,
                    ..thump_p(200.0, 60.0, 0.012, 1.0, 0.4, 4000.0, sh(0.0005, 0.0, 0.07))
                }),
            )
            .drive = 0.5;
            add(
                s,
                "Crack",
                0.0,
                0.8,
                noise(Low, 6000.0, 1500.0, 0.1, sh(0.0005, 0.003, 0.08)),
            );
            s.looping = Looping::Repeat { every: 0.09 };
            s.space = 0.2;
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Rate of fire",
                ControlTarget::Rate,
                "rpm",
                667.0,
                300.0,
                1500.0,
            ));
        },
    },
    Preset {
        category: SciFi,
        name: "Laser beam",
        description: "A continuous beam, until stopped",
        build: |s| {
            add(
                s,
                "Beam",
                0.0,
                0.5,
                Generator::Tone(ToneParams {
                    vibrato: 0.3,
                    vibrato_rate: 30.0,
                    fm: 1.2,
                    fm_ratio: 2.01,
                    cutoff: 3000.0,
                    ..tone(Wave::Saw, 220.0, 220.0, 0.1, sh(0.05, 1.0, 0.2))
                }),
            );
            add(
                s,
                "Sizzle",
                0.0,
                0.2,
                drifting(Band, 3000.0, 0.6, 0.3, 8.0, sh(0.05, 1.0, 0.2)),
            );
            s.looping = Looping::Sustain;
            s.max_voices = 2;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                220.0,
                80.0,
                880.0,
            ));
        },
    },
    // Magic
    Preset {
        category: Magic,
        name: "Spell cast",
        description: "A swirl of energy, released",
        build: |s| {
            add(
                s,
                "Swirl",
                0.0,
                0.7,
                noise(Band, 600.0, 3000.0, 0.6, sh(0.15, 0.05, 0.4)),
            );
            add(
                s,
                "Rise",
                0.0,
                0.35,
                Generator::Tone(ToneParams {
                    vibrato: 0.5,
                    vibrato_rate: 9.0,
                    ..tone(Wave::Sine, 600.0, 1800.0, 0.3, sh(0.05, 0.1, 0.5))
                }),
            );
            add(
                s,
                "Chime",
                0.2,
                0.3,
                Generator::Modal(modal(Material::Bell, 1400.0, 1.2, 0.3, 0.5)),
            );
            s.space = 0.5;
        },
    },
    Preset {
        category: Magic,
        name: "Heal",
        description: "A warm, rising shimmer",
        build: |s| {
            add(
                s,
                "Note 1",
                0.0,
                0.35,
                Generator::Tone(tone(Wave::Sine, 523.0, 523.0, 0.01, sh(0.02, 0.25, 0.6))),
            );
            add(
                s,
                "Note 2",
                0.12,
                0.3,
                Generator::Tone(tone(Wave::Triangle, 659.0, 659.0, 0.01, sh(0.02, 0.2, 0.6))),
            );
            add(
                s,
                "Note 3",
                0.24,
                0.25,
                Generator::Tone(tone(Wave::Triangle, 784.0, 784.0, 0.01, sh(0.02, 0.2, 0.7))),
            );
            add(
                s,
                "Sparkles",
                0.1,
                0.3,
                Generator::Modal(ModalParams {
                    hits: 6,
                    spread: 0.6,
                    scatter: 5.0,
                    ..modal(Material::Glass, 2093.0, 0.8, 0.3, 0.5)
                }),
            );
            s.space = 0.5;
            s.variation = 0.3;
        },
    },
    Preset {
        category: Magic,
        name: "Shield up",
        description: "A protective barrier forming",
        build: |s| {
            add(
                s,
                "Hum",
                0.0,
                0.35,
                Generator::Tone(ToneParams {
                    fm: 0.5,
                    vibrato: 0.2,
                    vibrato_rate: 12.0,
                    cutoff: 1500.0,
                    ..tone(Wave::Saw, 150.0, 300.0, 0.3, sh(0.05, 0.3, 0.4))
                }),
            );
            add(
                s,
                "Whoosh",
                0.0,
                0.3,
                noise(Band, 400.0, 2000.0, 0.7, sh(0.1, 0.1, 0.3)),
            );
            add(
                s,
                "Ring",
                0.25,
                0.3,
                Generator::Modal(modal(Material::Metal, 800.0, 1.0, 0.4, 0.3)),
            );
            s.space = 0.4;
        },
    },
    Preset {
        category: Magic,
        name: "Charge up",
        description: "Energy building up",
        build: |s| {
            add(
                s,
                "Rise",
                0.0,
                0.35,
                Generator::Tone(ToneParams {
                    fm: 0.6,
                    vibrato: 0.4,
                    vibrato_rate: 8.0,
                    cutoff: 4000.0,
                    ..tone(Wave::Saw, 100.0, 800.0, 1.5, sh(0.2, 1.3, 0.1))
                }),
            );
            add(
                s,
                "Air",
                0.0,
                0.3,
                noise(Band, 300.0, 5000.0, 0.5, sh(0.5, 1.0, 0.1)),
            );
            s.space = 0.3;
        },
    },
    Preset {
        category: Magic,
        name: "Fireball",
        description: "A ball of fire flying past",
        build: |s| {
            add(
                s,
                "Roar",
                0.0,
                0.9,
                Generator::Noise(NoiseParams {
                    wobble: 0.6,
                    wobble_rate: 12.0,
                    ..np(Band, 300.0, 1500.0, 0.3, sh(0.02, 0.2, 0.6))
                }),
            );
            add(
                s,
                "Crackle",
                0.0,
                0.5,
                crackle(Band, 2000.0, 2000.0, 150.0, 1.0, sh(0.02, 0.3, 0.5)),
            );
            add(
                s,
                "Body",
                0.0,
                0.5,
                thump(80.0, 40.0, 0.05, 0.0, 0.8, 400.0, sh(0.02, 0.1, 0.4)),
            );
        },
    },
    Preset {
        category: Magic,
        name: "Ice spell",
        description: "Crystals forming and cracking",
        build: |s| {
            add(
                s,
                "Crystals",
                0.0,
                0.6,
                Generator::Modal(ModalParams {
                    hits: 10,
                    spread: 0.5,
                    scatter: 4.0,
                    ..modal(Material::Glass, 2500.0, 1.0, 0.3, 0.9)
                }),
            );
            add(
                s,
                "Frost",
                0.0,
                0.3,
                noise(High, 7000.0, 9000.0, 0.1, sh(0.05, 0.1, 0.4)),
            );
            add(
                s,
                "Shimmer",
                0.0,
                0.1,
                Generator::Tone(ToneParams {
                    vibrato: 0.3,
                    vibrato_rate: 20.0,
                    ..tone(Wave::Sine, 3000.0, 4000.0, 0.5, sh(0.02, 0.1, 0.4))
                }),
            );
            s.space = 0.4;
        },
    },
    Preset {
        category: Magic,
        name: "Dark magic",
        description: "Something evil stirs",
        build: |s| {
            add(
                s,
                "Drone",
                0.0,
                0.5,
                Generator::Tone(ToneParams {
                    fm: 2.5,
                    fm_ratio: 0.51,
                    vibrato: 0.5,
                    vibrato_rate: 3.0,
                    cutoff: 800.0,
                    ..tone(Wave::Saw, 60.0, 45.0, 1.0, sh(0.2, 0.4, 0.8))
                }),
            );
            add(
                s,
                "Whisper",
                0.0,
                0.5,
                Generator::Voice(VoiceParams {
                    pitch: c(&[(0.0, 70.0), (1.0, 55.0)]),
                    openness: c(&[(0.0, 0.6), (1.0, 0.1)]),
                    frontness: flat(0.2),
                    quality: flat(1.0),
                    size: 3.0,
                    roughness: 0.6,
                    breath: 0.9,
                    ..voice(sh(0.2, 0.4, 0.8))
                }),
            );
            add(
                s,
                "Rumble",
                0.0,
                0.3,
                drifting(Low, 400.0, 0.1, 0.5, 2.0, sh(0.2, 0.4, 0.8)),
            );
            s.space = 0.5;
        },
    },
    Preset {
        category: Magic,
        name: "Sparkle",
        description: "Twinkling fairy dust",
        build: |s| {
            add(
                s,
                "Twinkles",
                0.0,
                0.6,
                Generator::Modal(ModalParams {
                    hits: 12,
                    spread: 0.8,
                    scatter: 12.0,
                    ..modal(Material::Glass, 3000.0, 0.5, 0.2, 0.6)
                }),
            );
            add(
                s,
                "Dust",
                0.0,
                0.08,
                noise(High, 8000.0, 8000.0, 0.1, sh(0.1, 0.2, 0.5)),
            );
            s.space = 0.4;
        },
    },
    Preset {
        category: Magic,
        name: "Portal hum",
        description: "A magic portal, open until stopped",
        build: |s| {
            add(
                s,
                "Drone",
                0.0,
                0.4,
                Generator::Tone(ToneParams {
                    fm: 1.5,
                    fm_ratio: 1.5,
                    vibrato: 0.3,
                    vibrato_rate: 4.0,
                    cutoff: 600.0,
                    ..tone(Wave::Saw, 55.0, 55.0, 0.1, sh(0.5, 1.0, 1.0))
                }),
            );
            add(
                s,
                "Swirl",
                0.0,
                0.3,
                drifting(Band, 800.0, 0.8, 1.0, 0.7, sh(0.5, 1.0, 1.0)),
            );
            add(
                s,
                "Choir",
                0.0,
                0.1,
                Generator::Tone(ToneParams {
                    vibrato: 0.5,
                    vibrato_rate: 5.5,
                    ..tone(Wave::Sine, 440.0, 440.0, 0.1, sh(0.5, 1.0, 1.0))
                }),
            );
            s.looping = Looping::Sustain;
            s.space = 0.5;
            s.max_voices = 2;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "×",
                1.0,
                0.5,
                2.0,
            ));
        },
    },
    // Movement & cloth
    Preset {
        category: Movement,
        name: "Cloth rustle",
        description: "Clothes moving",
        build: |s| {
            // Friction noise that swells and drifts quickly: smooth, not grainy.
            add(
                s,
                "Rustle",
                0.0,
                0.6,
                drifting(Band, 2500.0, 0.15, 0.8, 25.0, sh(0.03, 0.08, 0.15)),
            );
            add(
                s,
                "Swish",
                0.0,
                0.3,
                drifting(Band, 1200.0, 0.1, 0.6, 15.0, sh(0.03, 0.08, 0.12)),
            );
            s.max_voices = 6;
        },
    },
    Preset {
        category: Movement,
        name: "Quick whoosh",
        description: "A fast movement: a dodge or a swipe",
        build: |s| {
            add(
                s,
                "Whoosh",
                0.0,
                0.9,
                noise(Band, 700.0, 2500.0, 0.6, sh(0.05, 0.0, 0.08)),
            )
            .jitter
            .pitch = 2.0;
            s.max_voices = 6;
        },
    },
    Preset {
        category: Movement,
        name: "Big whoosh",
        description: "Something large swinging past",
        build: |s| {
            add(
                s,
                "Whoosh",
                0.0,
                1.0,
                noise(Band, 250.0, 900.0, 0.5, sh(0.25, 0.0, 0.35)),
            );
            add(
                s,
                "Air",
                0.0,
                0.4,
                noise(Low, 400.0, 400.0, 0.1, sh(0.2, 0.0, 0.3)),
            );
        },
    },
    Preset {
        category: Movement,
        name: "Leap",
        description: "A character jumping (no voice)",
        build: |s| {
            add(
                s,
                "Push",
                0.0,
                0.5,
                thump(110.0, 70.0, 0.01, 0.2, 0.6, 800.0, sh(0.002, 0.0, 0.06)),
            );
            add(
                s,
                "Whoosh",
                0.01,
                0.6,
                noise(Band, 600.0, 1500.0, 0.4, sh(0.03, 0.02, 0.12)),
            );
            add(
                s,
                "Cloth",
                0.0,
                0.4,
                drifting(Band, 2500.0, 0.15, 0.8, 25.0, sh(0.02, 0.05, 0.1)),
            );
            s.max_voices = 3;
        },
    },
    Preset {
        category: Movement,
        name: "Soft landing",
        description: "Landing on the feet after a jump",
        build: |s| {
            add(
                s,
                "Thud",
                0.0,
                0.8,
                thump(90.0, 60.0, 0.01, 0.1, 0.7, 700.0, sh(0.002, 0.01, 0.15)),
            );
            add(
                s,
                "Cloth",
                0.0,
                0.4,
                drifting(Band, 2500.0, 0.15, 0.8, 25.0, sh(0.005, 0.05, 0.12)),
            );
            s.max_voices = 3;
        },
    },
    Preset {
        category: Movement,
        name: "Body fall",
        description: "A body collapsing to the ground",
        build: |s| {
            add(
                s,
                "Impact",
                0.0,
                1.0,
                thump(80.0, 40.0, 0.04, 0.3, 0.7, 800.0, sh(0.002, 0.02, 0.3)),
            );
            add(
                s,
                "Settle",
                0.12,
                0.5,
                thump(90.0, 50.0, 0.03, 0.1, 0.8, 600.0, sh(0.002, 0.01, 0.2)),
            );
            add(
                s,
                "Cloth",
                0.0,
                0.4,
                drifting(Band, 2500.0, 0.15, 0.8, 25.0, sh(0.01, 0.1, 0.2)),
            );
            s.max_voices = 2;
        },
    },
    Preset {
        category: Movement,
        name: "Ladder step",
        description: "A foot on a metal rung",
        build: |s| {
            // A hollow steel rung: many irregular modes ringing a little.
            add(
                s,
                "Rung",
                0.0,
                0.6,
                Generator::Modal(ModalParams {
                    modes: 24,
                    randomize: 0.8,
                    ..modal(Material::Metal, 500.0, 0.35, 0.6, 0.6)
                }),
            );
            add(
                s,
                "Foot",
                0.0,
                0.6,
                thump(150.0, 90.0, 0.01, 0.2, 0.4, 1200.0, sh(0.001, 0.0, 0.06)),
            );
            s.max_voices = 4;
        },
    },
    Preset {
        category: Doors,
        name: "Door creak",
        description: "Old hinges groaning",
        build: |s| {
            // Farnell's creaking door: clean stick-slip clicks, faster and
            // jittery as the force rises, each louder the longer it stuck,
            // through the wood's broad resonances and the panel's delays.
            add(
                s,
                "Creak",
                0.0,
                1.0,
                Generator::Pulses(PulseParams {
                    rate_start: 30.0,
                    rate_end: 250.0,
                    irregular: 0.6,
                    body: 1.0,
                    grit: 0.0,
                    ..pulses(30.0, 1200.0, 0.0, 0.005, sh(0.1, 0.9, 0.2))
                }),
            );
            s.max_voices = 1;
        },
    },
    Preset {
        category: Doors,
        name: "Door open",
        description: "A latch, then creaky hinges",
        build: |s| {
            add(
                s,
                "Latch",
                0.0,
                0.6,
                noise(Band, 3500.0, 3500.0, 0.3, sh(0.0003, 0.002, 0.012)),
            );
            add(
                s,
                "Latch body",
                0.0,
                0.4,
                thump(
                    450.0,
                    320.0,
                    0.005,
                    1.0,
                    0.3,
                    4000.0,
                    sh(0.0005, 0.0, 0.015),
                ),
            );
            // Farnell's creaking door: clean stick-slip clicks, faster and
            // jittery as the force rises, each louder the longer it stuck,
            // through the wood's broad resonances and the panel's delays.
            add(
                s,
                "Creak",
                0.06,
                1.0,
                Generator::Pulses(PulseParams {
                    rate_start: 20.0,
                    rate_end: 180.0,
                    irregular: 0.6,
                    body: 1.0,
                    grit: 0.0,
                    ..pulses(20.0, 1200.0, 0.0, 0.005, sh(0.05, 0.55, 0.2))
                }),
            );
            add(
                s,
                "Swing",
                0.05,
                0.15,
                noise(Band, 400.0, 700.0, 0.2, sh(0.2, 0.2, 0.3)),
            );
            s.max_voices = 1;
        },
    },
    Preset {
        category: Doors,
        name: "Sliding door",
        description: "A sci-fi door: pshh",
        build: |s| {
            // A smooth swish, a motor's whine and a soft stop.
            add(
                s,
                "Swish",
                0.0,
                0.7,
                noise(Band, 600.0, 1400.0, 0.35, sh(0.08, 0.15, 0.2)),
            );
            add(
                s,
                "Motor",
                0.0,
                0.2,
                Generator::Tone(ToneParams {
                    vibrato: 0.1,
                    vibrato_rate: 30.0,
                    cutoff: 1500.0,
                    ..tone(Wave::Saw, 280.0, 420.0, 0.35, sh(0.02, 0.3, 0.1))
                }),
            );
            add(
                s,
                "Stop",
                0.4,
                0.5,
                thump(80.0, 70.0, 0.01, 0.2, 0.7, 900.0, sh(0.002, 0.0, 0.12)),
            );
            s.space = 0.2;
            s.max_voices = 2;
        },
    },
    Preset {
        category: Machines,
        name: "Switch",
        description: "A light switch or a button",
        build: |s| {
            add(
                s,
                "Snap",
                0.0,
                0.6,
                thump(300.0, 200.0, 0.005, 1.0, 0.2, 4000.0, sh(0.0005, 0.0, 0.03)),
            );
            add(
                s,
                "Click",
                0.0,
                0.5,
                Generator::Modal(modal(Material::Metal, 3500.0, 0.03, 0.5, 1.0)),
            );
            s.variation = 0.5;
        },
    },
    Preset {
        category: Machines,
        name: "Lever",
        description: "A heavy lever pulled into place",
        build: |s| {
            // Metal scraping on metal as it moves, then a solid clunk.
            add(
                s,
                "Scrape",
                0.0,
                0.4,
                drifting(Band, 1800.0, 0.5, 0.4, 8.0, sh(0.02, 0.25, 0.08)),
            );
            add(
                s,
                "Clunk",
                0.3,
                0.8,
                thump(120.0, 70.0, 0.02, 0.6, 0.6, 1500.0, sh(0.001, 0.0, 0.15)),
            );
            add(
                s,
                "Clunk noise",
                0.3,
                0.4,
                noise(Band, 600.0, 600.0, 0.3, sh(0.001, 0.01, 0.06)),
            );
            s.space = 0.2;
            s.max_voices = 1;
        },
    },
    Preset {
        category: Machines,
        name: "Ratchet",
        description: "A winch or a crank, click click click",
        build: |s| {
            add(
                s,
                "Clicks",
                0.0,
                0.8,
                Generator::Pulses(PulseParams {
                    irregular: 0.1,
                    ..pulses(14.0, 2500.0, 0.6, 0.0005, sh(0.001, 0.5, 0.05))
                }),
            );
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Speed",
                ControlTarget::Rate,
                "×",
                1.0,
                0.25,
                4.0,
            ));
        },
    },
    Preset {
        category: Machines,
        name: "Engine",
        description: "A four-cylinder car engine, until stopped",
        build: |s| {
            // Firing rate = rpm/60 × cylinders / 2 (four-stroke); the RPM
            // control changes it live while the pipe's resonance stays put.
            add(
                s,
                "Engine",
                0.0,
                0.9,
                Generator::Engine(EngineParams {
                    shape: sh(0.2, 1.0, 0.5),
                    rpm: 850.0,
                    cylinders: 4,
                    two_stroke: false,
                    uneven: 0.0,
                    exhaust: 2.0,
                    load: 0.45,
                    roughness: 0.35,
                    mechanics: 0.3,
                }),
            );
            s.looping = Looping::Sustain;
            s.max_voices = 2;
            s.controls.push(Control::new(
                "RPM",
                ControlTarget::Pitch,
                "rpm",
                850.0,
                600.0,
                7000.0,
            ));
        },
    },
    Preset {
        category: Machines,
        name: "V8 muscle car",
        description: "A big, burbling V8, until stopped",
        build: |s| {
            // A cross-plane V8 fires unevenly between its banks: the burble.
            add(
                s,
                "Engine",
                0.0,
                0.9,
                Generator::Engine(EngineParams {
                    shape: sh(0.2, 1.0, 0.6),
                    rpm: 750.0,
                    cylinders: 8,
                    two_stroke: false,
                    uneven: 0.5,
                    exhaust: 1.2,
                    load: 0.5,
                    roughness: 0.45,
                    mechanics: 0.25,
                }),
            )
            .drive = 0.3;
            s.looping = Looping::Sustain;
            s.max_voices = 2;
            s.controls.push(Control::new(
                "RPM",
                ControlTarget::Pitch,
                "rpm",
                750.0,
                600.0,
                6500.0,
            ));
        },
    },
    Preset {
        category: Machines,
        name: "Motorcycle",
        description: "A V-twin motorbike, until stopped",
        build: |s| {
            // A V-twin's two cylinders fire close together, then pause.
            add(
                s,
                "Engine",
                0.0,
                0.9,
                Generator::Engine(EngineParams {
                    shape: sh(0.2, 1.0, 0.5),
                    rpm: 1000.0,
                    cylinders: 2,
                    two_stroke: false,
                    uneven: 1.0,
                    exhaust: 0.8,
                    load: 0.5,
                    roughness: 0.3,
                    mechanics: 0.35,
                }),
            )
            .drive = 0.3;
            s.looping = Looping::Sustain;
            s.max_voices = 2;
            s.controls.push(Control::new(
                "RPM",
                ControlTarget::Pitch,
                "rpm",
                1000.0,
                800.0,
                9000.0,
            ));
        },
    },
    Preset {
        category: Machines,
        name: "Two-stroke engine",
        description: "A single-cylinder two-stroke (moped, dirt bike), until stopped",
        build: |s| {
            // One cylinder firing on every turn of the crank into an
            // expansion-chamber pipe: the buzzy "ring-ding" of mopeds and
            // dirt bikes, lots of flow noise, few firings to hide it.
            add(
                s,
                "Engine",
                0.0,
                0.9,
                Generator::Engine(EngineParams {
                    shape: sh(0.2, 1.0, 0.4),
                    rpm: 1800.0,
                    cylinders: 1,
                    two_stroke: true,
                    uneven: 0.0,
                    exhaust: 1.0,
                    load: 0.6,
                    roughness: 0.4,
                    mechanics: 0.4,
                }),
            )
            .drive = 0.3;
            s.looping = Looping::Sustain;
            s.max_voices = 2;
            s.controls.push(Control::new(
                "RPM",
                ControlTarget::Pitch,
                "rpm",
                1800.0,
                1200.0,
                9000.0,
            ));
        },
    },
    Preset {
        category: Machines,
        name: "Motor hum",
        description: "An electric motor or a fridge, until stopped",
        build: |s| {
            add(
                s,
                "Hum",
                0.0,
                0.5,
                Generator::Tone(ToneParams {
                    fm: 0.3,
                    vibrato: 0.05,
                    vibrato_rate: 3.0,
                    cutoff: 900.0,
                    ..tone(Wave::Saw, 100.0, 100.0, 0.1, sh(0.2, 1.0, 0.5))
                }),
            )
            .follow = 7.0;
            add(
                s,
                "Whine",
                0.0,
                0.25,
                Generator::Tone(tone(Wave::Sine, 200.0, 200.0, 0.1, sh(0.2, 1.0, 0.5))),
            )
            .follow = 7.0;
            add(
                s,
                "Hiss",
                0.0,
                0.08,
                noise(Band, 2000.0, 2000.0, 0.4, sh(0.2, 1.0, 0.5)),
            );
            s.looping = Looping::Sustain;
            s.max_voices = 2;
            s.controls.push(Control::new(
                "Speed",
                ControlTarget::Pitch,
                "×",
                1.0,
                0.25,
                4.0,
            ));
        },
    },
    Preset {
        category: Machines,
        name: "Clock ticking",
        description: "Tick, tock, until stopped",
        build: |s| {
            // Short, dry escapement clicks, a little lower for the tock.
            add(
                s,
                "Tick",
                0.0,
                0.7,
                noise(Band, 4000.0, 4000.0, 0.4, sh(0.0003, 0.001, 0.006)),
            );
            add(
                s,
                "Tick body",
                0.0,
                0.3,
                thump(
                    1500.0,
                    1200.0,
                    0.002,
                    0.6,
                    0.2,
                    6000.0,
                    sh(0.0003, 0.0, 0.01),
                ),
            );
            add(
                s,
                "Tock",
                0.5,
                0.6,
                noise(Band, 2800.0, 2800.0, 0.4, sh(0.0003, 0.001, 0.008)),
            );
            add(
                s,
                "Tock body",
                0.5,
                0.3,
                thump(
                    1000.0,
                    800.0,
                    0.002,
                    0.6,
                    0.2,
                    5000.0,
                    sh(0.0003, 0.0, 0.012),
                ),
            );
            s.looping = Looping::Repeat { every: 1.0 };
            s.variation = 0.3;
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Speed",
                ControlTarget::Rate,
                "×",
                1.0,
                0.25,
                4.0,
            ));
        },
    },
    Preset {
        category: Machines,
        name: "Chain rattle",
        description: "A chain shaking",
        build: |s| {
            // Links clinking: bright, noisy metal ticks with a short ring,
            // tumbling irregularly.
            add(
                s,
                "Clinks",
                0.0,
                0.6,
                Generator::Noise(NoiseParams {
                    crackle: 1.0,
                    density: 60.0,
                    ..np(Band, 4500.0, 4000.0, 0.7, sh(0.01, 0.45, 0.2))
                }),
            );
            add(
                s,
                "Links",
                0.0,
                0.4,
                Generator::Modal(ModalParams {
                    hits: 16,
                    spread: 0.6,
                    scatter: 6.0,
                    randomize: 0.8,
                    ..modal(Material::Metal, 3500.0, 0.04, 1.0, 1.0)
                }),
            );
            add(
                s,
                "Scrape",
                0.0,
                0.15,
                drifting(Band, 2500.0, 0.3, 0.5, 8.0, sh(0.02, 0.4, 0.2)),
            );
            s.max_voices = 2;
        },
    },
    Preset {
        category: Machines,
        name: "Alarm",
        description: "A rising siren, until stopped",
        build: |s| {
            add(
                s,
                "Siren",
                0.0,
                0.35,
                Generator::Tone(ToneParams {
                    cutoff: 3000.0,
                    ..tone(Wave::Square, 700.0, 1300.0, 0.45, sh(0.01, 0.4, 0.05))
                }),
            );
            s.looping = Looping::Repeat { every: 0.5 };
            s.variation = 0.0;
            s.space = 0.2;
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Speed",
                ControlTarget::Rate,
                "×",
                1.0,
                0.25,
                4.0,
            ));
        },
    },
    Preset {
        category: Machines,
        name: "Robot beeps",
        description: "A small robot chattering",
        build: |s| {
            let a = add(
                s,
                "Beep 1",
                0.0,
                0.35,
                Generator::Tone(ToneParams {
                    jump: -5.0,
                    jump_time: 0.06,
                    crush: 0.3,
                    ..tone(Wave::Square, 1200.0, 1200.0, 0.01, sh(0.002, 0.1, 0.03))
                }),
            );
            a.jitter.pitch = 4.0;
            let b = add(
                s,
                "Beep 2",
                0.15,
                0.35,
                Generator::Tone(ToneParams {
                    jump: 7.0,
                    jump_time: 0.05,
                    crush: 0.3,
                    ..tone(Wave::Square, 1600.0, 1600.0, 0.01, sh(0.002, 0.1, 0.03))
                }),
            );
            b.jitter.pitch = 4.0;
            b.jitter.timing = 0.03;
        },
    },
    Preset {
        category: Machines,
        name: "Servo",
        description: "A robot arm moving",
        build: |s| {
            add(
                s,
                "Motor",
                0.0,
                0.6,
                Generator::Tone(ToneParams {
                    fm: 0.3,
                    vibrato: 0.1,
                    vibrato_rate: 30.0,
                    cutoff: 1500.0,
                    ..tone(Wave::Saw, 300.0, 500.0, 0.3, sh(0.02, 0.25, 0.08))
                }),
            );
            add(
                s,
                "Gears",
                0.0,
                0.1,
                noise(Band, 2000.0, 2000.0, 0.3, sh(0.02, 0.25, 0.08)),
            );
            s.max_voices = 2;
        },
    },
    Preset {
        category: Machines,
        name: "Steam hiss",
        description: "Pressure escaping from a pipe",
        build: |s| {
            add(
                s,
                "Hiss",
                0.0,
                0.6,
                noise(High, 4000.0, 3000.0, 0.1, sh(0.02, 0.5, 0.4)),
            );
            add(
                s,
                "Body",
                0.0,
                0.3,
                noise(Band, 2000.0, 1500.0, 0.4, sh(0.02, 0.5, 0.4)),
            );
        },
    },
    // Nature & weather
    Preset {
        category: Nature,
        name: "Rain",
        description: "Steady rain, until stopped",
        build: |s| {
            add(
                s,
                "Drops",
                0.0,
                0.4,
                Generator::Noise(NoiseParams {
                    crackle: 0.9,
                    density: 1500.0,
                    wobble: 0.2,
                    wobble_rate: 0.3,
                    ..np(High, 3000.0, 3000.0, 0.1, sh(0.5, 1.0, 1.0))
                }),
            );
            add(
                s,
                "Wash",
                0.0,
                0.25,
                drifting(Low, 1200.0, 0.0, 0.3, 0.3, sh(0.5, 1.0, 1.0)),
            );
            add(
                s,
                "Splats",
                0.0,
                0.2,
                crackle(Band, 1800.0, 1800.0, 40.0, 1.0, sh(0.5, 1.0, 1.0)),
            );
            s.looping = Looping::Sustain;
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Heaviness",
                ControlTarget::Intensity,
                "×",
                1.0,
                0.2,
                2.0,
            ));
        },
    },
    Preset {
        category: Nature,
        name: "Wind",
        description: "Gusting wind, until stopped",
        build: |s| {
            add(
                s,
                "Gusts",
                0.0,
                0.9,
                drifting(Band, 500.0, 0.6, 1.2, 0.35, sh(1.0, 1.0, 1.5)),
            );
            add(
                s,
                "Whistle",
                0.0,
                0.3,
                drifting(Band, 1500.0, 0.75, 1.0, 0.5, sh(1.0, 1.0, 1.5)),
            );
            s.looping = Looping::Sustain;
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Strength",
                ControlTarget::Intensity,
                "×",
                1.0,
                0.2,
                2.0,
            ));
        },
    },
    Preset {
        category: Nature,
        name: "Thunder",
        description: "A close thunderclap rolling away",
        build: |s| {
            // A tearing crack, a deep boom and a long rolling rumble (after
            // Fineberg, Walters & Reiss 2022): all noise, no ringing tones.
            add(
                s,
                "Crack",
                0.0,
                0.7,
                noise(Low, 6000.0, 1200.0, 0.0, sh(0.001, 0.05, 0.7)),
            );
            add(
                s,
                "Tear",
                0.0,
                0.4,
                crackle(Low, 4000.0, 800.0, 90.0, 0.9, sh(0.005, 0.3, 1.2)),
            );
            add(
                s,
                "Boom",
                0.05,
                0.6,
                Generator::Thump(ThumpParams {
                    blast: 0.3,
                    ..thump_p(45.0, 28.0, 0.3, 0.0, 0.95, 250.0, sh(0.02, 0.3, 3.0))
                }),
            );
            add(
                s,
                "Rumble",
                0.2,
                0.5,
                drifting(Low, 220.0, 0.1, 1.2, 2.5, sh(0.3, 0.8, 8.0)),
            );
            add(
                s,
                "Deepener",
                0.1,
                0.45,
                drifting(Low, 70.0, 0.1, 0.6, 1.0, sh(0.2, 1.0, 6.0)),
            );
            s.space = 0.6;
            s.max_voices = 2;
        },
    },
    Preset {
        category: Nature,
        name: "Fire",
        description: "A campfire, until stopped",
        build: |s| {
            // Farnell's fire: sparse crackles, a flickering hiss and a low
            // "lapping" roar, mixed 0.1 / 0.3 / 0.6.
            add(
                s,
                "Crackles",
                0.0,
                0.4,
                crackle(Band, 2500.0, 2500.0, 6.0, 1.0, sh(0.3, 1.0, 0.8)),
            );
            add(
                s,
                "Hiss",
                0.0,
                0.1,
                drifting(High, 3000.0, 0.1, 0.5, 1.0, sh(0.3, 1.0, 0.8)),
            );
            let lap = add(
                s,
                "Lapping",
                0.0,
                0.6,
                drifting(Band, 30.0, 0.8, 0.8, 2.0, sh(0.3, 1.0, 0.8)),
            );
            lap.drive = 0.3;
            s.looping = Looping::Sustain;
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Size",
                ControlTarget::Intensity,
                "×",
                1.0,
                0.2,
                2.0,
            ));
        },
    },
    Preset {
        category: Nature,
        name: "Bonfire",
        description: "A big roaring fire, until stopped",
        build: |s| {
            add(
                s,
                "Crackles",
                0.0,
                0.4,
                crackle(Band, 2500.0, 2500.0, 25.0, 1.0, sh(0.3, 1.0, 0.8)),
            );
            add(
                s,
                "Flames",
                0.0,
                0.5,
                drifting(Low, 600.0, 0.1, 0.6, 3.0, sh(0.3, 1.0, 0.8)),
            );
            add(
                s,
                "Hiss",
                0.0,
                0.1,
                drifting(High, 3000.0, 0.1, 0.5, 1.0, sh(0.3, 1.0, 0.8)),
            );
            let lap = add(
                s,
                "Lapping",
                0.0,
                0.5,
                drifting(Band, 30.0, 0.8, 0.8, 2.0, sh(0.3, 1.0, 0.8)),
            );
            lap.drive = 0.3;
            s.looping = Looping::Sustain;
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Size",
                ControlTarget::Intensity,
                "×",
                1.0,
                0.2,
                2.0,
            ));
        },
    },
    Preset {
        category: Footsteps,
        name: "Step on sand",
        description: "A step in soft sand",
        build: |s| {
            add(
                s,
                "Swish",
                0.0,
                0.7,
                noise(Band, 2500.0, 500.0, 0.2, sh(0.001, 0.05, 0.3)),
            );
            add(
                s,
                "Grains",
                0.0,
                0.3,
                crackle(High, 5000.0, 4000.0, 400.0, 0.9, sh(0.01, 0.08, 0.2)),
            );
            add(
                s,
                "Thud",
                0.0,
                0.5,
                thump(150.0, 70.0, 0.02, 0.2, 0.9, 1000.0, sh(0.001, 0.0, 0.12)),
            );
            s.max_voices = 6;
        },
    },
    Preset {
        category: Nature,
        name: "Bubbles",
        description: "Bubbling water, until stopped",
        build: |s| {
            // Van den Doel's bubble model: a few large, slowly rising bubbles.
            add(
                s,
                "Bubbles",
                0.0,
                1.19,
                bubbles(8.0, 3.0, 6.0, 1.0, 0.5, sh(0.05, 1.0, 0.5)),
            );
            s.looping = Looping::Sustain;
            s.max_voices = 1;
        },
    },
    Preset {
        category: Nature,
        name: "Water drip",
        description: "A single drop in a cave",
        build: |s| {
            // van den Doel (2005): a bubble rings at f ≈ 3/r (2–7 mm drops:
            // ~430–1500 Hz), decays at 0.043f + 0.0014f^1.5 per second, and
            // its pitch rises by well under an octave.
            add(
                s,
                "Drop",
                0.0,
                0.7,
                Generator::Tone(tone(Wave::Sine, 1000.0, 1600.0, 0.08, sh(0.001, 0.0, 0.08))),
            )
            .jitter
            .pitch = 3.0;
            s.space = 0.5;
        },
    },
    Preset {
        category: Nature,
        name: "Splash",
        description: "Something falling into water",
        build: |s| {
            // Mostly a turbulent rush of water, with just a few bubbles and
            // a spray of droplets.
            add(
                s,
                "Rush",
                0.0,
                0.8,
                Generator::Noise(NoiseParams {
                    wobble: 0.5,
                    wobble_rate: 20.0,
                    ..np(Band, 1500.0, 500.0, 0.1, sh(0.005, 0.05, 0.35))
                }),
            );
            add(
                s,
                "Spray",
                0.02,
                0.08,
                crackle(High, 3000.0, 3000.0, 400.0, 0.9, sh(0.01, 0.1, 0.3)),
            );
            add(
                s,
                "Bubbles",
                0.0,
                0.15,
                bubbles(600.0, 1.0, 6.0, 2.0, 0.3, sh(0.001, 0.05, 0.25)),
            );
        },
    },
    Preset {
        category: Nature,
        name: "Stream",
        description: "Running water, until stopped",
        build: |s| {
            // Running water: a dense spray of small bubbles under a turbulent
            // wash.
            add(
                s,
                "Bubbles",
                0.0,
                0.3,
                bubbles(1500.0, 0.5, 3.0, 3.0, 0.5, sh(0.5, 1.0, 1.0)),
            );
            add(
                s,
                "Wash",
                0.0,
                0.5,
                drifting(Band, 1500.0, 0.1, 0.5, 3.0, sh(0.5, 1.0, 1.0)),
            );
            add(
                s,
                "Depth",
                0.0,
                0.25,
                drifting(Low, 500.0, 0.1, 0.4, 2.0, sh(0.5, 1.0, 1.0)),
            );
            s.looping = Looping::Sustain;
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Flow",
                ControlTarget::Intensity,
                "×",
                1.0,
                0.2,
                2.0,
            ));
        },
    },
    Preset {
        category: Voices,
        name: "Grunt",
        description: "An effort: lifting, pushing, swinging",
        build: |s| {
            add(
                s,
                "Voice",
                0.0,
                1.0,
                Generator::Voice(VoiceParams {
                    pitch: c(&[(0.0, 120.0), (0.3, 135.0), (1.0, 95.0)]),
                    openness: c(&[(0.0, 0.7), (1.0, 0.5)]),
                    frontness: flat(0.25),
                    quality: c(&[(0.0, 0.15), (1.0, 0.6)]),
                    loudness: c(&[(0.0, 1.0), (0.6, 0.8), (1.0, 0.3)]),
                    roughness: 0.35,
                    subharmonics: 0.2,
                    breath: 0.15,
                    ..voice(sh(0.01, 0.1, 0.1))
                }),
            );
            add(
                s,
                "Breath",
                0.0,
                0.2,
                noise(Band, 900.0, 900.0, 0.2, sh(0.005, 0.03, 0.05)),
            );
            s.max_voices = 2;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                115.0,
                60.0,
                300.0,
            ));
        },
    },
    Preset {
        category: Voices,
        name: "Pain",
        description: "Ow! Taking a hit",
        build: |s| {
            add(
                s,
                "Voice",
                0.0,
                0.9,
                Generator::Voice(VoiceParams {
                    pitch: c(&[(0.0, 200.0), (0.15, 260.0), (0.5, 230.0), (1.0, 150.0)]),
                    openness: c(&[(0.0, 0.9), (0.5, 0.7), (1.0, 0.05)]),
                    frontness: c(&[(0.0, 0.2), (1.0, 0.0)]),
                    quality: c(&[(0.0, 0.25), (1.0, 0.6)]),
                    loudness: c(&[(0.0, 0.8), (0.15, 1.0), (1.0, 0.5)]),
                    roughness: 0.3,
                    subharmonics: 0.15,
                    vibrato: 0.3,
                    vibrato_rate: 6.0,
                    ..voice(sh(0.01, 0.2, 0.2))
                }),
            );
            s.max_voices = 2;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                220.0,
                110.0,
                450.0,
            ));
        },
    },
    Preset {
        category: Voices,
        name: "Hup",
        description: "A short breath when jumping",
        build: |s| {
            add(
                s,
                "Voice",
                0.01,
                1.25,
                Generator::Voice(VoiceParams {
                    pitch: c(&[(0.0, 180.0), (1.0, 230.0)]),
                    openness: c(&[(0.0, 0.5), (1.0, 0.3)]),
                    frontness: flat(0.3),
                    quality: c(&[(0.0, 0.8), (0.4, 0.4), (1.0, 0.7)]),
                    loudness: c(&[(0.0, 0.7), (0.3, 1.0), (1.0, 0.6)]),
                    roughness: 0.2,
                    breath: 0.3,
                    ..voice(sh(0.005, 0.05, 0.07))
                }),
            );
            s.max_voices = 2;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                205.0,
                100.0,
                400.0,
            ));
        },
    },
    Preset {
        category: Voices,
        name: "Yell",
        description: "Hey! A shout to get attention",
        build: |s| {
            add(
                s,
                "Voice",
                0.03,
                0.9,
                Generator::Voice(VoiceParams {
                    pitch: c(&[(0.0, 240.0), (0.2, 330.0), (0.7, 310.0), (1.0, 220.0)]),
                    openness: c(&[(0.0, 0.7), (0.6, 0.5), (1.0, 0.1)]),
                    frontness: c(&[(0.0, 0.9), (1.0, 1.0)]),
                    quality: c(&[(0.0, 1.0), (0.2, 0.3), (1.0, 0.5)]),
                    loudness: c(&[(0.0, 0.7), (0.2, 1.0), (0.8, 0.9), (1.0, 0.4)]),
                    roughness: 0.25,
                    ..voice(sh(0.03, 0.25, 0.2))
                }),
            );
            s.space = 0.2;
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                300.0,
                150.0,
                600.0,
            ));
        },
    },
    Preset {
        category: Voices,
        name: "Death groan",
        description: "A last, fading groan",
        build: |s| {
            add(
                s,
                "Voice",
                0.0,
                0.9,
                Generator::Voice(VoiceParams {
                    pitch: c(&[(0.0, 150.0), (0.2, 160.0), (1.0, 70.0)]),
                    openness: c(&[(0.0, 0.6), (1.0, 0.2)]),
                    frontness: c(&[(0.0, 0.3), (1.0, 0.1)]),
                    quality: c(&[(0.0, 0.4), (1.0, 0.9)]),
                    loudness: c(&[(0.0, 0.8), (0.3, 1.0), (1.0, 0.3)]),
                    roughness: 0.5,
                    subharmonics: 0.3,
                    breath: 0.2,
                    vibrato: 0.4,
                    vibrato_rate: 5.0,
                    ..voice(sh(0.05, 0.5, 0.6))
                }),
            );
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                120.0,
                60.0,
                250.0,
            ));
        },
    },
    Preset {
        category: Voices,
        name: "Scream",
        description: "A long, frightened scream",
        build: |s| {
            // soundgen's scream contour, made unsteady: the pitch jumps and
            // wavers, the voice breaks into two pitches (biphonation) and
            // rattles in the 30–150 Hz roughness band (Arnal 2015).
            add(
                s,
                "Voice",
                0.0,
                0.8,
                Generator::Voice(VoiceParams {
                    pitch: c(&[
                        (0.0, 800.0),
                        (0.08, 1600.0),
                        (0.25, 1450.0),
                        (0.35, 1750.0),
                        (0.55, 1500.0),
                        (0.7, 1650.0),
                        (0.9, 1300.0),
                        (1.0, 1000.0),
                    ]),
                    openness: c(&[(0.0, 0.8), (0.2, 1.0), (0.6, 0.9), (1.0, 0.7)]),
                    frontness: c(&[(0.0, 0.5), (0.5, 0.7), (1.0, 0.5)]),
                    quality: flat(0.0),
                    loudness: c(&[
                        (0.0, 0.8),
                        (0.08, 1.0),
                        (0.3, 0.85),
                        (0.4, 1.0),
                        (0.7, 0.9),
                        (1.0, 0.5),
                    ]),
                    size: 0.9,
                    roughness: 0.7,
                    subharmonics: 0.5,
                    split: 0.4,
                    split_ratio: 1.41,
                    rasp: 0.4,
                    rasp_rate: 80.0,
                    breath: 0.15,
                    vibrato: 0.4,
                    vibrato_rate: 6.0,
                    ..voice(sh(0.02, 0.7, 0.4))
                }),
            )
            .drive = 0.35;
            s.space = 0.3;
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                1400.0,
                600.0,
                2200.0,
            ));
        },
    },
    Preset {
        category: Voices,
        name: "Laugh",
        description: "Ha ha ha!",
        build: |s| {
            // Bachorowski et al. (2001): ~0.17 s calls every ~0.3 s, the
            // first one longer; men ~272 Hz; flat or falling pitch; often
            // breathy or unvoiced.
            for (i, (t, pitch, hold)) in [
                (0.0, 285.0, 0.2),
                (0.32, 272.0, 0.1),
                (0.62, 262.0, 0.1),
                (0.92, 250.0, 0.1),
            ]
            .into_iter()
            .enumerate()
            {
                let l = add(
                    s,
                    &format!("Ha {}", i + 1),
                    t,
                    1.0,
                    Generator::Voice(VoiceParams {
                        pitch: c(&[(0.0, pitch), (1.0, pitch * 0.9)]),
                        openness: c(&[(0.0, 0.5), (0.3, 1.0), (1.0, 0.8)]),
                        frontness: flat(0.3),
                        quality: c(&[(0.0, 0.95), (0.3, 0.6), (1.0, 0.75)]),
                        loudness: c(&[(0.0, 0.5), (0.3, 1.0), (1.0, 0.6)]),
                        roughness: 0.15,
                        breath: 0.4,
                        ..voice(sh(0.01, hold, 0.06))
                    }),
                );
                l.jitter.timing = 0.02;
            }
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                260.0,
                130.0,
                520.0,
            ));
        },
    },
    Preset {
        category: Voices,
        name: "Gasp",
        description: "A sharp breath in: surprise",
        build: |s| {
            // A sharp breath in: only breath, through the mouth's shape.
            add(
                s,
                "Breath",
                0.0,
                0.9,
                Generator::Voice(VoiceParams {
                    pitch: c(&[(0.0, 330.0), (1.0, 360.0)]),
                    openness: c(&[(0.0, 0.4), (1.0, 0.7)]),
                    frontness: flat(0.3),
                    quality: flat(1.0),
                    loudness: c(&[(0.0, 0.4), (0.4, 1.0), (1.0, 0.6)]),
                    breath: 1.0,
                    ..voice(sh(0.08, 0.05, 0.12))
                }),
            );
            s.max_voices = 1;
        },
    },
    Preset {
        category: Voices,
        name: "Cough",
        description: "Two coughs",
        build: |s| {
            // A cough is mostly an explosive burst of breath (then a quieter
            // tail), shaped by the open mouth, with the chest behind it.
            for (i, t) in [0.0, 0.3].into_iter().enumerate() {
                add(
                    s,
                    &format!("Cough {}", i + 1),
                    t,
                    1.0,
                    Generator::Voice(VoiceParams {
                        pitch: c(&[(0.0, 150.0), (1.0, 120.0)]),
                        openness: c(&[(0.0, 0.6), (0.2, 0.9), (1.0, 0.5)]),
                        frontness: flat(0.3),
                        quality: flat(1.0),
                        loudness: c(&[(0.0, 1.0), (0.25, 0.6), (1.0, 0.2)]),
                        roughness: 0.6,
                        breath: 1.0,
                        ..voice(sh(0.002, 0.04, 0.2))
                    }),
                );
                add(
                    s,
                    &format!("Chest {}", i + 1),
                    t,
                    0.5,
                    thump(110.0, 70.0, 0.02, 0.2, 0.9, 900.0, sh(0.001, 0.0, 0.08)),
                );
            }
            s.max_voices = 1;
        },
    },
    Preset {
        category: Voices,
        name: "Sigh",
        description: "A tired breath out",
        build: |s| {
            add(
                s,
                "Voice",
                0.0,
                1.0,
                Generator::Voice(VoiceParams {
                    pitch: c(&[(0.0, 220.0), (1.0, 140.0)]),
                    openness: c(&[(0.0, 0.9), (1.0, 0.4)]),
                    frontness: flat(0.2),
                    quality: flat(0.95),
                    loudness: c(&[(0.0, 0.6), (0.3, 1.0), (1.0, 0.5)]),
                    breath: 0.7,
                    ..voice(sh(0.15, 0.2, 0.4))
                }),
            );
            s.max_voices = 1;
        },
    },
    Preset {
        category: Creatures,
        name: "Dog bark",
        description: "A big dog's rough woof (stylized)",
        build: |s| {
            // A rough, breathy woof through a big throat, over a bassy chest
            // voice: the closest formant synthesis got to a bark, by ear.
            add(
                s,
                "Woof",
                0.0,
                1.0,
                Generator::Voice(VoiceParams {
                    pitch: c(&[(0.0, 280.0), (0.25, 360.0), (1.0, 250.0)]),
                    openness: c(&[(0.0, 0.1), (0.25, 1.0), (0.7, 0.8), (1.0, 0.1)]),
                    frontness: flat(0.1),
                    quality: flat(0.2),
                    loudness: c(&[(0.0, 0.8), (0.2, 1.0), (1.0, 0.3)]),
                    size: 1.4,
                    roughness: 0.75,
                    subharmonics: 0.6,
                    breath: 0.35,
                    ..voice(sh(0.006, 0.12, 0.12))
                }),
            )
            .drive = 0.55;
            add(
                s,
                "Chest",
                0.0,
                0.8,
                Generator::Voice(VoiceParams {
                    pitch: c(&[(0.0, 280.0), (0.25, 360.0), (1.0, 250.0)]),
                    openness: flat(0.3),
                    frontness: flat(0.0),
                    quality: flat(0.2),
                    loudness: c(&[(0.0, 0.8), (0.2, 1.0), (1.0, 0.3)]),
                    size: 2.0,
                    roughness: 0.75,
                    subharmonics: 0.6,
                    ..voice(sh(0.006, 0.12, 0.12))
                }),
            )
            .drive = 0.55;
            s.max_voices = 2;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                310.0,
                155.0,
                620.0,
            ));
        },
    },
    Preset {
        category: Creatures,
        name: "Dog growl",
        description: "A big dog, warning you",
        build: |s| {
            // Faragó et al. 2010: large dogs growl at ~90 Hz with formants
            // ~670 Hz apart (a human man's are ~1 kHz apart: size 1.5).
            add(
                s,
                "Growl",
                0.0,
                0.8,
                Generator::Voice(VoiceParams {
                    pitch: c(&[(0.0, 85.0), (0.5, 95.0), (1.0, 85.0)]),
                    openness: flat(0.3),
                    frontness: flat(0.1),
                    quality: flat(0.25),
                    loudness: c(&[(0.0, 0.6), (0.2, 1.0), (0.45, 0.75), (0.7, 1.0), (1.0, 0.7)]),
                    size: 1.5,
                    roughness: 0.55,
                    subharmonics: 0.8,
                    breath: 0.2,
                    ..voice(sh(0.1, 0.7, 0.3))
                }),
            );
            add(
                s,
                "Rattle",
                0.0,
                0.1,
                Generator::Pulses(PulseParams {
                    irregular: 0.6,
                    ..pulses(30.0, 300.0, 0.2, 0.004, sh(0.1, 0.7, 0.3))
                }),
            );
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                90.0,
                50.0,
                180.0,
            ));
        },
    },
    Preset {
        category: Creatures,
        name: "Cat meow",
        description: "Mee-ow",
        build: |s| {
            // Schötz et al. 2024: ~0.7 s, around 525 Hz, rising then falling;
            // the mouth opens quickly from "mi" to "a" and closes on "ow".
            add(
                s,
                "Meow",
                0.0,
                0.7,
                Generator::Voice(VoiceParams {
                    pitch: c(&[(0.0, 480.0), (0.3, 700.0), (0.7, 620.0), (1.0, 450.0)]),
                    openness: c(&[(0.0, 0.1), (0.2, 0.9), (0.7, 0.8), (1.0, 0.2)]),
                    frontness: c(&[(0.0, 0.9), (0.25, 0.5), (0.6, 0.1), (1.0, 0.0)]),
                    quality: flat(0.4),
                    loudness: c(&[(0.0, 0.5), (0.25, 1.0), (0.8, 0.8), (1.0, 0.4)]),
                    size: 0.45,
                    vibrato: 0.3,
                    vibrato_rate: 6.0,
                    ..voice(sh(0.03, 0.4, 0.27))
                }),
            );
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                560.0,
                300.0,
                1000.0,
            ));
        },
    },
    Preset {
        category: Creatures,
        name: "Cow moo",
        description: "Mooo",
        build: |s| {
            // A closed-mouth hum (~80 Hz) opening into the call (~150 Hz),
            // Padilla de la Torre et al. 2015; a big, rough throat.
            add(
                s,
                "Moo",
                0.0,
                0.9,
                Generator::Voice(VoiceParams {
                    pitch: c(&[(0.0, 85.0), (0.25, 150.0), (0.8, 140.0), (1.0, 110.0)]),
                    openness: c(&[(0.0, 0.0), (0.3, 0.6), (0.85, 0.5), (1.0, 0.1)]),
                    frontness: flat(0.1),
                    quality: flat(0.2),
                    loudness: c(&[(0.0, 0.6), (0.25, 1.0), (1.0, 0.7)]),
                    size: 2.6,
                    roughness: 0.5,
                    subharmonics: 0.3,
                    breath: 0.1,
                    ..voice(sh(0.1, 1.1, 0.4))
                }),
            )
            .drive = 0.3;
            s.space = 0.3;
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                130.0,
                70.0,
                260.0,
            ));
        },
    },
    Preset {
        category: Magic,
        name: "Ghost moan",
        description: "A low, hollow moan",
        build: |s| {
            // Padilla de la Torre et al. 2015: open-mouth calls ~153 Hz;
            // calf calls ~1.4 s. (Throat size is a guess: adult formant data
            // not found.)
            add(
                s,
                "Moo",
                0.0,
                0.8,
                Generator::Voice(VoiceParams {
                    pitch: c(&[(0.0, 140.0), (0.2, 160.0), (0.8, 150.0), (1.0, 130.0)]),
                    openness: c(&[(0.0, 0.1), (0.25, 0.6), (1.0, 0.3)]),
                    frontness: flat(0.05),
                    quality: flat(0.35),
                    loudness: c(&[(0.0, 0.5), (0.2, 1.0), (1.0, 0.7)]),
                    size: 1.9,
                    roughness: 0.35,
                    subharmonics: 0.2,
                    vibrato: 0.2,
                    vibrato_rate: 4.0,
                    ..voice(sh(0.1, 0.9, 0.4))
                }),
            );
            s.space = 0.3;
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                130.0,
                60.0,
                260.0,
            ));
        },
    },
    Preset {
        category: Creatures,
        name: "Bird chirp",
        description: "A small bird singing",
        build: |s| {
            // House sparrow chirps: ~150–250 ms, 3–7 kHz.
            for (i, (t, from, to)) in [
                (0.0, 3500.0, 5000.0),
                (0.25, 4000.0, 5500.0),
                (0.5, 3800.0, 6000.0),
            ]
            .into_iter()
            .enumerate()
            {
                let l = add(
                    s,
                    &format!("Chirp {}", i + 1),
                    t,
                    0.6,
                    Generator::Tone(ToneParams {
                        vibrato: 1.0,
                        vibrato_rate: 40.0,
                        ..tone(Wave::Sine, from, to, 0.05, sh(0.005, 0.06, 0.1))
                    }),
                );
                l.jitter.pitch = 2.0;
                l.jitter.timing = 0.02;
            }
            s.space = 0.3;
        },
    },
    Preset {
        category: Creatures,
        name: "Crow caw",
        description: "Caw! Caw!",
        build: |s| {
            for (i, t) in [0.0, 0.45].into_iter().enumerate() {
                add(
                    s,
                    &format!("Caw {}", i + 1),
                    t,
                    0.8,
                    Generator::Voice(VoiceParams {
                        pitch: c(&[(0.0, 550.0), (0.3, 620.0), (1.0, 480.0)]),
                        openness: flat(1.0),
                        frontness: flat(0.1),
                        quality: flat(0.0),
                        loudness: c(&[(0.0, 0.7), (0.3, 1.0), (1.0, 0.6)]),
                        size: 0.8,
                        roughness: 0.6,
                        subharmonics: 0.7,
                        split: 0.4,
                        split_ratio: 1.27,
                        breath: 0.1,
                        ..voice(sh(0.01, 0.25, 0.12))
                    }),
                );
            }
            s.space = 0.4;
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                550.0,
                300.0,
                1000.0,
            ));
        },
    },
    Preset {
        category: Creatures,
        name: "Frog croak",
        description: "Ribbit",
        build: |s| {
            // A low, round throat pulsing at ~33 times a second (common frog:
            // 31–35 pulses/s).
            for (i, t) in [0.0, 0.45].into_iter().enumerate() {
                add(
                    s,
                    &format!("Croak {}", i + 1),
                    t,
                    0.9,
                    Generator::Voice(VoiceParams {
                        pitch: c(&[(0.0, 95.0), (1.0, 110.0)]),
                        openness: flat(0.15),
                        frontness: flat(0.0),
                        quality: flat(0.1),
                        loudness: c(&[(0.0, 0.7), (0.3, 1.0), (1.0, 0.6)]),
                        size: 1.4,
                        roughness: 0.5,
                        subharmonics: 0.6,
                        rasp: 0.9,
                        rasp_rate: 33.0,
                        breath: 0.05,
                        ..voice(sh(0.01, 0.3, 0.1))
                    }),
                )
                .drive = 0.2;
            }
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                100.0,
                60.0,
                200.0,
            ));
        },
    },
    Preset {
        category: Creatures,
        name: "Wolf howl",
        description: "A howl at the moon",
        build: |s| {
            // Kershenbaum et al. 2018: ~420–620 Hz, 3–5 s. The pitch rises and
            // falls gently; "Pitch range" makes the swoop bigger or smaller.
            add(
                s,
                "Howl",
                0.0,
                0.6,
                Generator::Voice(VoiceParams {
                    pitch: c(&[(0.0, 380.0), (0.3, 470.0), (0.7, 470.0), (1.0, 400.0)]),
                    openness: c(&[(0.0, 0.1), (0.2, 0.55), (1.0, 0.25)]),
                    frontness: c(&[(0.0, 0.0), (1.0, 0.1)]),
                    quality: flat(0.35),
                    loudness: c(&[(0.0, 0.5), (0.15, 1.0), (0.85, 0.9), (1.0, 0.5)]),
                    size: 1.2,
                    roughness: 0.25,
                    subharmonics: 0.15,
                    breath: 0.1,
                    vibrato: 0.1,
                    vibrato_rate: 5.0,
                    ..voice(sh(0.2, 2.8, 1.0))
                }),
            )
            .drive = 0.2;
            s.space = 0.6;
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                430.0,
                250.0,
                700.0,
            ));
            s.controls.push(Control::new(
                "Pitch range",
                ControlTarget::Swing,
                "×",
                1.0,
                0.0,
                4.0,
            ));
        },
    },
    Preset {
        category: Magic,
        name: "Ghost wail",
        description: "A long, rising wail",
        build: |s| {
            // Kershenbaum et al. 2018: mean F0 ~420–620 Hz, howls ~3–5 s.
            add(
                s,
                "Howl",
                0.0,
                0.4,
                Generator::Voice(VoiceParams {
                    pitch: c(&[(0.0, 350.0), (0.2, 480.0), (0.8, 460.0), (1.0, 400.0)]),
                    openness: c(&[(0.0, 0.1), (0.3, 0.5), (1.0, 0.2)]),
                    frontness: c(&[(0.0, 0.0), (1.0, 0.1)]),
                    quality: flat(0.45),
                    loudness: c(&[(0.0, 0.4), (0.2, 1.0), (0.85, 0.9), (1.0, 0.5)]),
                    breath: 0.15,
                    vibrato: 0.4,
                    vibrato_rate: 5.0,
                    ..voice(sh(0.3, 2.8, 1.0))
                }),
            );
            s.space = 0.6;
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                430.0,
                200.0,
                800.0,
            ));
        },
    },
    Preset {
        category: Creatures,
        name: "Pig oink",
        description: "Oink oink",
        build: |s| {
            // Low, nasal grunts with a croaky, pulsing rasp.
            for (i, t) in [0.0, 0.22].into_iter().enumerate() {
                add(
                    s,
                    &format!("Grunt {}", i + 1),
                    t,
                    0.9,
                    Generator::Voice(VoiceParams {
                        pitch: c(&[(0.0, 140.0), (0.3, 170.0), (1.0, 120.0)]),
                        openness: flat(0.3),
                        frontness: flat(0.1),
                        quality: flat(0.0),
                        size: 1.3,
                        roughness: 0.9,
                        subharmonics: 0.7,
                        rasp: 0.7,
                        rasp_rate: 40.0,
                        breath: 0.1,
                        ..voice(sh(0.01, 0.1, 0.08))
                    }),
                );
            }
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                150.0,
                80.0,
                300.0,
            ));
        },
    },
    Preset {
        category: Creatures,
        name: "Duck quack",
        description: "Quack",
        build: |s| {
            // The dog's bunched, high-F1 throat pitched down to ~200 Hz
            // quacks: a mallard's decrescendo of ~0.25 s notes, each
            // quieter and lower.
            for (i, (t, g, p)) in [(0.0, 1.0, 1.0), (0.3, 0.8, 0.94), (0.56, 0.65, 0.88)]
                .into_iter()
                .enumerate()
            {
                add(
                    s,
                    &format!("Quack {}", i + 1),
                    t,
                    g,
                    Generator::Voice(VoiceParams {
                        pitch: c(&[
                            (0.0, 190.0 * p),
                            (0.2, 218.0 * p),
                            (0.7, 205.0 * p),
                            (1.0, 165.0 * p),
                        ]),
                        openness: c(&[(0.0, 0.3), (0.15, 1.0), (0.7, 0.9), (1.0, 0.2)]),
                        frontness: flat(0.5),
                        quality: flat(0.15),
                        loudness: c(&[(0.0, 0.9), (0.15, 1.0), (0.75, 0.9), (1.0, 0.3)]),
                        roughness: 0.4,
                        subharmonics: 0.3,
                        breath: 0.1,
                        tract: Tract::Dog,
                        ..voice(sh(0.004, 0.12, 0.12))
                    }),
                )
                .drive = 0.3;
            }
            s.max_voices = 2;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                200.0,
                120.0,
                400.0,
            ));
        },
    },
    Preset {
        category: Creatures,
        name: "Owl hoot",
        description: "A great horned owl (300–400 Hz)",
        build: |s| {
            for (name, t, hold, decay) in [("Hoo", 0.0, 0.15, 0.15), ("Hoooo", 0.5, 0.35, 0.25)] {
                add(
                    s,
                    name,
                    t,
                    0.9,
                    Generator::Voice(VoiceParams {
                        pitch: c(&[(0.0, 360.0), (0.3, 395.0), (1.0, 350.0)]),
                        openness: flat(0.05),
                        frontness: flat(0.0),
                        quality: flat(0.75),
                        loudness: c(&[(0.0, 0.6), (0.3, 1.0), (1.0, 0.7)]),
                        roughness: 0.0,
                        breath: 0.3,
                        ..voice(sh(0.04, hold, decay))
                    }),
                );
            }
            s.space = 0.5;
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                370.0,
                250.0,
                700.0,
            ));
        },
    },
    Preset {
        category: Creatures,
        name: "Horse neigh",
        description: "A whinnying horse",
        build: |s| {
            // Briefer et al. 2015: two independent pitches ~4× apart. A long
            // whinny, a falling "heh-heh-heh", and a snort through the nose.
            add(
                s,
                "Neigh",
                0.0,
                0.7,
                Generator::Voice(VoiceParams {
                    pitch: c(&[(0.0, 300.0), (0.15, 410.0), (0.5, 300.0), (1.0, 200.0)]),
                    openness: c(&[(0.0, 0.5), (0.3, 0.9), (1.0, 0.7)]),
                    frontness: c(&[(0.0, 0.9), (1.0, 0.5)]),
                    quality: flat(0.2),
                    loudness: c(&[(0.0, 0.6), (0.15, 1.0), (1.0, 0.5)]),
                    size: 1.3,
                    roughness: 0.5,
                    split: 0.6,
                    split_ratio: 3.9,
                    vibrato: 1.2,
                    vibrato_rate: 12.0,
                    ..voice(sh(0.05, 0.6, 0.25))
                }),
            );
            for (i, (t, p, g)) in [(0.85, 260.0, 0.6), (1.0, 240.0, 0.5), (1.14, 220.0, 0.4)]
                .into_iter()
                .enumerate()
            {
                add(
                    s,
                    &format!("Heh {}", i + 1),
                    t,
                    g,
                    Generator::Voice(VoiceParams {
                        pitch: c(&[(0.0, p), (1.0, p * 0.85)]),
                        openness: flat(0.7),
                        frontness: flat(0.5),
                        quality: flat(0.3),
                        size: 1.3,
                        roughness: 0.6,
                        split: 0.4,
                        split_ratio: 3.9,
                        ..voice(sh(0.01, 0.05, 0.06))
                    }),
                );
            }
            add(
                s,
                "Snort",
                1.3,
                0.5,
                noise(Band, 900.0, 500.0, 0.3, sh(0.01, 0.05, 0.15)),
            );
            s.space = 0.3;
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                330.0,
                200.0,
                700.0,
            ));
        },
    },
    Preset {
        category: Creatures,
        name: "Monster growl",
        description: "Something big in the dark",
        build: |s| {
            add(
                s,
                "Growl",
                0.0,
                0.9,
                Generator::Voice(VoiceParams {
                    pitch: c(&[(0.0, 65.0), (0.3, 72.0), (1.0, 55.0)]),
                    openness: c(&[(0.0, 0.4), (1.0, 0.2)]),
                    frontness: flat(0.2),
                    quality: flat(0.05),
                    loudness: c(&[(0.0, 0.5), (0.2, 1.0), (0.45, 0.7), (0.7, 1.0), (1.0, 0.6)]),
                    size: 2.6,
                    roughness: 0.9,
                    subharmonics: 0.9,
                    split: 0.3,
                    split_ratio: 0.73,
                    breath: 0.25,
                    ..voice(sh(0.15, 0.9, 0.5))
                }),
            )
            .drive = 0.3;
            add(
                s,
                "Rattle",
                0.0,
                0.12,
                Generator::Pulses(PulseParams {
                    irregular: 0.7,
                    ..pulses(25.0, 200.0, 0.3, 0.004, sh(0.15, 0.9, 0.5))
                }),
            );
            add(
                s,
                "Rumble",
                0.0,
                0.2,
                noise(Low, 400.0, 400.0, 0.1, sh(0.15, 0.9, 0.5)),
            );
            s.space = 0.3;
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                65.0,
                35.0,
                130.0,
            ));
        },
    },
    Preset {
        category: Creatures,
        name: "Monster roar",
        description: "A huge creature roaring",
        build: |s| {
            // Loosely a big cat (tigers ~160 Hz, Klemuk et al. 2011): lots of
            // breath and moving vowels, rough but never steady, so it stays
            // organic rather than mechanical.
            add(
                s,
                "Roar",
                0.0,
                1.0,
                Generator::Voice(VoiceParams {
                    pitch: c(&[(0.0, 110.0), (0.25, 150.0), (0.7, 130.0), (1.0, 95.0)]),
                    openness: c(&[(0.0, 0.4), (0.25, 1.0), (0.7, 0.8), (1.0, 0.5)]),
                    frontness: c(&[(0.0, 0.2), (0.5, 0.4), (1.0, 0.2)]),
                    quality: flat(0.3),
                    loudness: c(&[(0.0, 0.5), (0.25, 1.0), (0.6, 0.85), (1.0, 0.4)]),
                    size: 2.2,
                    roughness: 0.8,
                    subharmonics: 0.3,
                    breath: 0.5,
                    ..voice(sh(0.06, 0.8, 0.5))
                }),
            )
            .drive = 0.2;
            add(
                s,
                "Chest",
                0.0,
                0.25,
                thump(60.0, 40.0, 0.2, 0.0, 0.6, 400.0, sh(0.1, 0.5, 0.8)),
            );
            s.space = 0.4;
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                130.0,
                60.0,
                260.0,
            ));
        },
    },
    Preset {
        category: Creatures,
        name: "Small creature squeak",
        description: "A vole or lemming-sized critter",
        build: |s| {
            add(
                s,
                "Squeak",
                0.0,
                0.6,
                Generator::Voice(VoiceParams {
                    pitch: c(&[(0.0, 1400.0), (0.4, 1900.0), (1.0, 1600.0)]),
                    openness: flat(0.2),
                    frontness: flat(1.0),
                    quality: flat(0.3),
                    size: 0.3,
                    roughness: 0.2,
                    ..voice(sh(0.005, 0.05, 0.05))
                }),
            )
            .jitter
            .pitch = 3.0;
            s.max_voices = 3;
        },
    },
    Preset {
        category: Creatures,
        name: "Mouse squeak",
        description: "A house mouse (audible squeaks near 3.8 kHz)",
        build: |s| {
            add(
                s,
                "Squeak",
                0.0,
                0.6,
                Generator::Voice(VoiceParams {
                    pitch: c(&[(0.0, 3600.0), (0.4, 4000.0), (1.0, 3700.0)]),
                    openness: flat(0.2),
                    frontness: flat(1.0),
                    quality: flat(0.3),
                    size: 0.25,
                    roughness: 0.2,
                    ..voice(sh(0.005, 0.05, 0.05))
                }),
            )
            .jitter
            .pitch = 2.0;
            s.max_voices = 3;
        },
    },
    Preset {
        category: Creatures,
        name: "Zombie moan",
        description: "Braaains",
        build: |s| {
            add(
                s,
                "Moan",
                0.0,
                0.8,
                Generator::Voice(VoiceParams {
                    pitch: c(&[(0.0, 100.0), (0.3, 115.0), (0.6, 95.0), (1.0, 80.0)]),
                    openness: c(&[(0.0, 0.4), (0.5, 0.6), (1.0, 0.1)]),
                    frontness: c(&[(0.0, 0.2), (1.0, 0.0)]),
                    quality: flat(0.6),
                    loudness: c(&[(0.0, 0.5), (0.3, 1.0), (0.7, 0.8), (1.0, 0.4)]),
                    size: 1.3,
                    roughness: 0.6,
                    subharmonics: 0.3,
                    breath: 0.3,
                    vibrato: 0.6,
                    vibrato_rate: 3.0,
                    ..voice(sh(0.2, 1.1, 0.8))
                }),
            );
            s.space = 0.3;
            s.max_voices = 2;
            s.controls.push(Control::new(
                "Pitch",
                ControlTarget::Pitch,
                "Hz",
                100.0,
                60.0,
                200.0,
            ));
        },
    },
    Preset {
        category: Creatures,
        name: "Insect buzz",
        description: "A fly or a bee, until stopped",
        build: |s| {
            add(
                s,
                "Wings",
                0.0,
                0.5,
                Generator::Tone(ToneParams {
                    vibrato: 0.3,
                    vibrato_rate: 11.0,
                    fm: 0.4,
                    fm_ratio: 1.0,
                    cutoff: 3000.0,
                    ..tone(Wave::Saw, 350.0, 350.0, 0.1, sh(0.05, 1.0, 0.2))
                }),
            );
            add(
                s,
                "Flutter",
                0.0,
                0.2,
                Generator::Pulses(PulseParams {
                    irregular: 0.05,
                    ..pulses(350.0, 1500.0, 0.3, 0.001, sh(0.05, 1.0, 0.2))
                }),
            );
            s.looping = Looping::Sustain;
            s.max_voices = 2;
            s.controls.push(Control::new(
                "Wing beat",
                ControlTarget::Pitch,
                "Hz",
                350.0,
                150.0,
                800.0,
            ));
        },
    },
    Preset {
        category: Feedback,
        name: "Countdown beep",
        description: "3… 2… 1…",
        build: |s| {
            add(
                s,
                "Beep",
                0.0,
                0.35,
                Generator::Tone(ToneParams {
                    cutoff: 3000.0,
                    ..tone(Wave::Square, 880.0, 880.0, 0.01, sh(0.002, 0.08, 0.05))
                }),
            );
            add(
                s,
                "Shine",
                0.0,
                0.1,
                Generator::Tone(tone(
                    Wave::Sine,
                    1760.0,
                    1760.0,
                    0.01,
                    sh(0.002, 0.08, 0.05),
                )),
            );
            s.variation = 0.0;
        },
    },
    Preset {
        category: Feedback,
        name: "Go!",
        description: "The last, higher beep of a countdown",
        build: |s| {
            add(
                s,
                "Beep",
                0.0,
                0.35,
                Generator::Tone(ToneParams {
                    cutoff: 4000.0,
                    ..tone(Wave::Square, 1760.0, 1760.0, 0.01, sh(0.002, 0.3, 0.2))
                }),
            );
            add(
                s,
                "Shine",
                0.0,
                0.1,
                Generator::Tone(tone(Wave::Sine, 3520.0, 3520.0, 0.01, sh(0.002, 0.3, 0.2))),
            );
            s.variation = 0.0;
        },
    },
    Preset {
        category: Feedback,
        name: "Level up",
        description: "A bright rising arpeggio",
        build: |s| {
            for (i, (t, f)) in [(0.0, 523.0), (0.08, 659.0), (0.16, 784.0), (0.24, 1047.0)]
                .into_iter()
                .enumerate()
            {
                let hold = if i == 3 { 0.25 } else { 0.06 };
                add(
                    s,
                    &format!("Note {}", i + 1),
                    t,
                    0.3,
                    Generator::Tone(ToneParams {
                        cutoff: 5000.0,
                        ..tone(Wave::Square, f, f, 0.01, sh(0.002, hold, 0.25))
                    }),
                );
            }
            add(
                s,
                "Sparkle",
                0.24,
                0.2,
                Generator::Modal(ModalParams {
                    hits: 6,
                    spread: 0.4,
                    scatter: 5.0,
                    ..modal(Material::Glass, 2093.0, 0.6, 0.3, 0.6)
                }),
            );
            s.variation = 0.0;
            s.space = 0.3;
        },
    },
    Preset {
        category: Feedback,
        name: "Game over",
        description: "Four sad, falling notes",
        build: |s| {
            for (i, (t, f)) in [(0.0, 392.0), (0.25, 370.0), (0.5, 349.0), (0.75, 330.0)]
                .into_iter()
                .enumerate()
            {
                let last = i == 3;
                add(
                    s,
                    &format!("Note {}", i + 1),
                    t,
                    0.3,
                    Generator::Tone(ToneParams {
                        cutoff: 3000.0,
                        vibrato: if last { 0.3 } else { 0.0 },
                        vibrato_rate: 6.0,
                        ..tone(
                            Wave::Square,
                            f,
                            f,
                            0.01,
                            sh(
                                0.002,
                                if last { 0.5 } else { 0.18 },
                                if last { 0.4 } else { 0.05 },
                            ),
                        )
                    }),
                );
            }
            s.variation = 0.0;
        },
    },
    Preset {
        category: Feedback,
        name: "Achievement",
        description: "A triumphant ding",
        build: |s| {
            add(
                s,
                "Notes",
                0.0,
                0.35,
                Generator::Tone(ToneParams {
                    jump: 5.0,
                    jump_time: 0.1,
                    ..tone(Wave::Triangle, 784.0, 784.0, 0.01, sh(0.002, 0.3, 0.4))
                }),
            );
            add(
                s,
                "Octave",
                0.0,
                0.1,
                Generator::Tone(ToneParams {
                    jump: 5.0,
                    jump_time: 0.1,
                    cutoff: 5000.0,
                    ..tone(Wave::Square, 1568.0, 1568.0, 0.01, sh(0.002, 0.3, 0.4))
                }),
            );
            add(
                s,
                "Bell",
                0.1,
                0.3,
                Generator::Modal(modal(Material::Bell, 2093.0, 1.5, 0.3, 0.5)),
            );
            s.variation = 0.0;
            s.space = 0.4;
        },
    },
    Preset {
        category: Feedback,
        name: "Heartbeat",
        description: "Lub-dub, until stopped",
        build: |s| {
            add(
                s,
                "Lub",
                0.0,
                1.0,
                thump(60.0, 40.0, 0.03, 0.0, 0.2, 300.0, sh(0.005, 0.01, 0.12)),
            );
            add(
                s,
                "Dub",
                0.2,
                0.7,
                thump(55.0, 38.0, 0.03, 0.0, 0.2, 300.0, sh(0.005, 0.01, 0.12)),
            );
            s.looping = Looping::Repeat { every: 0.85 };
            s.variation = 0.3;
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Heart rate",
                ControlTarget::Rate,
                "bpm",
                70.6,
                40.0,
                200.0,
            ));
        },
    },
    Preset {
        category: Feedback,
        name: "Low health warning",
        description: "A beep that repeats until stopped",
        build: |s| {
            add(
                s,
                "Beep",
                0.0,
                0.3,
                Generator::Tone(ToneParams {
                    cutoff: 2000.0,
                    ..tone(Wave::Square, 440.0, 420.0, 0.1, sh(0.002, 0.12, 0.06))
                }),
            );
            s.looping = Looping::Repeat { every: 0.6 };
            s.variation = 0.0;
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Speed",
                ControlTarget::Rate,
                "×",
                1.0,
                0.25,
                4.0,
            ));
        },
    },
    Preset {
        category: Feedback,
        name: "Dialogue blips",
        description: "Text appearing letter by letter, until stopped",
        build: |s| {
            add(
                s,
                "Blip",
                0.0,
                0.5,
                Generator::Tone(ToneParams {
                    crush: 0.3,
                    ..tone(Wave::Square, 900.0, 900.0, 0.01, sh(0.001, 0.01, 0.02))
                }),
            )
            .jitter
            .pitch = 4.0;
            s.looping = Looping::Repeat { every: 0.07 };
            s.max_voices = 1;
            s.controls.push(Control::new(
                "Text speed",
                ControlTarget::Rate,
                "letters/s",
                14.3,
                5.0,
                40.0,
            ));
        },
    },
    Preset {
        category: Feedback,
        name: "Notification",
        description: "Ding-ding: something happened",
        build: |s| {
            add(
                s,
                "Ding",
                0.0,
                0.5,
                Generator::Modal(modal(Material::Bell, 1320.0, 1.0, 0.4, 0.4)),
            );
            add(
                s,
                "Ding 2",
                0.12,
                0.5,
                Generator::Modal(modal(Material::Bell, 1760.0, 1.0, 0.4, 0.4)),
            );
            s.variation = 0.1;
            s.space = 0.3;
        },
    },
    Preset {
        category: Feedback,
        name: "Pickup item",
        description: "Picking up an object",
        build: |s| {
            add(
                s,
                "Rise",
                0.0,
                0.5,
                Generator::Tone(tone(
                    Wave::Triangle,
                    600.0,
                    1200.0,
                    0.06,
                    sh(0.001, 0.04, 0.1),
                )),
            );
            add(
                s,
                "Tick",
                0.0,
                0.2,
                noise(Band, 3000.0, 3000.0, 0.3, sh(0.001, 0.005, 0.02)),
            );
            s.max_voices = 4;
        },
    },
];

/// Presets are data; keep them from drifting out of sensible ranges.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sfx::{GeneratorKind, Jitter};

    #[test]
    fn presets_are_valid() {
        let mut names = std::collections::HashSet::new();
        for p in PRESETS {
            assert!(names.insert(p.name), "duplicate preset {}", p.name);
            let s = p.sound();
            assert_eq!(s.name, p.name);
            assert!(!s.layers.is_empty(), "{} has no layers", p.name);
            assert!(
                s.length() > 0.01 && s.length() < 12.0,
                "{}: {}s",
                p.name,
                s.length()
            );
            assert!(s.max_voices >= 1);
            for l in &s.layers {
                assert!(
                    l.start >= 0.0 && l.gain > 0.0 && l.gain <= 1.5,
                    "{}",
                    p.name
                );
                if let Generator::Modal(m) = l.generator {
                    assert!(m.hits <= crate::sfx::MAX_HITS);
                }
            }
            assert!(find(p.name).is_some());
        }
        assert!(PRESETS.len() >= 25);
    }

    #[test]
    fn every_category_and_generator_is_used() {
        for cat in SfxCategory::ALL {
            assert!(
                PRESETS.iter().any(|p| p.category == cat),
                "{cat:?} is empty"
            );
        }
        for kind in GeneratorKind::ALL {
            let used = PRESETS
                .iter()
                .flat_map(|p| p.sound().layers)
                .any(|l| l.generator.kind() == kind);
            assert!(used, "{kind:?} unused");
        }
    }

    #[test]
    fn default_jitter_is_subtle() {
        assert_eq!(Jitter::default(), Jitter::SUBTLE);
    }
}
