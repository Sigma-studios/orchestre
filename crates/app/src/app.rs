use std::collections::{HashMap, HashSet};

use orchestre_audio::Audio;
use orchestre_core::edit::Clip;
use orchestre_core::{Id, Key, PPQ, Project, Tick};
use orchestre_dsp::{Cmd, Event, Song};

use crate::history::History;
use crate::io::Inbox;

const STORAGE_KEY: &str = "orchestre_project";
const SOUND_KEY: &str = "orchestre_sound";
/// The sound as last opened or saved, to know what is unsaved after a restart.
const SOUND_SAVED_KEY: &str = "orchestre_sound_saved";
const SOUND_PATH_KEY: &str = "orchestre_sound_path";
const SOUND_DIR_KEY: &str = "orchestre_sound_dir";
const MODE_KEY: &str = "orchestre_mode";

/// Horizontal view shared by the note editor and the track lanes.
pub struct View {
    /// Pixels per quarter note.
    pub zoom: f32,
    /// Tick at the left edge of the timeline.
    pub scroll: f64,
    /// Vertical scroll of each track's note editor, in pixels.
    pub roll_scroll: HashMap<Id, f32>,
    /// Timeline width in pixels, as of the last frame.
    pub width: f32,
    pub fitted: bool,
}

impl View {
    pub fn px_per_tick(&self) -> f32 {
        self.zoom / PPQ as f32
    }

    pub fn tick_to_x(&self, left: f32, tick: f64) -> f32 {
        left + ((tick - self.scroll) as f32) * self.px_per_tick()
    }

    pub fn x_to_tick(&self, left: f32, x: f32) -> f64 {
        self.scroll + ((x - left) / self.px_per_tick()) as f64
    }

    /// Zoom around the timeline position under `x`.
    pub fn zoom_at(&mut self, left: f32, x: f32, factor: f32) {
        let anchor = self.x_to_tick(left, x);
        self.zoom = (self.zoom * factor).clamp(8.0, 600.0);
        self.scroll = (anchor - ((x - left) / self.px_per_tick()) as f64).max(0.0);
    }

    pub fn scroll_by_px(&mut self, dx: f32) {
        self.scroll = (self.scroll - (dx / self.px_per_tick()) as f64).max(0.0);
    }
}

/// The key-lock confirmation dialog.
pub struct KeyLockPrompt {
    /// `None`: the song key is changing; `Some`: one track starts following it.
    pub track: Option<Id>,
    pub key: Key,
    /// The song key before the change (offers "transpose" when set).
    pub from: Option<Key>,
    pub conflicts: usize,
}

/// An in-progress mouse gesture in the note editor.
pub enum Drag {
    Move {
        grab: Id,
        anchor_tick: f64,
        anchor_row: i32,
        orig: Vec<orchestre_core::Note>,
        last_pitch: Option<u8>,
    },
    Resize {
        grab: Id,
        anchor_tick: f64,
        orig: Vec<orchestre_core::Note>,
    },
    Select {
        origin: (f64, f32),
        current: (f64, f32),
        base: HashSet<Id>,
    },
    /// Drawing a new note: dragging sets its length.
    Draw { id: Id, start: Tick },
    /// Painting drum hits along a row, one per grid step.
    Paint { pitch: u8 },
}

