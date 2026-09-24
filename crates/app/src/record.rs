//! Recording live keyboard / MIDI playing into the selected track.
//!
//! Notes appear as soon as a key goes down and grow while it is held.
//! Snapping to the grid is deferred until a loop pass ends or recording
//! stops: snapping a note *forward* while the playhead hasn't reached it
//! yet would make the sequencer play it a second time.

use std::collections::{HashMap, HashSet};

use orchestre_core::edit::dedup_notes;
use orchestre_core::{Id, Note, PPQ, Tick, Track, snap_round};
use orchestre_dsp::Cmd;

use crate::app::OrchestreApp;

/// What produced a recorded note, so its release can be matched.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    Key(egui::Key),
    #[cfg(not(target_arch = "wasm32"))]
    Midi(u8),
}

pub struct Recorder {
    pub active: bool,
    /// Beats left in the count-in, while it is running.
    pub counting: Option<u32>,
    pub count_in: bool,
    pub click: bool,
    pub snap: bool,
    held: HashMap<Source, Id>,
    /// Every note recorded in the current take.
    take: HashSet<Id>,
    /// Playhead estimate from the previous frame, to detect loop wraps.
    last_pos: f64,
}

impl Default for Recorder {
    fn default() -> Self {
        Recorder {
            active: false,
            counting: None,
            count_in: true,
            click: true,
            snap: true,
            held: HashMap::new(),
            take: HashSet::new(),
            last_pos: 0.0,
        }
    }
}

/// Snap the given notes' starts and lengths to `step`. Notes pushed onto
/// the loop end (pressed just before the downbeat) wrap to the loop start.
pub fn snap_notes(track: &mut Track, ids: &HashSet<Id>, step: Tick, loop_end: Option<Tick>) {
    let drums = track.instrument.is_drums();
    for n in track.notes.iter_mut().filter(|n| ids.contains(&n.id)) {
        let mut start = snap_round(n.start, step).max(0);
        if let Some(end) = loop_end
            && start >= end
        {
            start -= end;
        }
        n.start = start;
        n.len = if drums {
            step
        } else {
            snap_round(n.len, step).max(step)
        };
    }
    dedup_notes(track);
}

impl OrchestreApp {
    fn loop_end(&self) -> Option<Tick> {
        self.loop_range().map(|(_, b)| b)
    }

    /// Current playhead, extrapolated between the engine's position reports.
    pub fn estimated_position(&self) -> f64 {
        if !self.playing {
            return self.position;
        }
        let ticks_per_sec = self.project.bpm as f64 / 60.0 * PPQ as f64;
        let mut t = self.position + (self.now - self.position_time).max(0.0) * ticks_per_sec;
        if let Some(end) = self.loop_end()
            && end > 0
        {
            t %= end as f64;
        }
        t
    }

    /// True if the playhead going from `prev` to `now` is the loop
    /// restarting, rather than the estimate correcting itself slightly.
    fn is_wrap(&self, prev: f64, now: f64) -> bool {
        self.loop_end()
            .is_some_and(|end| prev - now > end as f64 / 2.0)
    }

    /// Playhead for recording: never runs backwards within a loop pass.
    fn rec_position(&self) -> f64 {
        let raw = self.estimated_position();
        if raw < self.rec.last_pos && !self.is_wrap(self.rec.last_pos, raw) {
            self.rec.last_pos
        } else {
            raw
        }
    }

    pub fn toggle_record(&mut self) {
        if self.rec.active {
            self.stop_recording();
        } else {
            self.start_recording();
        }
    }

    pub fn start_recording(&mut self) {
        if self.selected_track().is_none() {
            self.notify("Select a track to record into");
            return;
        }
        self.rec.active = true;
        self.rec.take.clear();
        self.rec.held.clear();
        self.rec.last_pos = self.position;
        self.send(Cmd::SetMetronome(self.rec.click));
        if !self.playing {
            self.sync_audio();
            let beats = self.project.time_sig.num as u32;
            if self.rec.count_in {
                self.rec.counting = Some(beats);
                self.send(Cmd::CountIn(beats));
            } else {
                self.play();
            }
        }
    }

