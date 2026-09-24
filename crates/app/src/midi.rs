//! MIDI keyboard input (native only).

use std::sync::mpsc::{Receiver, channel};

use midir::{MidiInput, MidiInputConnection};

pub struct MidiNote {
    pub pitch: u8,
    pub vel: f32,
    pub on: bool,
}

pub struct Midi {
    _conns: Vec<MidiInputConnection<()>>,
    rx: Receiver<MidiNote>,
    pub device_names: Vec<String>,
}

impl Midi {
    /// Listen to every connected MIDI input. Returns `None` if there is none.
    pub fn connect(ctx: egui::Context) -> Option<Midi> {
        let (tx, rx) = channel();
        let probe = MidiInput::new("orchestre").ok()?;
        let ports = probe.ports();
        let mut conns = Vec::new();
        let mut names = Vec::new();
        for port in &ports {
            let Ok(input) = MidiInput::new("orchestre") else {
                continue;
            };
            let name = input.port_name(port).unwrap_or_else(|_| "MIDI".into());
            let (tx, ctx) = (tx.clone(), ctx.clone());
            let conn = input.connect(
                port,
                "orchestre-in",
                move |_, msg, _| {
                    let note = match msg {
                        [s, p, v] if s & 0xf0 == 0x90 && *v > 0 => MidiNote {
                            pitch: *p,
                            vel: *v as f32 / 127.0,
                            on: true,
                        },
                        [s, p, _] if s & 0xf0 == 0x80 || s & 0xf0 == 0x90 => MidiNote {
                            pitch: *p,
                            vel: 0.0,
                            on: false,
                        },
                        _ => return,
                    };
                    let _ = tx.send(note);
                    ctx.request_repaint();
                },
                (),
            );
            if let Ok(c) = conn {
                log::info!("MIDI input: {name}");
                names.push(name);
                conns.push(c);
            }
        }
        if conns.is_empty() {
            return None;
        }
        Some(Midi {
            _conns: conns,
            rx,
            device_names: names,
        })
    }

    pub fn poll(&self) -> Vec<MidiNote> {
        self.rx.try_iter().collect()
    }
}
