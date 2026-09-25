//! Sound effects mode: editing one sound (a `.orsfx` file) made of layers.

use std::collections::HashMap;
use std::path::PathBuf;

use orchestre_core::Id;
use orchestre_core::sfx::{Generator, GeneratorKind, Layer, Looping, PlayOpts, Sound};
use orchestre_dsp::Cmd;

use crate::app::OrchestreApp;
use crate::history::History;

/// Which document the app is editing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Mode {
    #[default]
    Music,
    Sounds,
}

/// Horizontal view of the layer timeline.
pub struct SfxView {
    /// Pixels per second.
    pub zoom: f32,
    /// Seconds at the left edge.
    pub scroll: f32,
    pub fitted: bool,
}

impl SfxView {
    pub fn time_to_x(&self, left: f32, t: f32) -> f32 {
        left + (t - self.scroll) * self.zoom
    }

    pub fn x_to_time(&self, left: f32, x: f32) -> f32 {
        self.scroll + (x - left) / self.zoom
    }

    pub fn zoom_at(&mut self, left: f32, x: f32, factor: f32) {
        let anchor = self.x_to_time(left, x);
        self.zoom = (self.zoom * factor).clamp(40.0, 20000.0);
        self.scroll = (anchor - (x - left) / self.zoom).max(0.0);
    }

    pub fn scroll_by_px(&mut self, dx: f32) {
        self.scroll = (self.scroll - dx / self.zoom).max(0.0);
    }
}

/// The sound folder shown on the left, with its subfolders (desktop only).
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub struct Library {
    pub root: Folder,
    /// App clock time of the last scan.
    pub scanned: f64,
    names: Names,
}

/// Each sound file's name as saved in it, and when the file was modified,
/// so a rescan only reads files that changed.
type Names = HashMap<PathBuf, (std::time::SystemTime, String)>;

/// The name saved in a sound file, else its file name.
fn saved_name(names: &mut Names, path: &std::path::Path, stem: String) -> String {
    let Some(modified) = std::fs::metadata(path).and_then(|m| m.modified()).ok() else {
        return stem;
    };
    if let Some((when, name)) = names.get(path)
        && *when == modified
    {
        return name.clone();
    }
    let name = std::fs::read_to_string(path)
        .ok()
        .and_then(|j| orchestre_core::sfx::file::parse_sound(&j).ok())
        .map(|s| s.name)
        .filter(|n| !n.trim().is_empty())
        .unwrap_or(stem);
    names.insert(path.to_path_buf(), (modified, name.clone()));
    name
}

/// A folder of sounds and the folders inside it, each sorted by name.
#[derive(Clone, Debug, PartialEq)]
pub struct Folder {
    pub path: PathBuf,
    pub folders: Vec<Folder>,
    /// (name, path) of each sound file; the name is the one saved in it.
    pub sounds: Vec<(String, PathBuf)>,
}

/// Folders deeper than this aren't scanned (and links can't loop forever).
const MAX_DEPTH: usize = 8;

