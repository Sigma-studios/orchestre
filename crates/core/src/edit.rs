//! Pure note-editing operations, shared by the UI and tests.

use std::collections::HashSet;

use crate::project::{Id, Note, Track};
use crate::theory::Key;
use crate::time::Tick;

/// Notes of `track` whose pitch doesn't belong to `key`.
pub fn notes_outside_key(track: &Track, key: Key) -> Vec<Id> {
    track
        .notes
        .iter()
        .filter(|n| !key.contains(n.pitch))
        .map(|n| n.id)
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyConflict {
    /// Remove out-of-key notes.
    Delete,
    /// Move out-of-key notes to the nearest in-key pitch.
    Snap,
}

/// Lock `track` to `key`, resolving out-of-key notes as requested.
pub fn apply_key_lock(track: &mut Track, key: Key, conflict: KeyConflict) {
    match conflict {
        KeyConflict::Delete => track.notes.retain(|n| key.contains(n.pitch)),
        KeyConflict::Snap => {
            for n in &mut track.notes {
                n.pitch = key.nearest(n.pitch);
            }
            dedup_notes(track);
        }
    }
    track.key_lock = Some(key);
}

/// Remove exact duplicates (same start and pitch), keeping the first.
pub fn dedup_notes(track: &mut Track) {
    let mut seen = HashSet::new();
    track.notes.retain(|n| seen.insert((n.start, n.pitch)));
}

fn clamp_pitch(track: &Track, p: i32) -> u8 {
    let (lo, hi) = track.pitch_range();
    p.clamp(lo as i32, hi as i32) as u8
}

/// Transpose a pitch by `steps`: scale degrees when the track is key-locked,
/// semitones otherwise (drum rows for drum tracks).
pub fn shift_pitch(track: &Track, pitch: u8, steps: i32) -> u8 {
    match track.effective_key() {
        Some(key) => {
            let p = key.step(pitch, steps);
            let (lo, hi) = track.pitch_range();
            if p < lo || p > hi { pitch } else { p }
        }
        None => clamp_pitch(track, pitch as i32 + steps),
    }
}

/// Move the given notes in time and pitch. `dt` is clamped so that no note
/// starts before 0; `dpitch` is interpreted by [`shift_pitch`].
pub fn move_notes(track: &mut Track, ids: &HashSet<Id>, dt: Tick, dpitch: i32) {
    let min_start = track
        .notes
        .iter()
        .filter(|n| ids.contains(&n.id))
        .map(|n| n.start)
        .min()
        .unwrap_or(0);
    let dt = dt.max(-min_start);
    let moved: Vec<(usize, u8)> = track
        .notes
        .iter()
        .enumerate()
        .filter(|(_, n)| ids.contains(&n.id))
        .map(|(i, n)| (i, shift_pitch(track, n.pitch, dpitch)))
        .collect();
    for (i, pitch) in moved {
        let n = &mut track.notes[i];
        n.start += dt;
        n.pitch = pitch;
    }
}

/// Change the length of the given notes by `dlen`, keeping at least `min_len`.
pub fn resize_notes(track: &mut Track, ids: &HashSet<Id>, dlen: Tick, min_len: Tick) {
    for n in track.notes.iter_mut().filter(|n| ids.contains(&n.id)) {
        n.len = (n.len + dlen).max(min_len);
    }
}

pub fn delete_notes(track: &mut Track, ids: &HashSet<Id>) {
    track.notes.retain(|n| !ids.contains(&n.id));
}

/// Notes overlapping the time range `[t0, t1)` with pitch in `[p0, p1]`.
pub fn notes_in_rect(track: &Track, t0: Tick, t1: Tick, p0: u8, p1: u8) -> HashSet<Id> {
    track
        .notes
        .iter()
        .filter(|n| n.start < t1 && n.end() > t0 && n.pitch >= p0 && n.pitch <= p1)
        .map(|n| n.id)
        .collect()
}

/// Copied notes, with starts relative to the earliest one.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Clip {
    pub notes: Vec<Note>,
    /// Whether the notes came from a drum track (pitches are drum rows).
    pub drums: bool,
}

