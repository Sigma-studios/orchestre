use orchestre_core::Adsr;

use crate::util::decay_coef;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Stage {
    #[default]
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

/// ADSR with a linear attack and exponential decay/release.
#[derive(Clone, Copy, Debug, Default)]
pub struct Env {
    stage: Stage,
    pub level: f32,
    attack_inc: f32,
    decay_coef: f32,
    sustain: f32,
    release_coef: f32,
}

impl Env {
    pub fn set(&mut self, adsr: &Adsr, sr: f32) {
        self.attack_inc = 1.0 / (adsr.attack * sr).max(1.0);
        self.decay_coef = decay_coef(adsr.decay.max(0.001), sr);
        self.sustain = adsr.sustain.clamp(0.0, 1.0);
        self.release_coef = decay_coef(adsr.release.max(0.003), sr);
    }

    /// Start (or restart, legato-style from the current level) the envelope.
    pub fn gate_on(&mut self) {
        self.stage = Stage::Attack;
    }

    pub fn gate_off(&mut self) {
        if self.stage != Stage::Idle {
            self.stage = Stage::Release;
        }
    }

    /// Fast fade used when a voice is stolen.
    pub fn is_idle(&self) -> bool {
        self.stage == Stage::Idle
    }

    pub fn is_released(&self) -> bool {
        matches!(self.stage, Stage::Release | Stage::Idle)
    }

    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> f32 {
        match self.stage {
            Stage::Idle => {}
            Stage::Attack => {
                self.level += self.attack_inc;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                self.level = self.sustain + (self.level - self.sustain) * self.decay_coef;
                if (self.level - self.sustain).abs() < 1e-4 {
                    self.level = self.sustain;
                    self.stage = if self.sustain <= 1e-4 {
                        Stage::Idle
                    } else {
                        Stage::Sustain
                    };
                }
            }
            Stage::Sustain => self.level = self.sustain,
            Stage::Release => {
                self.level *= self.release_coef;
                if self.level < 1e-4 {
                    self.level = 0.0;
                    self.stage = Stage::Idle;
                }
            }
        }
        self.level
    }
}