/// Seconds between rescans of the sound folder, to see files that changed
/// outside Orchestre.
pub const RESCAN_EVERY: f64 = 2.0;

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
impl Folder {
    fn scan(path: PathBuf, depth: usize, names: &mut Names) -> Folder {
        let ext = orchestre_core::sfx::file::EXTENSION;
        let mut folder = Folder {
            path,
            folders: Vec::new(),
            sounds: Vec::new(),
        };
        let Ok(entries) = std::fs::read_dir(&folder.path) else {
            return folder;
        };
        for p in entries.filter_map(|e| e.ok().map(|e| e.path())) {
            let hidden = p
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('.'));
            if hidden {
                continue;
            }
            if p.is_dir() {
                if depth < MAX_DEPTH {
                    folder.folders.push(Folder::scan(p, depth + 1, names));
                }
            } else if p.extension().is_some_and(|e| e == ext)
                && let Some(stem) = p.file_stem()
            {
                let name = saved_name(names, &p, stem.to_string_lossy().into_owned());
                folder.sounds.push((name, p));
            }
        }
        folder.folders.sort_by_key(|f| f.name().to_lowercase());
        folder.sounds.sort_by_key(|(name, _)| name.to_lowercase());
        folder
    }

    pub fn name(&self) -> String {
        self.path.file_name().map_or_else(
            || self.path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        )
    }

    /// How many sounds are in this folder and every folder inside it.
    pub fn count(&self) -> usize {
        self.sounds.len() + self.folders.iter().map(Folder::count).sum::<usize>()
    }

    /// Every sound in the tree as (path relative to this folder, path),
    /// e.g. ("weapons/laser", …).
    pub fn all_sounds(&self) -> Vec<(String, PathBuf)> {
        let mut out = Vec::new();
        self.collect(&mut out, "");
        out
    }

    fn collect(&self, out: &mut Vec<(String, PathBuf)>, prefix: &str) {
        for (name, path) in &self.sounds {
            out.push((format!("{prefix}{name}"), path.clone()));
        }
        for f in &self.folders {
            f.collect(out, &format!("{prefix}{}/", f.name()));
        }
    }
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
impl Library {
    pub fn open(dir: PathBuf) -> Library {
        let mut names = Names::new();
        Library {
            root: Folder::scan(dir, 0, &mut names),
            scanned: f64::NEG_INFINITY,
            names,
        }
    }

    pub fn dir(&self) -> &PathBuf {
        &self.root.path
    }

    pub fn refresh(&mut self) {
        self.root = Folder::scan(self.root.path.clone(), 0, &mut self.names);
    }

    /// Rescan now and then; returns whether anything changed.
    pub fn rescan_if_due(&mut self, now: f64) -> bool {
        if now - self.scanned < RESCAN_EVERY {
            return false;
        }
        self.scanned = now;
        let fresh = Folder::scan(self.root.path.clone(), 0, &mut self.names);
        let changed = fresh != self.root;
        self.root = fresh;
        changed
    }
}

/// A name for a new sound in `dir` whose file doesn't exist yet: "Laser",
/// else "Laser 2", "Laser 3"… Returns the name and its file's path.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub fn free_name(dir: &std::path::Path, name: &str) -> (String, PathBuf) {
    let ext = orchestre_core::sfx::file::EXTENSION;
    let base = match name.trim() {
        "" => "New sound",
        n => n,
    };
    (1..)
        .map(|i| match i {
            1 => base.to_string(),
            i => format!("{base} {i}"),
        })
        .map(|n| {
            let path = dir.join(format!("{}.{ext}", file_stem(&n)));
            (n, path)
        })
        .find(|(_, p)| !p.exists())
        .expect("some name is free")
}

/// Cached waveform (peak per bin) of each layer, for drawing.
#[derive(Default)]
pub struct Waves {
    layers: HashMap<Id, (Layer, Vec<f32>)>,
}

/// Waveform resolution, bins per second.
pub const WAVE_BINS: f32 = 400.0;

impl Waves {
    /// Peaks of a layer from its start, `WAVE_BINS` per second.
    pub fn get(&mut self, sound: &Sound, layer: &Layer) -> &[f32] {
        // Where the layer sits and whether it's muted don't change its shape.
        let key = Layer {
            start: 0.0,
            mute: false,
            solo: false,
            pan: 0.0,
            name: String::new(),
            color: [0; 3],
            ..layer.clone()
        };
        let fresh = self.layers.get(&layer.id).is_some_and(|(k, _)| *k == key);
        if !fresh {
            let len = layer.generator.length();
            let bins = ((len * WAVE_BINS).ceil() as usize).clamp(4, 8000);
            let env =
                orchestre_dsp::sfx::layer_envelope(sound, layer, bins as f32 / WAVE_BINS, bins);
            self.layers.insert(layer.id, (key, env));
        }
        &self.layers[&layer.id].1
    }