    pub fn stop_recording(&mut self) {
        if !self.rec.active {
            return;
        }
        let pos = self.rec_position() as Tick;
        let held: Vec<Id> = self.rec.held.drain().map(|(_, id)| id).collect();
        for id in held {
            self.finish_note(id, pos);
        }
        self.snap_take();
        let n = self.rec.take.len();
        self.rec.active = false;
        self.rec.counting = None;
        self.rec.take.clear();
        self.send(Cmd::SetMetronome(false));
        if n > 0 {
            self.notify(format!(
                "Recorded {n} note{} (Undo removes the take)",
                if n == 1 { "" } else { "s" }
            ));
        }
    }

    pub fn record_note_on(&mut self, src: Source, pitch: u8, vel: f32) {
        if !self.rec.active || !self.playing {
            return;
        }
        let start = self.rec_position() as Tick;
        let Some(track) = self.selected else { return };
        let id = self.project.new_id();
        if let Some(t) = self.project.track_mut(track) {
            t.notes.push(Note {
                id,
                start,
                len: 1,
                pitch,
                vel,
            });
            self.rec.take.insert(id);
            if let Some(old) = self.rec.held.insert(src, id) {
                self.finish_note(old, start);
            }
            self.touch();
        }
    }

    pub fn record_note_off(&mut self, src: Source) {
        if let Some(id) = self.rec.held.remove(&src) {
            let pos = self.rec_position() as Tick;
            self.finish_note(id, pos);
        }
    }

    fn finish_note(&mut self, id: Id, end: Tick) {
        let step = self.project.grid_ticks();
        let song_end = self.project.length_ticks();
        let Some(t) = self.selected.and_then(|tid| self.project.track_mut(tid)) else {
            return;
        };
        let drums = t.instrument.is_drums();
        if let Some(n) = t.notes.iter_mut().find(|n| n.id == id) {
            // Released after a loop wrap: the note runs to the loop end.
            let wrapped = (n.start - end) as f64 > song_end as f64 / 2.0;
            let end = if wrapped { song_end } else { end.max(n.start) };
            n.len = if drums {
                step
            } else {
                (end - n.start).max(PPQ / 32)
            };
            self.touch();
        }
    }

    fn snap_take(&mut self) {
        if !self.rec.snap || self.rec.take.is_empty() {
            return;
        }
        let step = self.project.grid_ticks();
        let loop_end = self.loop_end();
        let take = self.rec.take.clone();
        if let Some(t) = self.selected.and_then(|id| self.project.track_mut(id)) {
            snap_notes(t, &take, step, loop_end);
            let existing: HashSet<Id> = t.notes.iter().map(|n| n.id).collect();
            self.rec.take.retain(|id| existing.contains(id));
            self.touch();
        }
    }