impl Clip {
    pub fn from_selection(track: &Track, ids: &HashSet<Id>) -> Clip {
        let mut notes: Vec<Note> = track
            .notes
            .iter()
            .filter(|n| ids.contains(&n.id))
            .copied()
            .collect();
        let t0 = notes.iter().map(|n| n.start).min().unwrap_or(0);
        for n in &mut notes {
            n.start -= t0;
        }
        Clip {
            notes,
            drums: track.instrument.is_drums(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.notes.is_empty()
    }

    pub fn span(&self) -> Tick {
        self.notes.iter().map(Note::end).max().unwrap_or(0)
    }
}

/// Paste `clip` into `track` starting at `at`. Returns the new note ids.
/// Pitches are constrained to the track's range and key lock.
pub fn paste(track: &mut Track, clip: &Clip, at: Tick, next_id: &mut Id) -> HashSet<Id> {
    let mut ids = HashSet::new();
    // Pasting between drum and pitched tracks makes no musical sense.
    if clip.drums != track.instrument.is_drums() {
        return ids;
    }
    let key = track.effective_key();
    for n in &clip.notes {
        let mut pitch = clamp_pitch(track, n.pitch as i32);
        if let Some(k) = key {
            pitch = k.nearest(pitch);
        }
        let id = *next_id;
        *next_id += 1;
        track.notes.push(Note {
            id,
            start: at + n.start,
            pitch,
            ..*n
        });
        ids.insert(id);
    }
    dedup_notes(track);
    ids.retain(|id| track.note(*id).is_some());
    ids
}

/// Duplicate the selection right after itself; the offset is the selection
/// span rounded up to `step`. Returns the new note ids.
pub fn duplicate(
    track: &mut Track,
    ids: &HashSet<Id>,
    step: Tick,
    next_id: &mut Id,
) -> HashSet<Id> {
    let clip = Clip::from_selection(track, ids);
    if clip.is_empty() {
        return HashSet::new();
    }
    let t0 = track
        .notes
        .iter()
        .filter(|n| ids.contains(&n.id))
        .map(|n| n.start)
        .min()
        .unwrap_or(0);
    let step = step.max(1);
    let span = (clip.span() + step - 1) / step * step;
    paste(track, &clip, t0 + span, next_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instrument::InstrumentChoice;
    use crate::project::Project;
    use crate::theory::Scale;

    const C_MAJ: Key = Key {
        root: 0,
        scale: Scale::Major,
    };

    fn project_with(pitches: &[(Tick, u8)]) -> (Project, Id) {
        let mut p = Project::default();
        let t = p.add_track(InstrumentChoice::Piano);
        for &(start, pitch) in pitches {
            let id = p.new_id();
            p.track_mut(t).unwrap().notes.push(Note {
                id,
                start,
                len: 240,
                pitch,
                vel: 0.8,
            });
        }
        (p, t)
    }

    #[test]
    fn key_lock_conflicts() {
        let (mut p, t) = project_with(&[(0, 60), (240, 61), (480, 66), (720, 67)]);
        let track = p.track_mut(t).unwrap();
        assert_eq!(notes_outside_key(track, C_MAJ).len(), 2);

        let mut deleted = track.clone();
        apply_key_lock(&mut deleted, C_MAJ, KeyConflict::Delete);
        assert_eq!(
            deleted.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
            vec![60, 67]
        );
        assert_eq!(deleted.key_lock, Some(C_MAJ));

        apply_key_lock(track, C_MAJ, KeyConflict::Snap);
        assert_eq!(
            track.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
            vec![60, 60, 65, 67]
        );
    }

    #[test]
    fn move_respects_key_and_zero() {
        let (mut p, t) = project_with(&[(240, 64), (480, 67)]);
        let track = p.track_mut(t).unwrap();
        track.key_lock = Some(C_MAJ);
        let ids: HashSet<Id> = track.notes.iter().map(|n| n.id).collect();
        move_notes(track, &ids, -1000, 1);
        assert_eq!(track.notes[0].start, 0);
        assert_eq!(track.notes[1].start, 240);
        assert_eq!(track.notes[0].pitch, 65); // E -> F by one degree
        assert_eq!(track.notes[1].pitch, 69); // G -> A
    }

    #[test]
    fn copy_paste_and_duplicate() {
        let (mut p, t) = project_with(&[(480, 60), (720, 61)]);
        let mut next = p.next_id;
        let track = p.track_mut(t).unwrap();
        let all: HashSet<Id> = track.notes.iter().map(|n| n.id).collect();
        let clip = Clip::from_selection(track, &all);
        assert_eq!(clip.notes[0].start, 0);
        assert_eq!(clip.span(), 480);

        track.key_lock = Some(C_MAJ);
        let new = paste(track, &clip, 1920, &mut next);
        assert_eq!(new.len(), 2);
        let mut pasted: Vec<_> = track
            .notes
            .iter()
            .filter(|n| new.contains(&n.id))
            .map(|n| (n.start, n.pitch))
            .collect();
        pasted.sort();
        assert_eq!(pasted, vec![(1920, 60), (2160, 60)]);

        let dup = duplicate(track, &new, 960, &mut next);
        let mut d: Vec<_> = track
            .notes
            .iter()
            .filter(|n| dup.contains(&n.id))
            .map(|n| n.start)
            .collect();
        d.sort();
        assert_eq!(d, vec![2880, 3120]);
    }

    #[test]
    fn paste_rejects_drum_mismatch() {
        let (mut p, t) = project_with(&[(0, 60)]);
        let mut next = p.next_id;
        let clip = Clip {
            notes: vec![Note {
                id: 0,
                start: 0,
                len: 10,
                pitch: 0,
                vel: 1.0,
            }],
            drums: true,
        };
        assert!(paste(p.track_mut(t).unwrap(), &clip, 0, &mut next).is_empty());
    }

    #[test]
    fn rect_selection() {
        let (p, t) = project_with(&[(0, 60), (480, 62), (960, 64)]);
        let track = p.track(t).unwrap();
        assert_eq!(notes_in_rect(track, 100, 500, 55, 70).len(), 2);
        assert_eq!(notes_in_rect(track, 0, 2000, 63, 70).len(), 1);
    }
}