    pub fn retain(&mut self, sound: &Sound) {
        self.layers.retain(|id, _| sound.layer(*id).is_some());
    }
}

/// A sound file with unsaved changes that isn't the one being edited, kept
/// as it was left, like a tab in a text editor.
pub struct Draft {
    pub sound: Sound,
    pub baseline: Sound,
    history: History<Sound>,
}

/// A sound waiting to replace the current one, once the user has decided
/// what to do with unsaved changes (only for sounds not saved to a file).
pub struct Replace {
    pub sound: Sound,
    pub path: Option<PathBuf>,
}

/// What a preview plays: a built-in preset or a sound file.
#[derive(Clone, Debug, PartialEq)]
pub enum Preview {
    Preset(&'static str),
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    File(PathBuf),
}

/// Rest the pointer on a preset or a sound to hear it.
#[derive(Default)]
pub struct SoundPreview {
    pub hovered: Option<Preview>,
    candidate: Option<Preview>,
    since: f64,
    /// What is playing and whether it loops.
    playing: Option<(Preview, bool)>,
}

/// Something to do to the sound folder, waiting for the person to choose
/// or confirm in a dialog.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub enum FileAction {
    /// Pick a preset (or nothing) for a new sound in this folder; `None`
    /// makes an unsaved sound. The name or search typed so far.
    NewSound(Option<PathBuf>, String),
    /// New folder inside this one, with the name typed so far.
    NewFolder(PathBuf, String),
    /// Rename this sound file; the name typed so far.
    Rename(PathBuf, String),
    Delete(PathBuf),
}

/// An in-progress drag of a layer along the timeline.
pub struct LayerDrag {
    pub id: Id,
    pub grab: f32,
    pub orig: f32,
}

pub struct SfxEditor {
    pub sound: Sound,
    pub history: History<Sound>,
    /// The file the sound was opened from or last saved to.
    pub path: Option<PathBuf>,
    /// The sound as last opened or saved; anything else is unsaved.
    pub baseline: Sound,
    pub selected: Option<Id>,
    /// Intensity used when playing in the editor, 0..2.
    pub intensity: f32,
    /// Values of the sound's named controls, by name, while previewing.
    pub controls: HashMap<String, f32>,
    /// Play the same variation every time (the one `seed` picks).
    pub lock_seed: bool,
    pub seed: u32,
    pub library: Option<Library>,
    pub view: SfxView,
    pub waves: Waves,
    pub drag: Option<LayerDrag>,
    pub replace: Option<Replace>,
    /// Sound files with unsaved changes, other than the one being edited.
    pub drafts: HashMap<PathBuf, Draft>,
    /// Asking what to do with unsaved sounds before quitting.
    pub quitting: bool,
    /// The person chose to quit, unsaved sounds dealt with.
    pub quit_confirmed: bool,
    pub preview: SoundPreview,
    pub action: Option<FileAction>,
    /// Start times (app clock) and lengths of recent plays, for playheads.
    pub plays: Vec<(f64, f32)>,
    /// The looping sound playing in the editor, as it now plays.
    pub looping: Option<Sound>,
    /// The version of the sound the last one-shot play started with.
    heard: Option<Sound>,
    /// App clock time a playing loop last took on an edit.
    last_swap: f64,
    /// A drag is changing a one-shot sound: it replays until it has been
    /// heard as it was left.
    auditioning: bool,
    pub changed: bool,
}

impl SfxEditor {
    pub fn new(sound: Sound) -> Self {
        SfxEditor {
            history: History::new(&sound),
            baseline: sound.clone(),
            sound,
            path: None,
            selected: None,
            intensity: 1.0,
            controls: HashMap::new(),
            lock_seed: false,
            seed: 1,
            library: None,
            view: SfxView {
                zoom: 400.0,
                scroll: 0.0,
                fitted: false,
            },
            waves: Waves::default(),
            drag: None,
            replace: None,
            drafts: HashMap::new(),
            quitting: false,
            quit_confirmed: false,
            preview: SoundPreview::default(),
            action: None,
            plays: Vec::new(),
            looping: None,
            heard: None,
            last_swap: f64::NEG_INFINITY,
            auditioning: false,
            changed: false,
        }
    }

