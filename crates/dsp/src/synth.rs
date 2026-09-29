use orchestre_core::{LfoWave, ModSource, ModTarget, SynthParams, Wave};

use crate::env::Env;
use crate::filter::MultiFilter;
use crate::fx::Ensemble;
use crate::osc::{Osc, SyncOsc, default_width};
use crate::util::{Rng, TAU, midi_to_hz, pan_gains, soft_clip};

pub const MAX_VOICES: usize = 16;
const MAX_UNISON: usize = 7;
const MONO_STACK: usize = 8;
/// Envelopes, modulation and the filter are updated every this many samples.
const SUB: usize = 8;

#[derive(Clone, Copy, Default)]
struct Voice {
    active: bool,
    /// Pitch the voice was triggered with (for live note-off).
    note: u8,
    /// Current (gliding) and target pitch, in semitones.
    pitch: f32,
    target: f32,
    vel: f32,
    /// 0..1: how much this note is accented (loud notes, see `accent`).
    accent: f32,
    /// Tick at which the note is released; infinite for live notes.
    off_at: f64,
    age: u64,
    osc1: [Osc; MAX_UNISON],
    osc2: [Osc; MAX_UNISON],
    sync2: [SyncOsc; MAX_UNISON],
    /// Last wave-2 sample of each unison voice (FM source when synced).
    last2: [f32; MAX_UNISON],
    osc3: Osc,
    sub: Osc,
    amp: Env,
    filt: Env,
    filter: [MultiFilter; 2],
    lfo_phase: f32,
    /// Current level of the random (sample & hold) LFO.
    lfo_held: f32,
    /// Random value per note (a modulation source).
    random: f32,
    rng: Rng,
    /// Remaining pitch bend in semitones (falls to 0).
    bend: f32,
}

/// Modulation amounts for one sub-block, in each target's units.
#[derive(Default)]
struct Mods {
    pitch: f32,
    pitch2: f32,
    cutoff: f32,
    resonance: f32,
    width: f32,
    fm: f32,
    mix: f32,
    level: f32,
    noise: f32,
}

pub(crate) fn lfo_value(wave: LfoWave, phase: f32, held: f32) -> f32 {
    match wave {
        LfoWave::Sine => (phase * TAU).sin(),
        LfoWave::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
        LfoWave::Square => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        LfoWave::SawDown => 1.0 - 2.0 * phase,
        LfoWave::SawUp => 2.0 * phase - 1.0,
        LfoWave::Random => held,
    }
}

impl Voice {
    #[allow(clippy::too_many_arguments)]
    fn start(
        &mut self,
        p: &SynthParams,
        sr: f32,
        pitch: u8,
        vel: f32,
        off_at: f64,
        age: u64,
        seed: u32,
    ) {
        self.active = true;
        self.note = pitch;
        self.pitch = pitch as f32;
        self.bend = p.bend;
        self.target = pitch as f32;
        self.vel = vel;
        self.accent = p.accent * ((vel - 0.85) / 0.15).clamp(0.0, 1.0);
        self.off_at = off_at;
        self.age = age;
        self.amp.level = 0.0;
        self.filt.level = 0.0;
        self.amp.set(&p.amp, sr);
        self.filt.set(&p.filter_adsr, sr);
        self.amp.gate_on();
        self.filt.gate_on();
        for f in &mut self.filter {
            f.reset();
        }
        // Random start phases keep unison stacks from sounding phasey.
        let mut rng = Rng::new(seed);
        for o in self.osc1.iter_mut().chain(self.osc2.iter_mut()) {
            o.phase = rng.noise() * 0.5 + 0.5;
        }
        for o in &mut self.sync2 {
            *o = SyncOsc::default();
        }
        self.last2 = [0.0; MAX_UNISON];
        self.osc3.phase = rng.noise() * 0.5 + 0.5;
        // Tempo-synced LFOs restart with each note so they line up with the beat.
        if p.lfo_beats > 0.0 {
            self.lfo_phase = 0.0;
        }
        self.lfo_held = rng.noise();
        self.random = rng.noise();
        self.rng = rng;
    }