pub struct OrchestreApp {
    pub project: Project,
    pub history: History,
    pub audio: Audio,
    pub view: View,
    /// Track whose note editor is open.
    pub selected: Option<Id>,
    /// Selected notes (in the selected track).
    pub selection: HashSet<Id>,
    pub clipboard: Clip,
    pub drag: Option<Drag>,
    pub playing: bool,
    pub position: f64,
    pub loop_on: bool,
    pub follow: bool,
    pub levels: HashMap<Id, f32>,
    /// Length of the next note drawn with a click.
    pub note_len: Tick,
    pub key_prompt: Option<KeyLockPrompt>,
    pub confirm_delete_track: Option<Id>,
    pub renaming: Option<(Id, String)>,
    pub octave: i8,
    pub settings: crate::settings::Settings,
    pub settings_open: bool,
    /// Keyboard keys currently held down, and the note they play.
    pub held_keys: HashMap<egui::Key, (Id, u8)>,
    /// Short note previews: (track, pitch, stop time).
    pub auditions: Vec<(Id, u8, f64)>,
    pub toast: Option<(String, f64)>,
    pub inbox: Inbox,
    /// Last position of the mouse over the note editor, in ticks.
    pub hover_tick: Option<Tick>,
    #[cfg(not(target_arch = "wasm32"))]
    pub midi: Option<crate::midi::Midi>,
    last_song: Option<Song>,
    song_dirty: bool,
    loop_sent: Option<Option<(Tick, Tick)>>,
    /// Set whenever `project` is modified; drives undo checkpoints.
    changed: bool,
    pub(crate) now: f64,
    /// When the last position report arrived (for extrapolating the playhead).
    pub(crate) position_time: f64,
    pub rec: crate::record::Recorder,
    pub preview: crate::ui::preview::PreviewState,
    pub mode: crate::sfx::Mode,
    pub sfx: crate::sfx::SfxEditor,
}

