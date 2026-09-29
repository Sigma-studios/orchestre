//! Modular patches: an instrument built from modules wired together with
//! cables, for expert users. Every note plays its own copy of the patch.
//!
//! Cables carry numbers, sample by sample; the input they plug into gives
//! them a meaning (see [`Module::inputs`]), and the cable's `amount` scales
//! them: an envelope into a filter's "cutoff" input with amount 3 opens the
//! filter by up to 3 octaves.

use serde::{Deserialize, Serialize};

use crate::instrument::{Adsr, FilterType, LfoWave, SynthParams, Wave};

pub type NodeId = u32;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Module {
    /// The note being played.
    Keyboard,
    Osc {
        wave: Wave,
        /// Pitch offset from the note, in semitones.
        semitones: f32,
        /// Cents.
        detune: f32,
        /// Pulse width of square and pulse waves.
        width: f32,
    },
    Noise,
    Lfo {
        wave: LfoWave,
        /// Hz, when not synced.
        rate: f32,
        /// Cycle length in beats; 0 = free-running at `rate`.
        beats: f32,
    },
    Envelope {
        adsr: Adsr,
    },
    Filter {
        kind: FilterType,
        cutoff: f32,
        resonance: f32,
    },
    /// Volume control: output = input × (level + gain inputs).
    Amp {
        level: f32,
    },
    Mixer {
        levels: [f32; 4],
    },
    /// Multiplies its two inputs (ring modulation).
    Ring,
    Drive {
        amount: f32,
    },
    /// Where the voice's sound goes out. A patch has exactly one.
    Output,
}

/// What an input's numbers mean.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputKind {
    /// Sound (or any signal) to process.
    Audio,
    /// Semitones.
    Pitch,
    /// Phase modulation depth (FM).
    Fm,
    /// Octaves.
    Cutoff,
    /// Added to a 0..1 setting.
    Amount,
    /// Multiplies (volume).
    Gain,
}

impl InputKind {
    /// Amount a new cable starts with.
    pub fn default_amount(self) -> f32 {
        match self {
            InputKind::Pitch => 1.0,
            InputKind::Cutoff => 3.0,
            InputKind::Amount => 0.3,
            _ => 1.0,
        }
    }
}

/// Kinds of modules, for the "add module" menu.
pub const MODULE_KINDS: [(&str, &str); 10] = [
    ("Oscillator", "A tone: sine, triangle, saw, square or pulse"),
    ("Noise", "Hiss, for breath, wind and drums"),
    ("Filter", "Makes the sound darker or brighter"),
    ("Envelope", "Rises and falls with each note (ADSR)"),
    ("LFO", "A slow wobble to move other settings"),
    ("Amp", "Volume, controlled by an envelope or LFO"),
    ("Mixer", "Adds up to four signals"),
    ("Ring mod", "Multiplies two signals: metallic tones"),
    ("Drive", "Distortion"),
    ("Keyboard", "The note played: gate, velocity, key"),
];

impl Module {
    /// A new module of the kind named in [`MODULE_KINDS`].
    pub fn new(kind: &str) -> Option<Module> {
        Some(match kind {
            "Oscillator" => Module::Osc {
                wave: Wave::Saw,
                semitones: 0.0,
                detune: 0.0,
                width: 0.5,
            },
            "Noise" => Module::Noise,
            "Filter" => Module::Filter {
                kind: FilterType::Ladder,
                cutoff: 1500.0,
                resonance: 0.2,
            },
            "Envelope" => Module::Envelope {
                adsr: Adsr::new(0.005, 0.3, 0.6, 0.3),
            },
            "LFO" => Module::Lfo {
                wave: LfoWave::Sine,
                rate: 4.0,
                beats: 0.0,
            },
            "Amp" => Module::Amp { level: 1.0 },
            "Mixer" => Module::Mixer { levels: [1.0; 4] },
            "Ring mod" => Module::Ring,
            "Drive" => Module::Drive { amount: 0.5 },
            "Keyboard" => Module::Keyboard,
            _ => return None,
        })
    }