    pub fn dirty(&self) -> bool {
        self.sound != self.baseline
    }

    pub fn touch(&mut self) {
        self.changed = true;
    }

    /// Switch to another sound. Unsaved changes to a sound file are kept
    /// for when it's opened again, and a file opened again comes back as it
    /// was left, unsaved changes and undo included.
    pub fn set_sound(&mut self, sound: Sound, path: Option<PathBuf>) {
        let next = path.as_ref().and_then(|p| self.drafts.remove(p));
        let next = next.unwrap_or_else(|| Draft {
            history: History::new(&sound),
            baseline: sound.clone(),
            sound,
        });
        let old = Draft {
            sound: std::mem::replace(&mut self.sound, next.sound),
            baseline: std::mem::replace(&mut self.baseline, next.baseline),
            history: std::mem::replace(&mut self.history, next.history),
        };
        if let Some(old_path) = self.path.take()
            && old.sound != old.baseline
            && path.as_ref() != Some(&old_path)
        {
            self.drafts.insert(old_path, old);
        }
        self.path = path;
        self.selected = None;
        self.drag = None;
        self.changed = false;
        self.view.fitted = false;
        self.waves = Waves::default();
        self.controls.clear();
    }

    pub fn add_layer(&mut self, kind: GeneratorKind) {
        let n = self
            .sound
            .layers
            .iter()
            .filter(|l| l.generator.kind() == kind)
            .count();
        let name = match n {
            0 => kind.label().to_string(),
            n => format!("{} {}", kind.label(), n + 1),
        };
        let id = self.sound.add_layer(&name, kind.default_generator());
        self.selected = Some(id);
        self.touch();
    }

    /// Copy layers from another sound (a preset or a library file).
    pub fn add_layers_from(&mut self, other: &Sound) {
        let mut last = None;
        for l in &other.layers {
            last = Some(self.sound.insert_layer(l));
        }
        if last.is_some() {
            self.selected = last;
            self.touch();
        }
    }

    pub fn duplicate_layer(&mut self, id: Id) {
        let Some(l) = self.sound.layer(id).cloned() else {
            return;
        };
        let new = self.sound.insert_layer(&Layer {
            name: format!("{} copy", l.name),
            ..l
        });
        // Right after the original, not at the bottom.
        if let Some(from) = self.sound.layers.iter().position(|l| l.id == new)
            && let Some(at) = self.sound.layers.iter().position(|l| l.id == id)
        {
            let layer = self.sound.layers.remove(from);
            self.sound.layers.insert(at + 1, layer);
        }
        self.selected = Some(new);
        self.touch();
    }

    pub fn delete_layer(&mut self, id: Id) {
        self.sound.layers.retain(|l| l.id != id);
        if self.selected == Some(id) {
            self.selected = None;
        }
        self.touch();
    }

    /// Change a layer's generator, keeping its place and mix.
    pub fn set_generator(&mut self, id: Id, generator: Generator) {
        if let Some(l) = self.sound.layer_mut(id) {
            l.generator = generator;
            self.touch();
        }
    }

    /// Pick another variation for the next play, even when locked.
    pub fn reroll(&mut self) {
        let lock = std::mem::replace(&mut self.lock_seed, false);
        self.next_opts();
        self.lock_seed = lock;
    }

    /// Options for the next play in the editor.
    pub fn next_opts(&mut self) -> PlayOpts {
        if !self.lock_seed {
            self.seed = self.seed.wrapping_mul(1_103_515_245).wrapping_add(12_345) | 1;
        }
        self.live_opts()
    }

