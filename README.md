# Orchestre

A beginner-friendly music maker. Every sound (synths, drums, piano) is synthesized from scratch; no samples. Runs natively and in the browser from one Rust codebase.

## Run

```sh
# Native
cargo run --release -p orchestre

# Web (needs: rustup target add wasm32-unknown-unknown; cargo install trunk)
trunk serve --open        # http://127.0.0.1:8080
trunk build --release     # static site in dist/
```

## Layout

| Crate | What |
|---|---|
| `crates/core` | Project model (tracks, notes, time, keys/scales) and pure edit ops. No audio/UI deps. |
| `crates/dsp` | Realtime engine: oscillators, filters, envelopes, synth/drum/piano voices, reverb/echo, sequencer, offline render. |
| `crates/audio` | Output backends: `cpal` natively; an AudioWorklet in the browser. |
| `crates/dsp-worklet` | The engine as a plain C-ABI wasm module, loaded by `web/worklet.js`. |
| `crates/app` | The egui/eframe UI. |
| `crates/bevy` | `orchestre-bevy`: Bevy 0.19 plugin that plays `.orch` files as audio assets. |

On the web the engine runs in an AudioWorklet (built by a Trunk pre-build hook into `web/engine.wasm`). It is fed postcard-encoded commands over the node's MessagePort, so UI work on the main thread never causes audio glitches, and no special COOP/COEP headers are needed.

## Using it

- Click a track at the bottom to open its notes above it. Click it again, or press Esc, to close.
- In the note editor:
  - click to add a note;
  - drag a note to move it, or drag its right edge to change its length;
  - right-click a note to delete it;
  - drag on empty space to select.
- Ctrl/Cmd + C / X / V / D: copy, cut, paste (at the mouse), duplicate. Arrows nudge the selection; Alt disables snapping.
- **Key** (sidebar): locks a melodic track to a key and shows only in-key rows. If existing notes don't fit, you're asked whether to delete them or move them to the nearest in-key note.
- Space: play/stop. Ctrl/Cmd+Z: undo; Ctrl/Cmd+Shift+Z: redo.
- Your computer keyboard plays the selected track. On melodic tracks the home row (A S D F… on QWERTY, Q S D F… on AZERTY) plays white keys and the row above plays black keys; `-` / `=` change the octave. On drum tracks each home-row key plays one drum, in order: kick, snare, clap, rim, hi-hat, open hat, toms, crash. MIDI keyboards work natively.
- File menu: save/open `.orch` songs and export WAV. The song also autosaves.

## Using songs in a Bevy game

`orchestre-bevy` plays `.orch` files directly, synthesizing the music live. A song is a few KB on disk instead of megabytes of audio, and costs under 1% of a CPU core to play.

```toml
# your game's Cargo.toml
orchestre-bevy = { path = "../orchestre/crates/bevy" }
```

```rust
use bevy::prelude::*;
use orchestre_bevy::{OrchestrePlugin, OrchestreSettings, OrchestreSong};

App::new().add_plugins((DefaultPlugins, OrchestrePlugin));

// In a system:
let music: Handle<OrchestreSong> = assets.load("music/theme.orch");   // loops seamlessly
commands.spawn(AudioPlayer(music));

let jingle: Handle<OrchestreSong> =
    assets.load_with_settings("music/win.orch", |s: &mut OrchestreSettings| s.looping = false);
commands.spawn((AudioPlayer(jingle), PlaybackSettings::DESPAWN));
```

Songs loop inside the engine, on the beat. Don't use `PlaybackSettings::LOOP`: Bevy would keep the whole song in memory and restart only after the reverb tail. Volume, pause and the other playback settings work as usual. Try it with `cargo run --release -p orchestre-bevy --example play`.

## Test

```sh
cargo test --workspace
cargo run -p orchestre-dsp --example levels   # loudness of each instrument
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.