    fn mods(&self, p: &SynthParams, lfo: f32) -> Mods {
        let mut m = Mods::default();
        for slot in &p.mods {
            let src = match slot.source {
                ModSource::None => continue,
                ModSource::Lfo => lfo,
                ModSource::FilterEnv => self.filt.level,
                ModSource::AmpEnv => self.amp.level,
                ModSource::Velocity => self.vel,
                ModSource::KeyTrack => (self.pitch - 60.0) / 12.0,
                ModSource::Random => self.random,
            };
            let v = src * slot.amount * slot.target.scale();
            match slot.target {
                ModTarget::None => {}
                ModTarget::Pitch => m.pitch += v,
                ModTarget::Osc2Pitch => m.pitch2 += v,
                ModTarget::Cutoff => m.cutoff += v,
                ModTarget::Resonance => m.resonance += v,
                ModTarget::PulseWidth => m.width += v,
                ModTarget::Fm => m.fm += v,
                ModTarget::OscMix => m.mix += v,
                ModTarget::Level => m.level += v,
                ModTarget::Noise => m.noise += v,
            }
        }
        m
    }

    /// `bpm` sets synced LFOs; `bright` shifts the cutoff, in octaves.
    fn render(
        &mut self,
        p: &SynthParams,
        sr: f32,
        bpm: f32,
        bright: f32,
        l: &mut [f32],
        r: &mut [f32],
    ) {
        let n = l.len();
        if p.mono && p.glide > 0.0 {
            let k = (-(n as f32) / (p.glide * sr)).exp();
            self.pitch = self.target + (self.pitch - self.target) * k;
        } else {
            self.pitch = self.target;
        }
        let bend = self.bend;
        self.bend = if p.bend_time > 0.0 {
            self.bend * (-(n as f32) / (p.bend_time * sr)).exp()
        } else {
            0.0
        };
        let lfo_rate = if p.lfo_beats > 0.0 {
            bpm / 60.0 / p.lfo_beats
        } else {
            p.lfo_rate
        };

        let uni = (p.unison as usize).clamp(1, MAX_UNISON);
        let mut ratio = [1.0f32; MAX_UNISON];
        let mut gl = [0.0f32; MAX_UNISON];
        let mut gr = [0.0f32; MAX_UNISON];
        // After Szabo's analysis of the JP-8000 supersaw: inner voices sit
        // closer to the centre than outer ones, and the centre voice is a
        // little louder than each side voice (his mix curves at half mix).
        let (center, side) = if uni >= 3 { (0.72, 0.5) } else { (1.0, 1.0) };
        let middle = (uni % 2 == 1).then_some(uni / 2);
        let power: f32 = (0..uni)
            .map(|i| {
                if Some(i) == middle {
                    center * center
                } else {
                    side * side
                }
            })
            .sum();
        let norm = 1.0 / power.sqrt();
        for i in 0..uni {
            let pos = if uni == 1 {
                0.0
            } else {
                i as f32 / (uni - 1) as f32 * 2.0 - 1.0
            };
            let spaced = pos.signum() * pos.abs().powf(1.5);
            ratio[i] = (2.0f32).powf(spaced * p.unison_spread * 0.5 / 1200.0);
            let level = if Some(i) == middle { center } else { side } * norm;
            let (a, b) = pan_gains(pos * p.width);
            gl[i] = a * level;
            gr[i] = b * level;
        }

        let key_track = (self.pitch - 60.0) / 12.0 * 0.4;
        let vel_bright = (self.vel - 0.7) * 1.2;
        let vel_gain = 0.25 + 0.75 * self.vel * self.vel;
        let accent = self.accent;
        let osc3_on = p.osc3_level > 0.0;

        let mut start = 0;
        while start < n {
            let m = SUB.min(n - start);
            let lfo = lfo_value(p.lfo_wave, self.lfo_phase, self.lfo_held);
            let next = self.lfo_phase + lfo_rate * m as f32 / sr;
            if next >= 1.0 {
                self.lfo_held = self.rng.noise();
            }
            self.lfo_phase = next.fract();
            let md = self.mods(p, lfo);
            let fenv = self.filt.level;
            for _ in 0..m {
                self.filt.next();
            }

            let pitch = self.pitch + bend + p.octave as f32 * 12.0 + lfo * p.lfo_pitch + md.pitch;
            let f1 = midi_to_hz(pitch);
            let f2 = midi_to_hz(
                pitch
                    + p.osc2_semitones as f32
                    + p.osc2_detune / 100.0
                    + p.env_pitch2 * fenv
                    + md.pitch2,
            );
            let dt3 = midi_to_hz(pitch + p.osc3_semitones as f32 + p.osc3_detune / 100.0) / sr;
            let dt_sub = f1 * 0.5 / sr;

            let octaves = p.filter_env * fenv * (1.0 + accent * 0.6)
                + accent * 1.2
                + lfo * p.lfo_cutoff
                + key_track
                + vel_bright
                + md.cutoff
                + bright;
            let cutoff = p.cutoff * (2.0f32).powf(octaves);
            let res = (p.resonance + md.resonance).clamp(0.0, 0.98);
            for f in &mut self.filter {
                f.set(p.filter_type, cutoff, res, sr);
            }

            let pw_mod = lfo * p.pwm * 0.4 + md.width;
            let w1 = (default_width(p.osc1) + pw_mod).clamp(0.05, 0.95);
            let w2 = (default_width(p.osc2) + pw_mod).clamp(0.05, 0.95);
            let w3 = default_width(p.osc3);
            let mix = (p.osc_mix + md.mix).clamp(0.0, 1.0);
            let fm = (p.fm + md.fm).max(0.0);
            let noise = (p.noise + md.noise).clamp(0.0, 1.0);
            let tremolo = 1.0 - p.lfo_amp * (0.5 - 0.5 * lfo);
            let gain = p.gain
                * vel_gain
                * 0.7
                * (1.0 + accent * 0.4)
                * tremolo
                * (1.0 + md.level).max(0.0);

            let mut dt1 = [0.0f32; MAX_UNISON];
            let mut dt2 = [0.0f32; MAX_UNISON];
            for u in 0..uni {
                dt1[u] = f1 * ratio[u] / sr;
                dt2[u] = f2 * ratio[u] / sr;
            }
            for i in start..start + m {
                let mut sl = 0.0;
                let mut sr_ = 0.0;
                for u in 0..uni {
                    let (o1, o2) = if p.sync {
                        let before = self.osc1[u].phase;
                        let o1 =
                            self.osc1[u].next_pw(p.osc1, dt1[u], self.last2[u] * fm * 0.25, w1);
                        let after = self.osc1[u].phase;
                        let reset = (after < before).then(|| after / dt1[u].max(1e-9));
                        let o2 = self.sync2[u].next(p.osc2, dt2[u], w2, reset);
                        self.last2[u] = o2;
                        (o1, o2)
                    } else {
                        let o2 = self.osc2[u].next_pw(p.osc2, dt2[u], 0.0, w2);
                        let o1 = self.osc1[u].next_pw(p.osc1, dt1[u], o2 * fm * 0.25, w1);
                        (o1, o2)
                    };
                    let mut s = o1 * (1.0 - mix) + o2 * mix;
                    if p.ring > 0.0 {
                        s += (o1 * o2 - s) * p.ring;
                    }
                    sl += s * gl[u];
                    sr_ += s * gr[u];
                }
                let mut mono = 0.0;
                if osc3_on {
                    mono += self.osc3.next_pw(p.osc3, dt3, 0.0, w3) * p.osc3_level;
                }
                if p.sub > 0.0 {
                    mono += self.sub.next(Wave::Square, dt_sub, 0.0) * p.sub * 0.7;
                }
                if noise > 0.0 {
                    mono += self.rng.noise() * noise * 0.5;
                }
                let a = self.amp.next() * gain;
                l[i] += self.filter[0].process(sl + mono) * a;
                r[i] += self.filter[1].process(sr_ + mono) * a;
            }
            start += m;
        }
        if self.amp.is_idle() {
            self.active = false;
        }
    }
}