    /// Intensity and control values as play options (current seed).
    pub fn live_opts(&self) -> PlayOpts {
        let base = PlayOpts {
            seed: self.seed,
            intensity: self.intensity,
            ..PlayOpts::default()
        };
        let values = self.sound.controls.iter().map(|c| {
            let v = self.controls.get(&c.name).copied().unwrap_or(c.base);
            (c.name.as_str(), v)
        });
        self.sound.with_controls(base, values)
    }

    /// What a preview plays and how: a sound file as it is being edited
    /// (unsaved changes included), the open one at the editor's intensity
    /// and control values.
    fn preview(&mut self, p: &Preview, seed: u32) -> Option<(Sound, PlayOpts)> {
        let seeded = PlayOpts::seeded(seed);
        match p {
            Preview::Preset(name) => {
                Some((orchestre_core::sfx::presets::find(name)?.sound(), seeded))
            }
            Preview::File(path) if self.path.as_ref() == Some(path) => {
                Some((self.sound.clone(), self.next_opts()))
            }
            Preview::File(path) => match self.drafts.get(path) {
                Some(draft) => Some((draft.sound.clone(), seeded)),
                None => std::fs::read_to_string(path)
                    .ok()
                    .and_then(|j| orchestre_core::sfx::file::parse_sound(&j).ok())
                    .map(|s| (s, seeded)),
            },
        }
    }

    /// Whether this sound file has unsaved changes, open or not.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn unsaved(&self, path: &PathBuf) -> bool {
        self.drafts.contains_key(path) || (self.path.as_ref() == Some(path) && self.dirty())
    }

    /// Names of every sound with unsaved changes.
    pub fn unsaved_names(&self) -> Vec<String> {
        let current =
            (self.dirty() && !self.sound.layers.is_empty()).then(|| self.sound.name.clone());
        let mut names: Vec<_> = current
            .into_iter()
            .chain(self.drafts.values().map(|d| d.sound.name.clone()))
            .collect();
        names.sort_by_key(|n| n.to_lowercase());
        names
    }

    /// Forget every unsaved change.
    pub fn discard_all(&mut self) {
        self.drafts.clear();
        self.sound = self.baseline.clone();
        self.history = History::new(&self.sound);
    }

    pub fn file_stem(&self) -> String {
        file_stem(&self.sound.name)
    }
}

/// A file name for a sound: lowercase words joined by dashes.
pub fn file_stem(name: &str) -> String {
    let stem: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let stem = stem
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if stem.is_empty() {
        "sound".into()
    } else {
        stem
    }
}

/// Seconds between a playing loop taking on edits during a drag: often enough
/// to follow a slider, not so often the crossfades pile up.
const SWAP_EVERY: f64 = 0.04;

/// Pointer must rest this long on a preset before it plays.
const PREVIEW_DELAY: f64 = 0.15;

impl OrchestreApp {
    pub fn set_mode(&mut self, mode: Mode) {
        if self.mode == mode {
            return;
        }
        match mode {
            Mode::Sounds => self.stop(),
            Mode::Music => {
                self.send(Cmd::StopSounds);
                self.sfx.looping = None;
            }
        }
        self.release_held_keys();
        self.mode = mode;
    }

    /// Play the sound being edited, at the editor's intensity. A looping
    /// sound plays until this is called again.
    pub fn play_sound(&mut self) {
        if self.sfx.looping.is_some() {
            self.release_sound();
            return;
        }
        let intensity = self.sfx.intensity;
        self.play_sound_at(intensity);
    }

    /// Play at a given intensity; while a loop plays, change its intensity.
    pub fn play_sound_at(&mut self, intensity: f32) {
        if self.sfx.looping.is_some() {
            self.sfx.intensity = intensity;
            self.sound_live_changed();
            return;
        }
        let saved = std::mem::replace(&mut self.sfx.intensity, intensity);
        let opts = self.sfx.next_opts();
        self.sfx.intensity = saved;
        let sound = self.sfx.sound.clone();
        if sound.looping != Looping::Once {
            self.sfx.looping = Some(sound.clone());
        } else {
            self.sfx.plays.push((self.now, sound.length()));
            self.sfx.heard = Some(sound.clone());
        }
        self.send(Cmd::PlaySound {
            sound: Box::new(sound),
            opts,
        });
    }