    pub fn label(&self) -> &'static str {
        match self {
            Module::Keyboard => "Keyboard",
            Module::Osc { .. } => "Oscillator",
            Module::Noise => "Noise",
            Module::Lfo { .. } => "LFO",
            Module::Envelope { .. } => "Envelope",
            Module::Filter { .. } => "Filter",
            Module::Amp { .. } => "Amp",
            Module::Mixer { .. } => "Mixer",
            Module::Ring => "Ring mod",
            Module::Drive { .. } => "Drive",
            Module::Output => "Output",
        }
    }

    pub fn inputs(&self) -> &'static [(&'static str, InputKind)] {
        use InputKind::*;
        match self {
            Module::Keyboard | Module::Noise | Module::Envelope { .. } => &[],
            Module::Osc { .. } => &[("pitch", Pitch), ("fm", Fm), ("width", Amount)],
            Module::Lfo { .. } => &[("rate", Pitch)],
            Module::Filter { .. } => &[("in", Audio), ("cutoff", Cutoff), ("res", Amount)],
            Module::Amp { .. } => &[("in", Audio), ("gain", Gain)],
            Module::Mixer { .. } => &[("1", Audio), ("2", Audio), ("3", Audio), ("4", Audio)],
            Module::Ring => &[("a", Audio), ("b", Audio)],
            Module::Drive { .. } => &[("in", Audio)],
            Module::Output => &[("in", Audio)],
        }
    }

    pub fn outputs(&self) -> &'static [&'static str] {
        match self {
            Module::Keyboard => &["gate", "velocity", "key"],
            Module::Output => &[],
            _ => &["out"],
        }
    }

    /// The module's number settings, for macros: (name, value, range).
    pub fn params_mut(&mut self) -> Vec<(&'static str, &mut f32, std::ops::RangeInclusive<f32>)> {
        match self {
            Module::Osc {
                semitones,
                detune,
                width,
                ..
            } => vec![
                ("Pitch", semitones, -24.0..=24.0),
                ("Detune", detune, -50.0..=50.0),
                ("Width", width, 0.05..=0.95),
            ],
            Module::Lfo { rate, .. } => vec![("Speed", rate, 0.05..=20.0)],
            Module::Envelope { adsr } => vec![
                ("Attack", &mut adsr.attack, 0.001..=3.0),
                ("Decay", &mut adsr.decay, 0.001..=4.0),
                ("Sustain", &mut adsr.sustain, 0.0..=1.0),
                ("Release", &mut adsr.release, 0.001..=5.0),
            ],
            Module::Filter {
                cutoff, resonance, ..
            } => vec![
                ("Cutoff", cutoff, 30.0..=16000.0),
                ("Resonance", resonance, 0.0..=0.98),
            ],
            Module::Amp { level } => vec![("Level", level, 0.0..=1.0)],
            Module::Mixer { levels } => {
                let [a, b, c, d] = levels;
                vec![
                    ("Level 1", a, 0.0..=1.0),
                    ("Level 2", b, 0.0..=1.0),
                    ("Level 3", c, 0.0..=1.0),
                    ("Level 4", d, 0.0..=1.0),
                ]
            }
            Module::Drive { amount } => vec![("Amount", amount, 0.0..=1.0)],
            Module::Keyboard | Module::Noise | Module::Ring | Module::Output => Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    /// Position in the patch editor.
    pub pos: [f32; 2],
    pub module: Module,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cable {
    pub from: NodeId,
    pub from_port: u8,
    pub to: NodeId,
    pub to_port: u8,
    pub amount: f32,
}

/// A module setting shown as a simple knob outside the patch editor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Macro {
    pub name: String,
    pub node: NodeId,
    /// Index in [`Module::params_mut`].
    pub param: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Patch {
    pub nodes: Vec<Node>,
    pub cables: Vec<Cable>,
    pub macros: Vec<Macro>,
    /// One note at a time, gliding.
    pub mono: bool,
    pub glide: f32,
    pub chorus: f32,
    pub gain: f32,
}

impl Patch {
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    pub fn next_id(&self) -> NodeId {
        self.nodes.iter().map(|n| n.id).max().unwrap_or(0) + 1
    }

    pub fn add(&mut self, module: Module, pos: [f32; 2]) -> NodeId {
        let id = self.next_id();
        self.nodes.push(Node { id, pos, module });
        id
    }

    /// Remove a module, its cables and its macros.
    pub fn remove(&mut self, id: NodeId) {
        self.nodes.retain(|n| n.id != id);
        self.cables.retain(|c| c.from != id && c.to != id);
        self.macros.retain(|m| m.node != id);
    }

    /// Whether `from` feeds into `to`, directly or through other modules.
    pub fn reaches(&self, from: NodeId, to: NodeId) -> bool {
        let mut stack = vec![from];
        let mut seen = Vec::new();
        while let Some(n) = stack.pop() {
            if n == to {
                return true;
            }
            if seen.contains(&n) {
                continue;
            }
            seen.push(n);
            stack.extend(self.cables.iter().filter(|c| c.from == n).map(|c| c.to));
        }
        false
    }

    /// Plug a cable in, unless it would make a loop or already exists.
    pub fn connect(&mut self, from: NodeId, from_port: u8, to: NodeId, to_port: u8) -> bool {
        let exists = self.cables.iter().any(|c| {
            c.from == from && c.from_port == from_port && c.to == to && c.to_port == to_port
        });
        let Some(kind) = self
            .node(to)
            .and_then(|n| n.module.inputs().get(to_port as usize))
            .map(|i| i.1)
        else {
            return false;
        };
        if exists || from == to || self.reaches(to, from) {
            return false;
        }
        self.cables.push(Cable {
            from,
            from_port,
            to,
            to_port,
            amount: kind.default_amount(),
        });
        true
    }

    /// Current value of a macro, 0..1.
    pub fn macro_value(&mut self, m: &Macro) -> Option<f32> {
        let node = self.node_mut(m.node)?;
        let mut params = node.module.params_mut();
        let (_, v, range) = params.get_mut(m.param as usize)?;
        let (lo, hi) = (*range.start(), *range.end());
        let t = if is_log(lo, hi) {
            (v.max(lo) / lo).ln() / (hi / lo).ln()
        } else {
            (**v - lo) / (hi - lo)
        };
        Some(t.clamp(0.0, 1.0))
    }

    pub fn set_macro(&mut self, m: &Macro, value: f32) {
        let Some(node) = self.node_mut(m.node) else {
            return;
        };
        let mut params = node.module.params_mut();
        if let Some((_, v, range)) = params.get_mut(m.param as usize) {
            let (lo, hi) = (*range.start(), *range.end());
            let t = value.clamp(0.0, 1.0);
            **v = if is_log(lo, hi) {
                lo * (hi / lo).powf(t)
            } else {
                lo + (hi - lo) * t
            };
        }
    }

    /// Longest envelope release, for how long notes ring on.
    pub fn tail_seconds(&self) -> f32 {
        self.nodes
            .iter()
            .filter_map(|n| match n.module {
                Module::Envelope { adsr } => Some(adsr.release),
                _ => None,
            })
            .fold(0.05, f32::max)
    }

    /// The classic synth's settings, rebuilt as a patch (a starting point
    /// for editing: unison, sync and a few extras are left out).
    pub fn from_synth(p: &SynthParams) -> Patch {
        let mut patch = Patch {
            nodes: Vec::new(),
            cables: Vec::new(),
            macros: Vec::new(),
            mono: p.mono,
            glide: p.glide,
            chorus: p.chorus,
            gain: p.gain,
        };
        let osc = |wave: Wave, semitones: f32, detune: f32| Module::Osc {
            wave,
            semitones,
            detune,
            width: crate::patch::default_width(wave),
        };
        let oct = p.octave as f32 * 12.0;
        let key = patch.add(Module::Keyboard, [20.0, 220.0]);
        let o1 = patch.add(osc(p.osc1, oct, 0.0), [250.0, 20.0]);
        let o2 = patch.add(
            osc(p.osc2, oct + p.osc2_semitones as f32, p.osc2_detune),
            [250.0, 210.0],
        );
        let mix = patch.add(
            Module::Mixer {
                levels: [1.0 - p.osc_mix, p.osc_mix, p.sub, p.noise * 0.5],
            },
            [490.0, 40.0],
        );
        let filter = patch.add(
            Module::Filter {
                kind: p.filter_type,
                cutoff: p.cutoff,
                resonance: p.resonance,
            },
            [730.0, 40.0],
        );
        let fenv = patch.add(
            Module::Envelope {
                adsr: p.filter_adsr,
            },
            [490.0, 290.0],
        );
        let amp = patch.add(Module::Amp { level: 0.0 }, [970.0, 40.0]);
        let aenv = patch.add(Module::Envelope { adsr: p.amp }, [730.0, 290.0]);
        let out = patch.add(Module::Output, [970.0, 200.0]);
        patch.connect(o1, 0, mix, 0);
        patch.connect(o2, 0, mix, 1);
        if p.sub > 0.0 {
            let sub = patch.add(osc(Wave::Square, oct - 12.0, 0.0), [250.0, 400.0]);
            patch.connect(sub, 0, mix, 2);
        }
        if p.noise > 0.0 {
            let noise = patch.add(Module::Noise, [20.0, 360.0]);
            patch.connect(noise, 0, mix, 3);
        }
        if p.fm > 0.0 {
            patch.connect(o2, 0, o1, 1);
            set_amount(&mut patch, o2, o1, p.fm);
        }
        patch.connect(mix, 0, filter, 0);
        patch.connect(fenv, 0, filter, 1);
        set_amount(&mut patch, fenv, filter, p.filter_env);
        patch.connect(key, 2, filter, 1);
        set_amount(&mut patch, key, filter, 0.4);
        patch.connect(filter, 0, amp, 0);
        patch.connect(aenv, 0, amp, 1);
        patch.connect(amp, 0, out, 0);
        if p.lfo_pitch > 0.0 || p.lfo_cutoff > 0.0 {
            let lfo = patch.add(
                Module::Lfo {
                    wave: p.lfo_wave,
                    rate: p.lfo_rate,
                    beats: p.lfo_beats,
                },
                [20.0, 20.0],
            );
            if p.lfo_pitch > 0.0 {
                for o in [o1, o2] {
                    patch.connect(lfo, 0, o, 0);
                    set_amount(&mut patch, lfo, o, p.lfo_pitch);
                }
            }
            if p.lfo_cutoff > 0.0 {
                patch.connect(lfo, 0, filter, 1);
                set_amount(&mut patch, lfo, filter, p.lfo_cutoff);
            }
        }
        patch.macros = vec![
            Macro {
                name: "Brightness".into(),
                node: filter,
                param: 0,
            },
            Macro {
                name: "Resonance".into(),
                node: filter,
                param: 1,
            },
            Macro {
                name: "Fade out".into(),
                node: aenv,
                param: 3,
            },
        ];
        patch
    }
}

impl Default for Patch {
    fn default() -> Self {
        Patch::from_synth(&SynthParams::default())
    }
}

fn set_amount(patch: &mut Patch, from: NodeId, to: NodeId, amount: f32) {
    if let Some(c) = patch
        .cables
        .iter_mut()
        .rev()
        .find(|c| c.from == from && c.to == to)
    {
        c.amount = amount;
    }
}

/// Ranges spanning orders of magnitude (frequencies, times) feel right on
/// a logarithmic knob.
fn is_log(lo: f32, hi: f32) -> bool {
    lo > 0.0 && hi / lo >= 100.0
}

/// Pulse width of a wave when not modulated.
pub fn default_width(wave: Wave) -> f32 {
    match wave {
        Wave::Pulse => 0.25,
        _ => 0.5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cables_never_make_loops() {
        let mut p = Patch::default();
        let filter = p
            .nodes
            .iter()
            .find(|n| matches!(n.module, Module::Filter { .. }))
            .unwrap()
            .id;
        let osc = p
            .nodes
            .iter()
            .find(|n| matches!(n.module, Module::Osc { .. }))
            .unwrap()
            .id;
        // The oscillator already feeds the filter: the reverse is a loop.
        assert!(!p.connect(filter, 0, osc, 1));
        let lfo = p.add(Module::new("LFO").unwrap(), [0.0, 0.0]);
        assert!(p.connect(lfo, 0, filter, 1));
        assert!(!p.connect(lfo, 0, filter, 1), "duplicate");
    }

    #[test]
    fn macros_map_to_module_settings() {
        let mut p = Patch::default();
        let m = p.macros[0].clone();
        p.set_macro(&m, 1.0);
        assert_eq!(p.macro_value(&m), Some(1.0));
        let v = match p.node(m.node).unwrap().module {
            Module::Filter { cutoff, .. } => cutoff,
            _ => unreachable!(),
        };
        assert!((v - 16000.0).abs() < 1.0);
        // Cutoffs are on a log scale: the middle is a few hundred Hz.
        p.set_macro(&m, 0.5);
        assert!((p.macro_value(&m).unwrap() - 0.5).abs() < 1e-4);
    }
}
