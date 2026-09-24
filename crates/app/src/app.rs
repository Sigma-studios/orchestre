use std::collections::{HashMap, HashSet};

use orchestre_audio::Audio;
use orchestre_core::edit::Clip;
use orchestre_core::{Id, Key, PPQ, Project, Tick};
use orchestre_dsp::{Cmd, Event, Song};

use crate::history::History;
use crate::io::Inbox;

const STORAGE_KEY: &str = "orchestre_project";
const LAYOUT_KEY: &str = "orchestre_kb_layout";

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
    pub track: Id,
    pub key: Key,
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
    pub kb_layout: crate::input::KbLayout,
    /// Chosen by the user; otherwise detected from key presses.
    pub kb_layout_manual: bool,
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
}

impl OrchestreApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::theme::apply(&cc.egui_ctx);
        let saved = cc
            .storage
            .and_then(|s| s.get_string(STORAGE_KEY))
            .and_then(|json| crate::io::parse_project(&json).ok());
        let first_run = saved.is_none();
        // Stored as "<layout>" when chosen by the user, "auto:<layout>" when detected.
        let layout_pref = cc
            .storage
            .and_then(|s| s.get_string(LAYOUT_KEY))
            .unwrap_or_default();
        let (kb_layout_manual, layout_name) = match layout_pref.strip_prefix("auto:") {
            Some(name) => (false, name.to_string()),
            None => (!layout_pref.is_empty(), layout_pref.clone()),
        };
        let kb_layout = crate::input::KbLayout::from_label(&layout_name).unwrap_or_default();
        let project = saved.unwrap_or_else(Project::demo);
        let selected = if first_run {
            project.tracks.get(1).map(|t| t.id)
        } else {
            None
        };
        let mut app = Self::build(project, Audio::new());
        app.selected = selected;
        app.kb_layout = kb_layout;
        app.kb_layout_manual = kb_layout_manual;
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
            kb_layout: Default::default(),
            kb_layout_manual: false,
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
            || self.rec.counting.is_some()
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
        self.tick(&ctx);
        crate::input::handle(self, &ctx);

        crate::ui::transport::show(self, ui);
        crate::ui::lanes::show(self, ui);
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(ui.style()).inner_margin(0.0))
            .show(ui, |ui| crate::ui::editor::show(self, ui));
        crate::ui::dialogs::show(self, &ctx);

        self.sync_audio();
        self.checkpoint(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Ok(json) = crate::io::project_to_json(&self.project) {
            storage.set_string(STORAGE_KEY, json);
        }
        let layout = self.kb_layout.label();
        let pref = if self.kb_layout_manual {
            layout.to_string()
        } else {
            format!("auto:{layout}")
        };
        storage.set_string(LAYOUT_KEY, pref);
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(10)
    }
}