    /// Let a playing loop end.
    pub fn release_sound(&mut self) {
        self.sfx.looping = None;
        self.send(Cmd::ReleaseSounds);
    }

    /// The intensity slider or a control moved: playing sounds follow.
    pub fn sound_live_changed(&mut self) {
        let opts = self.sfx.live_opts();
        self.send(Cmd::SoundLive(opts));
    }

    /// Open a sound. Unsaved changes to a sound file are kept for later;
    /// only a sound that isn't saved anywhere asks first.
    pub fn request_sound(&mut self, sound: Sound, path: Option<PathBuf>) {
        if self.sfx.path.is_none() && self.sfx.dirty() && !self.sfx.sound.layers.is_empty() {
            self.sfx.replace = Some(Replace { sound, path });
        } else {
            self.set_sound(sound, path);
        }
    }

    /// Replace the sound right away, ending any loop of the old one.
    pub fn set_sound(&mut self, sound: Sound, path: Option<PathBuf>) {
        if self.sfx.looping.is_some() {
            self.release_sound();
        }
        self.sfx.set_sound(sound, path);
    }

    pub fn undo_sound(&mut self) {
        if let Some(s) = self.sfx.history.undo(&self.sfx.sound) {
            self.sfx.sound = s;
            self.after_sound_jump();
        }
    }

    pub fn redo_sound(&mut self) {
        if let Some(s) = self.sfx.history.redo() {
            self.sfx.sound = s;
            self.after_sound_jump();
        }
    }

    fn after_sound_jump(&mut self) {
        self.sfx.drag = None;
        if let Some(id) = self.sfx.selected
            && self.sfx.sound.layer(id).is_none()
        {
            self.sfx.selected = None;
        }
    }

    /// End of frame, in sound mode: previews and undo checkpoints.
    pub fn sounds_end_frame(&mut self, pointer_down: bool) {
        let now = self.now;
        let pv = &mut self.sfx.preview;
        let hovered = pv.hovered.take();
        if hovered != pv.candidate {
            pv.candidate = hovered;
            pv.since = now;
        }
        let due = pv.candidate.clone().filter(|c| {
            pv.playing.as_ref().is_none_or(|(p, _)| p != c) && now - pv.since >= PREVIEW_DELAY
        });
        if let Some(candidate) = due {
            let play = self.sfx.preview(&candidate, now.to_bits() as u32 | 1);
            let loops = play
                .as_ref()
                .is_some_and(|(s, _)| s.looping != Looping::Once);
            self.sfx.preview.playing = Some((candidate, loops));
            if let Some((sound, opts)) = play {
                self.sfx.looping = None;
                self.send(Cmd::StopSounds);
                self.send(Cmd::PlaySound {
                    sound: Box::new(sound),
                    opts,
                });
            }
        } else if pv.candidate.is_none()
            && let Some((_, loops)) = pv.playing.take()
            && loops
        {
            // Looping sounds play while the pointer rests on them.
            self.send(Cmd::ReleaseSounds);
        }

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(lib) = &mut self.sfx.library {
            lib.rescan_if_due(now);
        }

        self.hear_edits(pointer_down);

        self.sfx
            .plays
            .retain(|&(t, len)| now - t < len as f64 + 0.3);
        self.sfx.waves.retain(&self.sfx.sound);
        if self.sfx.changed && !pointer_down && self.sfx.drag.is_none() {
            self.sfx.changed = false;
            self.sfx.history.commit(&self.sfx.sound);
        }
    }