pub struct SynthEngine {
    pub params: SynthParams,
    sr: f32,
    voices: [Voice; MAX_VOICES],
    counter: u64,
    /// Held notes in mono mode: (pitch, vel, off_at), most recent last.
    stack: [(u8, f32, f64); MONO_STACK],
    stack_len: usize,
    chorus: Ensemble,
    bpm: f32,
    /// Cutoff shift from automation, in octaves.
    bright: f32,
}

impl SynthEngine {
    pub fn new(params: SynthParams, sr: f32) -> Self {
        SynthEngine {
            params,
            sr,
            voices: [Voice::default(); MAX_VOICES],
            counter: 0,
            stack: [(0, 0.0, 0.0); MONO_STACK],
            stack_len: 0,
            chorus: Ensemble::new(sr),
            bpm: 120.0,
            bright: 0.0,
        }
    }

    pub fn set_tempo(&mut self, bpm: f32) {
        self.bpm = bpm.max(1.0);
    }

    /// Shift the brightness (automation), in octaves.
    pub fn set_brightness(&mut self, octaves: f32) {
        self.bright = octaves;
    }

    pub fn set_params(&mut self, params: SynthParams) {
        if params.mono != self.params.mono {
            self.all_off();
        }
        self.params = params;
        for v in self.voices.iter_mut().filter(|v| v.active) {
            v.amp.set(&params.amp, self.sr);
            v.filt.set(&params.filter_adsr, self.sr);
        }
    }