impl OrchestreApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::theme::apply(&cc.egui_ctx);
        let saved = cc
            .storage
            .and_then(|s| s.get_string(STORAGE_KEY))
            .and_then(|json| crate::io::parse_project(&json).ok());
        let first_run = saved.is_none();
        let project = saved.unwrap_or_else(Project::demo);
        let selected = if first_run {
            project.tracks.get(1).map(|t| t.id)
        } else {
            None
        };
        let mut app = Self::build(project, Audio::new());
        app.selected = selected;
        app.settings = crate::settings::Settings::load(cc.storage);
        if let Some(storage) = cc.storage {
            restore_sounds(&mut app, storage);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            app.midi = crate::midi::Midi::connect(cc.egui_ctx.clone());
        }
        if first_run {
            app.notify("Welcome! Press Space to play the demo song.");
        }
        app
    }

    /// The app state without any window or saved settings.
    pub fn build(project: Project, audio: Audio) -> Self {
        OrchestreApp {
            history: History::new(&project),
            project,
            audio,
            view: View {
                zoom: 60.0,
                scroll: 0.0,
                roll_scroll: HashMap::new(),
                width: 800.0,
                fitted: false,
            },
            selected: None,
            selection: HashSet::new(),
            clipboard: Clip::default(),
            drag: None,
            playing: false,
            position: 0.0,
            loop_on: true,
            follow: true,
            levels: HashMap::new(),
            note_len: PPQ / 2,
            key_prompt: None,
            confirm_delete_track: None,
            renaming: None,
            octave: 4,
            settings: Default::default(),
            settings_open: false,
            held_keys: HashMap::new(),
            auditions: Vec::new(),
            toast: None,
            inbox: Inbox::default(),
            hover_tick: None,
            #[cfg(not(target_arch = "wasm32"))]
            midi: None,
            last_song: None,
            song_dirty: true,
            loop_sent: None,
            changed: false,
            now: 0.0,
            position_time: 0.0,
            rec: Default::default(),
            preview: Default::default(),
            mode: Default::default(),
            sfx: crate::sfx::SfxEditor::new(
                orchestre_core::sfx::presets::find("Explosion")
                    .map_or_else(Default::default, |p| p.sound()),
            ),
        }
    }

    /// Call after any change to `project`.
    pub fn touch(&mut self) {
        self.changed = true;
        self.song_dirty = true;
    }

    pub fn notify(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), self.now + 3.5));
    }

    pub fn selected_track(&self) -> Option<&orchestre_core::Track> {
        self.selected.and_then(|id| self.project.track(id))
    }

    pub fn select_track(&mut self, id: Option<Id>) {
        if self.selected != id {
            self.stop_recording();
            self.selection.clear();
            self.drag = None;
            self.release_held_keys();
        }
        self.selected = id;
    }

    pub fn send(&mut self, cmd: Cmd) {
        self.audio.send(cmd);
    }

    pub fn play(&mut self) {
        self.sync_audio();
        self.send(Cmd::Play);
        self.playing = true;
    }

    pub fn stop(&mut self) {
        self.stop_recording();
        self.send(Cmd::Stop);
        self.playing = false;
    }

    pub fn toggle_play(&mut self) {
        if self.playing || self.rec.counting.is_some() {
            self.stop()
        } else {
            self.play()
        }
    }

    pub fn seek(&mut self, tick: f64) {
        let t = tick.max(0.0);
        self.position = t;
        self.send(Cmd::Seek(t as Tick));
    }

    /// Play a note briefly (when drawing or clicking notes).
    pub fn audition(&mut self, track: Id, pitch: u8) {
        if self.playing {
            return;
        }
        self.auditions.retain(|&(t, p, _)| {
            if t == track && p == pitch {
                return false;
            }
            true
        });
        self.send(Cmd::LiveNoteOn {
            track,
            pitch,
            vel: 0.8,
        });
        self.auditions.push((track, pitch, self.now + 0.25));
    }

    pub fn release_held_keys(&mut self) {
        let held: Vec<(egui::Key, (Id, u8))> = self.held_keys.drain().collect();
        for (key, (track, pitch)) in held {
            self.send(Cmd::LiveNoteOff { track, pitch });
            self.record_note_off(crate::record::Source::Key(key));
        }
    }

    pub fn naming(&self) -> orchestre_core::NoteNaming {
        self.settings.note_naming
    }

    /// Change the song key, asking first if existing notes don't fit.
    pub fn request_song_key(&mut self, key: Option<Key>) {
        use orchestre_core::edit::KeyChange;
        let Some(k) = key else {
            self.project.set_key(None, KeyChange::Delete);
            self.touch();
            return;
        };
        let conflicts = self.project.notes_outside(k);
        if conflicts == 0 {
            self.project.set_key(Some(k), KeyChange::Delete);
            self.touch();
        } else {
            self.key_prompt = Some(KeyLockPrompt {
                track: None,
                key: k,
                from: self.project.key,
                conflicts,
            });
        }
    }

    /// Make a track follow (or stop following) the song key.
    pub fn request_follow(&mut self, track: Id, follow: bool) {
        use orchestre_core::edit::{KeyConflict, notes_outside_key};
        let conflicts = match (follow, self.project.key, self.project.track(track)) {
            (true, Some(k), Some(t)) => notes_outside_key(t, k).len(),
            _ => 0,
        };
        if conflicts == 0 {
            self.project.set_follow(track, follow, KeyConflict::Delete);
            self.touch();
        } else if let Some(key) = self.project.key {
            self.key_prompt = Some(KeyLockPrompt {
                track: Some(track),
                key,
                from: None,
                conflicts,
            });
        }
    }

    pub fn undo(&mut self) {
        if let Some(p) = self.history.undo(&self.project) {
            self.project = p;
            self.after_history_jump();
        }
    }

    pub fn redo(&mut self) {
        if let Some(p) = self.history.redo() {
            self.project = p;
            self.after_history_jump();
        }
    }

    fn after_history_jump(&mut self) {
        self.drag = None;
        if let Some(id) = self.selected
            && self.project.track(id).is_none()
        {
            self.selected = None;
        }
        let existing: HashSet<Id> = self
            .selected_track()
            .map(|t| t.notes.iter().map(|n| n.id).collect())
            .unwrap_or_default();
        self.selection.retain(|id| existing.contains(id));
        self.song_dirty = true;
    }

    /// Replace the whole project (open / new), keeping it undoable.
    pub fn replace_project(&mut self, project: Project) {
        self.stop();
        self.project = project;
        self.selected = None;
        self.selection.clear();
        self.view.fitted = false;
        self.view.roll_scroll.clear();
        self.seek(0.0);
        self.touch();
    }

    pub fn loop_range(&self) -> Option<(Tick, Tick)> {
        self.loop_on.then(|| (0, self.project.length_ticks()))
    }

    /// Push project changes to the audio engine. Parameter-only changes
    /// (knobs, volume…) are sent without resending all notes.
    pub fn sync_audio(&mut self) {
        if self.song_dirty {
            self.song_dirty = false;
            self.project.fit_length();
            let song = Song::from_project(&self.project);
            match &self.last_song {
                Some(prev) if prev.same_notes(&song) => {
                    let changed: Vec<_> = prev
                        .tracks
                        .iter()
                        .zip(&song.tracks)
                        .filter(|(a, b)| a.params != b.params)
                        .map(|(_, b)| b.params)
                        .collect();
                    for p in changed {
                        self.audio.send(Cmd::SetTrack(p));
                    }
                }
                _ => self.audio.send(Cmd::SetSong(Box::new(song.clone()))),
            }
            self.last_song = Some(song);
        }
        let range = self.loop_range();
        if self.loop_sent != Some(range) {
            self.loop_sent = Some(range);
            self.audio.send(Cmd::SetLoop(range));
        }
    }

    fn poll_audio(&mut self) {
        let mut position = None;
        let mut levels = Vec::new();
        let mut count_in = None;
        self.audio.poll_events(|e| match e {
            Event::Position { tick, playing } => position = Some((tick, playing)),
            Event::Level { track, peak } => levels.push((track, peak)),
            Event::CountIn { beats_left } => count_in = Some(beats_left),
        });
        if let Some(beats) = count_in
            && self.rec.active
        {
            self.rec.counting = Some(beats);
        }
        if let Some((tick, playing)) = position {
            self.position = tick;
            self.position_time = self.now;
            self.playing = playing;
            if playing {
                self.rec.counting = None;
            }
        }
        for (track, peak) in levels {
            let l = self.levels.entry(track).or_insert(0.0);
            *l = peak.max(*l * 0.7);
        }
    }

    fn tick(&mut self, ctx: &egui::Context) {
        self.now = ctx.input(|i| i.time);
        self.poll_audio();
        self.update_recording();

        let now = self.now;
        let (due, keep): (Vec<_>, Vec<_>) = self.auditions.drain(..).partition(|a| a.2 <= now);
        self.auditions = keep;
        for (track, pitch, _) in due {
            self.send(Cmd::LiveNoteOff { track, pitch });
        }

        #[cfg(not(target_arch = "wasm32"))]
        self.poll_midi();

        crate::io::poll_inbox(self);

        if self.toast.as_ref().is_some_and(|t| t.1 < now) {
            self.toast = None;
        }

        // Follow the playhead, page by page.
        if self.playing && self.follow && self.drag.is_none() {
            let visible = (self.view.width / self.view.px_per_tick()) as f64;
            if self.position < self.view.scroll || self.position > self.view.scroll + visible * 0.95
            {
                self.view.scroll = (self.position - visible * 0.05).max(0.0);
            }
        }

        let animating = self.playing
            || self.sounds_animating()
            || self.rec.counting.is_some()
            || self.preview.pending()
            || !self.auditions.is_empty()
            || self.toast.is_some()
            || self.levels.values().any(|&l| l > 0.001);
        if animating {
            ctx.request_repaint();
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn poll_midi(&mut self) {
        let Some(midi) = &self.midi else { return };
        let msgs = midi.poll();
        let Some(track) = self.selected_track() else {
            return;
        };
        let (id, drums, key) = (track.id, track.instrument.is_drums(), track.effective_key());
        for m in msgs {
            let pitch = if drums {
                // General MIDI drum map subset → our drum rows.
                match m.pitch {
                    35 | 36 => 0,
                    38 | 40 => 1,
                    39 => 2,
                    37 => 3,
                    42 | 44 => 4,
                    46 => 5,
                    41 | 43 => 6,
                    45 | 47 => 7,
                    48 | 50 => 8,
                    49 | 57 => 9,
                    56 => 10,
                    54 => 11,
                    69 | 70 | 82 => 12,
                    75 => 13,
                    62 | 63 => 14,
                    64 => 15,
                    60 | 61 => 16,
                    _ => continue,
                }
            } else {
                key.map_or(m.pitch, |k| k.nearest(m.pitch))
            };
            let cmd = if m.on {
                Cmd::LiveNoteOn {
                    track: id,
                    pitch,
                    vel: m.vel,
                }
            } else {
                Cmd::LiveNoteOff { track: id, pitch }
            };
            self.send(cmd);
            let src = crate::record::Source::Midi(m.pitch);
            if m.on {
                self.record_note_on(src, pitch, m.vel);
            } else {
                self.record_note_off(src);
            }
        }
    }

    /// End of frame: record an undo step once the user has finished an
    /// interaction (no mouse button held), so a whole drag is one step.
    fn checkpoint(&mut self, ctx: &egui::Context) {
        // A whole recording take is one undo step.
        let pointer_down = ctx.input(|i| i.pointer.any_down());
        self.commit_if_idle(pointer_down);
        self.sounds_end_frame(pointer_down);
        self.sync_sound_file(pointer_down || ctx.egui_wants_keyboard_input());
    }

    pub(crate) fn commit_if_idle(&mut self, pointer_down: bool) {
        let busy = pointer_down || self.drag.is_some() || self.rec.active;
        if self.changed && !busy {
            self.changed = false;
            self.history.commit(&self.project);
        }
    }
}

impl eframe::App for OrchestreApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Sounds with unsaved changes: ask before quitting.
        if ctx.input(|i| i.viewport().close_requested())
            && !self.sfx.quit_confirmed
            && !self.sfx.unsaved_names().is_empty()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.sfx.quitting = true;
        }
        self.tick(&ctx);
        crate::input::handle(self, &ctx);

        match self.mode {
            crate::sfx::Mode::Music => {
                crate::ui::transport::show(self, ui);
                crate::ui::lanes::show(self, ui);
                egui::CentralPanel::default()
                    .frame(egui::Frame::central_panel(ui.style()).inner_margin(0.0))
                    .show(ui, |ui| crate::ui::editor::show(self, ui));
            }
            crate::sfx::Mode::Sounds => crate::ui::sfx::show(self, ui),
        }
        crate::ui::dialogs::show(self, &ctx);
        crate::settings::window(self, &ctx);
        crate::ui::preview::end_frame(self);

        self.sync_audio();
        self.checkpoint(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Ok(json) = crate::io::project_to_json(&self.project) {
            storage.set_string(STORAGE_KEY, json);
        }
        self.settings.save(storage);
        use orchestre_core::sfx::file::sound_to_json;
        if let (Ok(sound), Ok(saved)) = (
            sound_to_json(&self.sfx.sound),
            sound_to_json(&self.sfx.baseline),
        ) {
            storage.set_string(SOUND_KEY, sound);
            storage.set_string(SOUND_SAVED_KEY, saved);
        }
        let path = self
            .sfx
            .path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned());
        storage.set_string(SOUND_PATH_KEY, path.unwrap_or_default());
        let dir = self
            .sfx
            .library
            .as_ref()
            .map(|l| l.dir().to_string_lossy().into_owned());
        storage.set_string(SOUND_DIR_KEY, dir.unwrap_or_default());
        let mode = serde_json::to_string(&self.mode).unwrap_or_default();
        storage.set_string(MODE_KEY, mode);
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(10)
    }
}

/// Bring back the sound mode as it was: the sound, its file and folder.
fn restore_sounds(app: &mut OrchestreApp, storage: &dyn eframe::Storage) {
    use orchestre_core::sfx::file::parse_sound;
    let get = |key| storage.get_string(key).filter(|s| !s.is_empty());
    if let Some(sound) = get(SOUND_KEY).and_then(|j| parse_sound(&j).ok()) {
        let path = get(SOUND_PATH_KEY).map(std::path::PathBuf::from);
        app.sfx.set_sound(sound.clone(), path);
        app.sfx.baseline = get(SOUND_SAVED_KEY)
            .and_then(|j| parse_sound(&j).ok())
            .unwrap_or(sound);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(dir) = get(SOUND_DIR_KEY).map(std::path::PathBuf::from)
        && dir.is_dir()
    {
        app.sfx.library = Some(crate::sfx::Library::open(dir));
    }
    if let Some(mode) = get(MODE_KEY).and_then(|m| serde_json::from_str(&m).ok()) {
        app.mode = mode;
    }
}
