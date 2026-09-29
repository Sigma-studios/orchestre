//! Plays modular patches. A patch is compiled into a program: its modules
//! in signal order (sources before what they feed), each output given a
//! block buffer. Every voice runs the program with its own module states
//! (oscillator phases, envelopes, filters); the buffers are shared scratch.

use orchestre_core::patch::{Module, Patch};

use crate::env::Env;
use crate::filter::MultiFilter;
use crate::fx::Ensemble;
use crate::osc::Osc;
use crate::synth::lfo_value;
use crate::util::{Rng, midi_to_hz, soft_clip};

const MAX_VOICES: usize = 12;
const MONO_STACK: usize = 8;
/// Pitch and filter settings are updated every this many samples.
const SUB: usize = 8;
const MAX_BLOCK: usize = 64;
/// Seconds of fade-out when a voice ends.
const FADE: f32 = 0.01;

struct CNode {
    module: Module,
    /// First output buffer.
    out: usize,
    /// Per input port: (buffer, amount) of every cable plugged in.
    inputs: Vec<Vec<(usize, f32)>>,
}

#[derive(Default)]
struct Program {
    nodes: Vec<CNode>,
    buffers: usize,
    has_env: bool,
}

/// Node ids and kinds, and cable ends: if these match, a new version of
/// the patch can be played without rebuilding the voices.
type Shape = (Vec<(u32, &'static str)>, Vec<(u32, u8, u32, u8)>);

fn shape(p: &Patch) -> Shape {
    (
        p.nodes.iter().map(|n| (n.id, n.module.label())).collect(),
        p.cables
            .iter()
            .map(|c| (c.from, c.from_port, c.to, c.to_port))
            .collect(),
    )
}

fn compile(p: &Patch) -> Program {
    // Kahn's algorithm; modules caught in a loop (the editor prevents
    // them) are left out.
    let n = p.nodes.len();
    let index = |id: u32| p.nodes.iter().position(|nd| nd.id == id);
    let valid: Vec<_> = p
        .cables
        .iter()
        .filter_map(|c| {
            let (a, b) = (index(c.from)?, index(c.to)?);
            let ok = (c.from_port as usize) < p.nodes[a].module.outputs().len()
                && (c.to_port as usize) < p.nodes[b].module.inputs().len();
            ok.then_some((a, b, c))
        })
        .collect();
    let mut indeg = vec![0usize; n];
    for &(_, b, _) in &valid {
        indeg[b] += 1;
    }
    let mut ready: Vec<usize> = (0..n).filter(|&i| indeg[i] == 0).collect();
    let mut order = Vec::with_capacity(n);
    while let Some(i) = ready.pop() {
        order.push(i);
        for &(a, b, _) in &valid {
            if a == i {
                indeg[b] -= 1;
                if indeg[b] == 0 {
                    ready.push(b);
                }
            }
        }
    }
    let mut base = vec![None; n];
    let mut buffers = 0;
    for &i in &order {
        base[i] = Some(buffers);
        buffers += p.nodes[i].module.outputs().len();
    }
    let nodes = order
        .iter()
        .map(|&i| {
            let module = p.nodes[i].module;
            let inputs = (0..module.inputs().len())
                .map(|port| {
                    valid
                        .iter()
                        .filter(|(_, b, c)| *b == i && c.to_port as usize == port)
                        .filter_map(|(a, _, c)| Some((base[*a]? + c.from_port as usize, c.amount)))
                        .collect()
                })
                .collect();
            CNode {
                module,
                out: base[i].unwrap_or(0),
                inputs,
            }
        })
        .collect::<Vec<_>>();
    let has_env = nodes
        .iter()
        .any(|c| matches!(c.module, Module::Envelope { .. }));
    Program {
        nodes,
        buffers,
        has_env,
    }
}

#[derive(Clone, Copy)]
enum State {
    None,
    Osc(Osc),
    Noise(Rng),
    Lfo { phase: f32, held: f32, rng: Rng },
    Env(Env),
    Filter(MultiFilter),
}

#[derive(Clone, Default)]
struct Voice {
    active: bool,
    note: u8,
    pitch: f32,
    target: f32,
    vel: f32,
    off_at: f64,
    age: u64,
    gate: bool,
    /// Counts down once the voice is ending (seconds of fade left).
    fade: Option<f32>,
    states: Vec<State>,
}

pub struct PatchEngine {
    pub params: Patch,
    sr: f32,
    bpm: f32,
    prog: Program,
    shape: Shape,
    voices: Vec<Voice>,
    counter: u64,
    stack: [(u8, f32, f64); MONO_STACK],
    stack_len: usize,
    bufs: Vec<[f32; MAX_BLOCK]>,
    chorus: Ensemble,
}

impl PatchEngine {
    pub fn new(params: Patch, sr: f32) -> Self {
        let mut e = PatchEngine {
            prog: Program::default(),
            shape: Default::default(),
            params,
            sr,
            bpm: 120.0,
            voices: vec![Voice::default(); MAX_VOICES],
            counter: 0,
            stack: [(0, 0.0, 0.0); MONO_STACK],
            stack_len: 0,
            bufs: Vec::new(),
            chorus: Ensemble::new(sr),
        };
        e.rebuild();
        e
    }

    fn rebuild(&mut self) {
        self.prog = compile(&self.params);
        self.shape = shape(&self.params);
        self.bufs = vec![[0.0; MAX_BLOCK]; self.prog.buffers];
        for v in &mut self.voices {
            v.active = false;
            v.states = vec![State::None; self.prog.nodes.len()];
        }
        self.stack_len = 0;
    }

    pub fn set_tempo(&mut self, bpm: f32) {
        self.bpm = bpm.max(1.0);
    }

    pub fn set_params(&mut self, params: &Patch) {
        let mono_changed = params.mono != self.params.mono;
        self.params = params.clone();
        if shape(params) != self.shape {
            self.rebuild();
            return;
        }
        if mono_changed {
            self.all_off();
        }
        // Same wiring: take the new settings and cable amounts in place.
        let fresh = compile(params);
        for (old, new) in self.prog.nodes.iter_mut().zip(fresh.nodes) {
            old.module = new.module;
            old.inputs = new.inputs;
        }
        for v in self.voices.iter_mut().filter(|v| v.active) {
            for (s, node) in v.states.iter_mut().zip(&self.prog.nodes) {
                if let (State::Env(env), Module::Envelope { adsr }) = (s, node.module) {
                    env.set(&adsr, self.sr);
                }
            }
        }
    }

    fn start(&mut self, idx: usize, pitch: u8, vel: f32, off_at: f64) {
        self.counter += 1;
        let seed = (self.counter as u32).wrapping_mul(2_654_435_761) ^ pitch as u32;
        let mut rng = Rng::new(seed);
        let sr = self.sr;
        let v = &mut self.voices[idx];
        v.active = true;
        v.note = pitch;
        v.pitch = pitch as f32;
        v.target = pitch as f32;
        v.vel = vel;
        v.off_at = off_at;
        v.age = self.counter;
        v.gate = true;
        v.fade = None;
        for (s, node) in v.states.iter_mut().zip(&self.prog.nodes) {
            *s = match node.module {
                Module::Osc { .. } => State::Osc(Osc {
                    phase: rng.noise() * 0.5 + 0.5,
                }),
                Module::Noise => State::Noise(Rng::new(rng.noise().to_bits())),
                Module::Lfo { beats, .. } => State::Lfo {
                    // Synced LFOs restart with the note, others anywhere.
                    phase: if beats > 0.0 {
                        0.0
                    } else {
                        rng.noise() * 0.5 + 0.5
                    },
                    held: rng.noise(),
                    rng: Rng::new(rng.noise().to_bits()),
                },
                Module::Envelope { adsr } => {
                    let mut env = Env::default();
                    env.set(&adsr, sr);
                    env.gate_on();
                    State::Env(env)
                }
                Module::Filter { .. } => State::Filter(MultiFilter::default()),
                _ => State::None,
            };
        }
    }

    fn gate_off(v: &mut Voice) {
        v.gate = false;
        for s in &mut v.states {
            if let State::Env(env) = s {
                env.gate_off();
            }
        }
    }

    pub fn note_on(&mut self, pitch: u8, vel: f32, off_at: f64) {
        if self.params.mono {
            if self.stack_len == MONO_STACK {
                self.stack.copy_within(1.., 0);
                self.stack_len -= 1;
            }
            self.stack[self.stack_len] = (pitch, vel, off_at);
            self.stack_len += 1;
            let v = &mut self.voices[0];
            if v.active && v.gate {
                v.target = pitch as f32;
                v.note = pitch;
                v.off_at = off_at;
                v.vel = vel;
            } else {
                let from = v.active.then_some(v.pitch);
                self.start(0, pitch, vel, off_at);
                if let Some(from) = from {
                    self.voices[0].pitch = from;
                }
            }
            return;
        }
        let idx = self
            .voices
            .iter()
            .position(|v| !v.active)
            .or_else(|| {
                self.voices
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| (v.gate, v.age))
                    .map(|(i, _)| i)
            })
            .unwrap_or(0);
        self.start(idx, pitch, vel, off_at);
    }

    fn mono_remove(&mut self, pred: impl Fn(&(u8, f32, f64)) -> bool) {
        let before = self.stack_len;
        let mut j = 0;
        for i in 0..self.stack_len {
            if !pred(&self.stack[i]) {
                self.stack[j] = self.stack[i];
                j += 1;
            }
        }
        self.stack_len = j;
        if j == before {
            return;
        }
        let v = &mut self.voices[0];
        if j == 0 {
            Self::gate_off(v);
        } else {
            let (pitch, vel, off_at) = self.stack[j - 1];
            v.target = pitch as f32;
            v.note = pitch;
            v.vel = vel;
            v.off_at = off_at;
        }
    }

    pub fn release_due(&mut self, tick: f64) {
        if self.params.mono {
            self.mono_remove(|e| e.2 <= tick);
            return;
        }
        for v in self
            .voices
            .iter_mut()
            .filter(|v| v.active && v.gate && v.off_at <= tick)
        {
            Self::gate_off(v);
        }
    }

    pub fn live_off(&mut self, pitch: u8) {
        if self.params.mono {
            self.mono_remove(|e| e.0 == pitch && e.2.is_infinite());
            return;
        }
        for v in self
            .voices
            .iter_mut()
            .filter(|v| v.active && v.gate && v.note == pitch && v.off_at.is_infinite())
        {
            Self::gate_off(v);
        }
    }

    pub fn all_off(&mut self) {
        self.stack_len = 0;
        for v in self.voices.iter_mut().filter(|v| v.active) {
            Self::gate_off(v);
        }
    }

    pub fn render(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(MAX_BLOCK);
        for vi in 0..self.voices.len() {
            if self.voices[vi].active {
                self.render_voice(vi, &mut l[..n], &mut r[..n]);
            }
        }
        if self.params.chorus > 0.0 {
            self.chorus.process(l, r, self.params.chorus);
        }
    }

    fn render_voice(&mut self, vi: usize, l: &mut [f32], r: &mut [f32]) {
        let n = l.len();
        let sr = self.sr;
        let (mono, glide, bpm) = (self.params.mono, self.params.glide, self.bpm);
        let v = &mut self.voices[vi];
        if mono && glide > 0.0 {
            let k = (-(n as f32) / (glide * sr)).exp();
            v.pitch = v.target + (v.pitch - v.target) * k;
        } else {
            v.pitch = v.target;
        }
        let mut out = [0.0f32; MAX_BLOCK];
        let mut ins = [[0.0f32; MAX_BLOCK]; 4];
        for (ni, node) in self.prog.nodes.iter().enumerate() {
            for (k, srcs) in node.inputs.iter().enumerate().take(4) {
                let dst = &mut ins[k][..n];
                dst.fill(0.0);
                for &(b, amount) in srcs {
                    for (d, s) in dst.iter_mut().zip(&self.bufs[b][..n]) {
                        *d += s * amount;
                    }
                }
            }
            let o = node.out;
            match (node.module, &mut v.states[ni]) {
                (Module::Keyboard, _) => {
                    let gate = if v.gate { 1.0 } else { 0.0 };
                    let key = (v.pitch - 60.0) / 12.0;
                    self.bufs[o][..n].fill(gate);
                    self.bufs[o + 1][..n].fill(v.vel);
                    self.bufs[o + 2][..n].fill(key);
                }
                (
                    Module::Osc {
                        wave,
                        semitones,
                        detune,
                        width,
                    },
                    State::Osc(osc),
                ) => {
                    let buf = &mut self.bufs[o];
                    let mut i = 0;
                    while i < n {
                        let m = SUB.min(n - i);
                        let pitch = v.pitch + semitones + detune / 100.0 + ins[0][i];
                        let dt = midi_to_hz(pitch) / sr;
                        let w = (width + ins[2][i]).clamp(0.05, 0.95);
                        for j in i..i + m {
                            buf[j] = osc.next_pw(wave, dt, ins[1][j] * 0.25, w);
                        }
                        i += m;
                    }
                }
                (Module::Noise, State::Noise(rng)) => {
                    for x in &mut self.bufs[o][..n] {
                        *x = rng.noise();
                    }
                }
                (Module::Lfo { wave, rate, beats }, State::Lfo { phase, held, rng }) => {
                    let hz = if beats > 0.0 {
                        bpm / 60.0 / beats
                    } else {
                        rate
                    };
                    let hz = hz * (2.0f32).powf(ins[0][0] / 12.0);
                    let step = hz / sr;
                    for x in &mut self.bufs[o][..n] {
                        *x = lfo_value(wave, *phase, *held);
                        *phase += step;
                        if *phase >= 1.0 {
                            *phase -= 1.0;
                            *held = rng.noise();
                        }
                    }
                }
                (Module::Envelope { .. }, State::Env(env)) => {
                    for x in &mut self.bufs[o][..n] {
                        *x = env.next();
                    }
                }
                (
                    Module::Filter {
                        kind,
                        cutoff,
                        resonance,
                    },
                    State::Filter(f),
                ) => {
                    let buf = &mut self.bufs[o];
                    let mut i = 0;
                    while i < n {
                        let m = SUB.min(n - i);
                        let fc = cutoff * (2.0f32).powf(ins[1][i]);
                        let res = (resonance + ins[2][i]).clamp(0.0, 0.98);
                        f.set(kind, fc, res, sr);
                        for j in i..i + m {
                            buf[j] = f.process(ins[0][j]);
                        }
                        i += m;
                    }
                }
                (Module::Amp { level }, _) => {
                    let (a, g) = (&ins[0], &ins[1]);
                    for (j, x) in self.bufs[o][..n].iter_mut().enumerate() {
                        *x = a[j] * (level + g[j]).max(0.0);
                    }
                }
                (Module::Mixer { levels }, _) => {
                    for (j, x) in self.bufs[o][..n].iter_mut().enumerate() {
                        *x = (0..4).map(|k| ins[k][j] * levels[k]).sum();
                    }
                }
                (Module::Ring, _) => {
                    for (j, x) in self.bufs[o][..n].iter_mut().enumerate() {
                        *x = ins[0][j] * ins[1][j];
                    }
                }
                (Module::Drive { amount }, _) => {
                    let pre = 1.0 + 14.0 * amount * amount;
                    let makeup = 1.0 / (1.0 + 4.0 * amount);
                    for (j, x) in self.bufs[o][..n].iter_mut().enumerate() {
                        *x = soft_clip(ins[0][j] * pre) * makeup;
                    }
                }
                (Module::Output, _) => out[..n].copy_from_slice(&ins[0][..n]),
                // A module whose state doesn't match (never, after a rebuild).
                _ => {
                    for k in 0..node.module.outputs().len() {
                        self.bufs[o + k][..n].fill(0.0);
                    }
                }
            }
        }

        // The voice ends once released and its envelopes are done (or
        // right away without envelopes), with a short fade.
        let envs_done = v
            .states
            .iter()
            .all(|s| !matches!(s, State::Env(e) if !e.is_idle()));
        if !v.gate && v.fade.is_none() && (!self.prog.has_env || envs_done) {
            v.fade = Some(FADE);
        }
        let gain = self.params.gain * (0.25 + 0.75 * v.vel * v.vel) * 0.7;
        let step = 1.0 / (FADE * sr);
        let mut fade = v.fade.map(|f| f / FADE);
        for i in 0..n {
            let g = match &mut fade {
                Some(f) => {
                    *f = (*f - step).max(0.0);
                    *f
                }
                None => 1.0,
            };
            let s = out[i].clamp(-4.0, 4.0) * gain * g;
            l[i] += s;
            r[i] += s;
        }
        if let Some(f) = fade {
            v.fade = Some(f * FADE);
            if f <= 0.0 {
                v.active = false;
            }
        }
    }
}