    pub fn note_on(&mut self, pitch: u8, vel: f32, off_at: f64) {
        self.counter += 1;
        let seed = (self.counter as u32).wrapping_mul(2_654_435_761) ^ pitch as u32;
        if self.params.mono {
            if self.stack_len == MONO_STACK {
                self.stack.copy_within(1.., 0);
                self.stack_len -= 1;
            }
            self.stack[self.stack_len] = (pitch, vel, off_at);
            self.stack_len += 1;
            let v = &mut self.voices[0];
            if v.active && !v.amp.is_released() {
                // Legato: glide to the new pitch without retriggering.
                v.target = pitch as f32;
                v.note = pitch;
                v.off_at = off_at;
                v.vel = vel;
            } else {
                let glide_from = if v.active && !self.params.slide_only {
                    Some(v.pitch)
                } else {
                    None
                };
                v.start(
                    &self.params,
                    self.sr,
                    pitch,
                    vel,
                    off_at,
                    self.counter,
                    seed,
                );
                if let Some(from) = glide_from {
                    v.pitch = from;
                }
            }
            return;
        }
        let idx = self
            .voices
            .iter()
            .position(|v| !v.active)
            .or_else(|| {
                // Steal: prefer the oldest released voice, then the oldest.
                self.voices
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| (!v.amp.is_released(), v.age))
                    .map(|(i, _)| i)
            })
            .unwrap_or(0);
        self.voices[idx].start(
            &self.params,
            self.sr,
            pitch,
            vel,
            off_at,
            self.counter,
            seed,
        );
    }

    fn mono_update(&mut self) {
        let v = &mut self.voices[0];
        if self.stack_len == 0 {
            v.amp.gate_off();
            v.filt.gate_off();
        } else {
            let (pitch, vel, off_at) = self.stack[self.stack_len - 1];
            v.target = pitch as f32;
            v.note = pitch;
            v.vel = vel;
            v.off_at = off_at;
        }
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
        if j != before {
            self.mono_update();
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
            .filter(|v| v.active && v.off_at <= tick)
        {
            v.amp.gate_off();
            v.filt.gate_off();
            v.off_at = f64::INFINITY;
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
            .filter(|v| v.active && v.note == pitch && v.off_at.is_infinite())
        {
            v.amp.gate_off();
            v.filt.gate_off();
        }
    }

    pub fn all_off(&mut self) {
        self.stack_len = 0;
        for v in self.voices.iter_mut() {
            v.amp.gate_off();
            v.filt.gate_off();
            v.off_at = f64::INFINITY;
        }
    }

    pub fn render(&mut self, l: &mut [f32], r: &mut [f32]) {
        for v in self.voices.iter_mut().filter(|v| v.active) {
            v.render(&self.params, self.sr, self.bpm, self.bright, l, r);
        }
        let drive = self.params.drive;
        if drive > 0.0 {
            // A fuzz pedal after the synth: hard gain into a soft clipper.
            let pre = 1.0 + 14.0 * drive * drive;
            let makeup = 1.0 / (1.0 + 4.0 * drive);
            for x in l.iter_mut().chain(r.iter_mut()) {
                *x = soft_clip(*x * pre) * makeup;
            }
        }
        if self.params.chorus > 0.0 {
            self.chorus.process(l, r, self.params.chorus);
        }
    }
}