    /// Per frame: grow held notes, handle loop wraps and the song ending.
    pub fn update_recording(&mut self) {
        if !self.rec.active {
            return;
        }
        if self.rec.counting.is_some() {
            return;
        }
        if !self.playing {
            // Reached the end of the song (loop off) or stopped elsewhere.
            self.stop_recording();
            return;
        }
        let raw = self.estimated_position();
        let wrapped = self.is_wrap(self.rec.last_pos, raw);
        let pos = if wrapped {
            raw
        } else {
            raw.max(self.rec.last_pos)
        };
        if wrapped {
            // The loop wrapped: close held notes at the loop end, and snap
            // this pass now that the playhead is past it.
            let end = self.project.length_ticks();
            let held: Vec<Id> = self.rec.held.drain().map(|(_, id)| id).collect();
            for id in held {
                self.finish_note(id, end);
            }
            self.snap_take();
        }
        self.rec.last_pos = pos;
        let held: Vec<Id> = self.rec.held.values().copied().collect();
        if let Some(t) = self.selected.and_then(|id| self.project.track_mut(id)) {
            for n in t.notes.iter_mut().filter(|n| held.contains(&n.id)) {
                n.len = (pos as Tick - n.start).max(1);
            }
            if !held.is_empty() {
                self.touch();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestre_core::{InstrumentChoice, Project};

    /// Pretend the engine just reported the playhead at `tick`.
    fn at(app: &mut OrchestreApp, tick: f64) {
        app.position = tick;
        app.position_time = app.now;
        app.playing = true;
        app.update_recording();
    }

    #[test]
    fn a_take_records_snaps_wraps_and_undoes_as_one_step() {
        let mut p = Project {
            length_bars: 1,
            ..Project::default()
        };
        let t = p.add_track(InstrumentChoice::Piano);
        let mut app = OrchestreApp::build(p, orchestre_audio::Audio::disabled());
        app.select_track(Some(t));
        app.rec.count_in = false;
        app.start_recording();
        assert!(app.rec.active && app.playing);

        // Press slightly late on the second 16th, hold for about a 16th.
        at(&mut app, 250.0);
        app.record_note_on(Source::Key(egui::Key::A), 60, 0.8);
        at(&mut app, 600.0);
        assert_eq!(
            app.project.track(t).unwrap().notes[0].len,
            350,
            "held note grows live"
        );
        app.record_note_off(Source::Key(egui::Key::A));

        // Press just before the loop end, still held when the loop wraps.
        at(&mut app, 3830.0);
        app.record_note_on(Source::Key(egui::Key::S), 62, 0.8);
        at(&mut app, 100.0);

        // Nothing is committed to undo history mid-take.
        app.commit_if_idle(false);
        assert!(!app.history.can_undo());

        app.stop_recording();
        let mut got: Vec<(Tick, Tick, u8)> = app
            .project
            .track(t)
            .unwrap()
            .notes
            .iter()
            .map(|n| (n.start, n.len, n.pitch))
            .collect();
        got.sort();
        assert_eq!(got, vec![(0, 240, 62), (240, 240, 60)]);

        app.commit_if_idle(false);
        app.undo();
        assert!(
            app.project.track(t).unwrap().notes.is_empty(),
            "undo removes the whole take"
        );
    }

    #[test]
    fn playhead_jitter_is_not_a_loop_wrap() {
        let mut p = Project {
            length_bars: 2,
            ..Project::default()
        };
        let t = p.add_track(InstrumentChoice::Synth(orchestre_core::SynthPreset::Bass));
        let mut app = OrchestreApp::build(p, orchestre_audio::Audio::disabled());
        app.select_track(Some(t));
        app.rec.count_in = false;
        app.rec.snap = false;
        app.start_recording();

        at(&mut app, 1000.0);
        app.record_note_on(Source::Key(egui::Key::A), 45, 0.8);
        // The engine's report lands slightly behind the extrapolated playhead.
        at(&mut app, 1100.0);
        at(&mut app, 1090.0);
        at(&mut app, 1200.0);
        app.record_note_off(Source::Key(egui::Key::A));
        // A quick tap whose release is estimated just before its press.
        at(&mut app, 1500.0);
        app.record_note_on(Source::Key(egui::Key::S), 47, 0.8);
        at(&mut app, 1495.0);
        app.record_note_off(Source::Key(egui::Key::S));
        app.stop_recording();

        let lens: Vec<Tick> = app
            .project
            .track(t)
            .unwrap()
            .notes
            .iter()
            .map(|n| n.len)
            .collect();
        assert_eq!(lens.len(), 2);
        assert_eq!(lens[0], 200, "held note stretched: {lens:?}");
        assert!(lens[1] < 100, "quick tap stretched: {lens:?}");
    }

    #[test]
    fn nothing_is_recorded_during_count_in() {
        let mut p = Project::default();
        let t = p.add_track(InstrumentChoice::Drums);
        let mut app = OrchestreApp::build(p, orchestre_audio::Audio::disabled());
        app.select_track(Some(t));
        app.start_recording();
        assert_eq!(app.rec.counting, Some(4));
        app.record_note_on(Source::Key(egui::Key::A), 0, 0.8);
        assert!(app.project.track(t).unwrap().notes.is_empty());
        // Counting in is not "stopped": recording stays armed.
        app.update_recording();
        assert!(app.rec.active);
    }

    #[test]
    fn snapping_rounds_and_wraps_to_loop_start() {
        let mut p = Project::default();
        let t = p.add_track(InstrumentChoice::Piano);
        let bar = p.time_sig.bar_ticks();
        let step = PPQ / 4;
        let notes = [(250, 100, 60), (bar - 20, 300, 64), (1000, 5, 67)];
        let mut ids = HashSet::new();
        for (start, len, pitch) in notes {
            let id = p.new_id();
            ids.insert(id);
            p.track_mut(t).unwrap().notes.push(Note {
                id,
                start,
                len,
                pitch,
                vel: 0.8,
            });
        }
        let track = p.track_mut(t).unwrap();
        snap_notes(track, &ids, step, Some(bar));
        let got: Vec<(Tick, Tick)> = track.notes.iter().map(|n| (n.start, n.len)).collect();
        // Early press before the loop end lands on beat 1; tiny notes get one step.
        assert_eq!(got, vec![(240, step), (0, 240), (960, step)]);
    }
}
