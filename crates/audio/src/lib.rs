//! Audio output backends. The UI talks to the engine only through [`Audio`],
//! which hides whether the engine runs on a native audio thread (cpal) or in
//! a browser AudioWorklet.

use orchestre_dsp::{Cmd, Event};

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
use native::Backend;
#[cfg(target_arch = "wasm32")]
use web::Backend;

pub struct Audio {
    backend: Result<Backend, String>,
}

impl Audio {
    /// Open the default output device. Failure is not fatal: the app keeps
    /// working silently and reports the error.
    pub fn new() -> Audio {
        let backend = Backend::new();
        if let Err(e) = &backend {
            log::error!("audio unavailable: {e}");
        }
        Audio { backend }
    }

    /// No audio output at all (tests, headless use).
    pub fn disabled() -> Audio {
        Audio {
            backend: Err("audio disabled".into()),
        }
    }

    pub fn send(&mut self, cmd: Cmd) {
        if let Ok(b) = &mut self.backend {
            b.send(cmd);
        }
    }

    pub fn poll_events(&mut self, mut f: impl FnMut(Event)) {
        if let Ok(b) = &mut self.backend {
            b.poll_events(&mut f);
        }
    }

    pub fn error(&self) -> Option<String> {
        match &self.backend {
            Ok(b) => b.error(),
            Err(e) => Some(e.clone()),
        }
    }
}

impl Default for Audio {
    fn default() -> Self {
        Audio::new()
    }
}