    /// A sound's name and its file's name are one: once the name has been
    /// changed (typed, or undone) and nobody is typing, rename the file.
    pub fn sync_sound_file(&mut self, typing: bool) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = self.sfx.path.clone()
            && !typing
            && self.sfx.sound.name != self.sfx.baseline.name
        {
            let name = self.sfx.sound.name.clone();
            if !crate::io::rename_sound(self, path, &name) {
                self.sfx.sound.name = self.sfx.baseline.name.clone();
            }
        }
        #[cfg(target_arch = "wasm32")]
        let _ = typing;
    }

    /// Edits are heard as they are made. A loop that is playing takes each
    /// one on where it has got to, without starting over; a one-shot plays
    /// again, back to back, for as long as a drag keeps changing it, and
    /// once more as it was let go.
    fn hear_edits(&mut self, pointer_down: bool) {
        let now = self.now;
        let sound = &self.sfx.sound;
        let playable = !sound.layers.is_empty();

        if let Some(playing) = &self.sfx.looping
            && playing != sound
        {
            if sound.looping == Looping::Once || !playable {
                self.send(Cmd::StopSounds);
                self.sfx.looping = None;
            } else if !pointer_down || now - self.sfx.last_swap >= SWAP_EVERY {
                self.sfx.last_swap = now;
                self.sfx.looping = Some(sound.clone());
                self.send(Cmd::SwapSound(Box::new(sound.clone())));
            }
            return;
        }

        if self.sfx.looping.is_some() || sound.looping != Looping::Once || !playable {
            self.sfx.auditioning = false;
            return;
        }
        if pointer_down && self.sfx.changed {
            self.sfx.auditioning = true;
        }
        let unheard = self.sfx.heard.as_ref() != Some(sound);
        let busy = self
            .sfx
            .plays
            .iter()
            .any(|&(at, len)| now < at + len as f64);
        if self.sfx.auditioning && unheard && !busy {
            self.play_sound();
        } else if !pointer_down && !unheard {
            self.sfx.auditioning = false;
        }
    }

    pub fn sounds_animating(&self) -> bool {
        let pv = &self.sfx.preview;
        !self.sfx.plays.is_empty()
            || pv
                .candidate
                .as_ref()
                .is_some_and(|c| pv.playing.as_ref().is_none_or(|(p, _)| p != c))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestre_core::sfx::presets;

    #[test]
    fn stems() {
        assert_eq!(file_stem("Step on a hard floor"), "step-on-a-hard-floor");
        assert_eq!(file_stem("  Power-up!! "), "power-up");
        assert_eq!(file_stem("???"), "sound");
        assert_eq!(file_stem("Déjà vu"), "déjà-vu");
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("orchestre-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn library_scans_subfolders() {
        let dir = temp_dir("library");
        std::fs::create_dir_all(dir.join("weapons/guns")).unwrap();
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        for f in [
            "coin.orsfx",
            "weapons/Laser.orsfx",
            "weapons/guns/shot.orsfx",
            ".git/x.orsfx",
            "notes.txt",
        ] {
            std::fs::write(dir.join(f), "{}").unwrap();
        }
        let mut lib = Library::open(dir.clone());
        assert_eq!(lib.root.count(), 3);
        let names: Vec<_> = lib.root.all_sounds().into_iter().map(|s| s.0).collect();
        assert_eq!(names, ["coin", "weapons/Laser", "weapons/guns/shot"]);
        assert!(!lib.rescan_if_due(0.0), "nothing changed");
        std::fs::write(dir.join("weapons/zap.orsfx"), "{}").unwrap();
        assert!(!lib.rescan_if_due(1.0), "not due yet");
        assert!(lib.rescan_if_due(10.0));
        assert_eq!(lib.root.folders[0].sounds.len(), 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn free_names_count_up() {
        let dir = temp_dir("names");
        let (name, path) = free_name(&dir, "Laser");
        assert_eq!(
            (name.as_str(), path.clone()),
            ("Laser", dir.join("laser.orsfx"))
        );
        std::fs::write(&path, "{}").unwrap();
        let (name, path) = free_name(&dir, "Laser");
        assert_eq!(
            (name.as_str(), path),
            ("Laser 2", dir.join("laser-2.orsfx"))
        );
        assert_eq!(free_name(&dir, "  ").0, "New sound");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn switching_sounds_keeps_unsaved_changes() {
        let coin = presets::find("Coin").unwrap().sound();
        let laser = presets::find("Laser").unwrap().sound();
        let (a, b) = (PathBuf::from("a.orsfx"), PathBuf::from("b.orsfx"));
        let mut e = SfxEditor::new(Sound::default());
        e.set_sound(coin.clone(), Some(a.clone()));
        e.add_layer(GeneratorKind::Noise);
        e.history.commit(&e.sound);
        let edited = e.sound.clone();

        // Opening another file keeps the edits, marked unsaved.
        e.set_sound(laser.clone(), Some(b.clone()));
        assert!(e.unsaved(&a) && !e.unsaved(&b));
        assert_eq!(e.unsaved_names(), std::slice::from_ref(&coin.name));

        // Opening it again (even as read from disk) brings the edits back,
        // undo included.
        e.set_sound(coin.clone(), Some(a.clone()));
        assert_eq!(e.sound, edited);
        assert!(e.dirty() && e.drafts.is_empty());
        e.sound = e.history.undo(&e.sound).unwrap();
        assert_eq!(e.sound, coin);

        // Unchanged sounds aren't kept.
        e.set_sound(laser.clone(), Some(b.clone()));
        assert!(e.drafts.is_empty());

        e.set_sound(coin.clone(), Some(a.clone()));
        e.add_layer(GeneratorKind::Tone);
        e.set_sound(laser, Some(b));
        e.add_layer(GeneratorKind::Tone);
        assert_eq!(e.unsaved_names().len(), 2);
        e.discard_all();
        assert!(e.unsaved_names().is_empty());
    }

    #[test]
    fn dirty_tracks_edits_since_open() {
        let mut e = SfxEditor::new(presets::find("Coin").unwrap().sound());
        assert!(!e.dirty());
        e.add_layer(GeneratorKind::Noise);
        assert!(e.dirty());
        e.set_sound(presets::find("Laser").unwrap().sound(), None);
        assert!(!e.dirty());
        assert_eq!(e.selected, None);
    }

    #[test]
    fn duplicate_goes_right_after_the_original() {
        let mut e = SfxEditor::new(presets::find("Explosion").unwrap().sound());
        let first = e.sound.layers[0].id;
        let count = e.sound.layers.len();
        e.duplicate_layer(first);
        assert_eq!(e.sound.layers.len(), count + 1);
        assert_eq!(
            e.sound.layers[1].name,
            format!("{} copy", e.sound.layers[0].name)
        );
        assert_eq!(e.selected, Some(e.sound.layers[1].id));
        e.delete_layer(e.sound.layers[1].id);
        assert_eq!(e.selected, None);
        assert_eq!(e.sound.layers.len(), count);
    }

    #[test]
    fn locked_seed_repeats() {
        let mut e = SfxEditor::new(Sound::default());
        let a = e.next_opts().seed;
        let b = e.next_opts().seed;
        assert_ne!(a, b);
        e.lock_seed = true;
        assert_eq!(e.next_opts().seed, b);
    }

    #[test]
    fn waves_recompute_only_when_the_shape_changes() {
        let mut e = SfxEditor::new(presets::find("Punch").unwrap().sound());
        let layer = e.sound.layers[0].clone();
        let a = e.waves.get(&e.sound, &layer).to_vec();
        assert!(a.iter().any(|&p| p > 0.1));
        let moved = Layer {
            start: 0.5,
            ..layer.clone()
        };
        assert_eq!(e.waves.get(&e.sound, &moved), &a[..]);
        let quieter = Layer {
            gain: layer.gain * 0.5,
            ..layer
        };
        let b = e.waves.get(&e.sound, &quieter).to_vec();
        assert!(b[2] < a[2]);
    }
}
