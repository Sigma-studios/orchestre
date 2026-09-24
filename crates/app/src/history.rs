use orchestre_core::Project;

const MAX_STEPS: usize = 200;

/// Snapshot-based undo/redo. Projects are small (notes are a few bytes
/// each), so storing whole copies keeps this simple and robust.
pub struct History {
    /// The last committed state.
    stable: Project,
    undo: Vec<Project>,
    redo: Vec<Project>,
}

impl History {
    pub fn new(project: &Project) -> Self {
        History {
            stable: project.clone(),
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// Record `project` as a new step if it differs from the last one.
    pub fn commit(&mut self, project: &Project) {
        if *project == self.stable {
            return;
        }
        let prev = std::mem::replace(&mut self.stable, project.clone());
        self.undo.push(prev);
        if self.undo.len() > MAX_STEPS {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Returns the project to restore. `current` is committed first so that
    /// uncommitted edits are not lost to redo.
    pub fn undo(&mut self, current: &Project) -> Option<Project> {
        self.commit(current);
        let prev = self.undo.pop()?;
        let cur = std::mem::replace(&mut self.stable, prev.clone());
        self.redo.push(cur);
        Some(prev)
    }

    pub fn redo(&mut self) -> Option<Project> {
        let next = self.redo.pop()?;
        let cur = std::mem::replace(&mut self.stable, next.clone());
        self.undo.push(cur);
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_roundtrip() {
        let mut p = Project::default();
        let mut h = History::new(&p);
        p.bpm = 100.0;
        h.commit(&p);
        p.bpm = 90.0;
        h.commit(&p);
        h.commit(&p); // no-op
        assert_eq!(h.undo(&p).unwrap().bpm, 100.0);
        assert_eq!(
            h.undo(&Project {
                bpm: 100.0,
                ..p.clone()
            })
            .unwrap()
            .bpm,
            110.0
        );
        assert!(h.undo(&Project::default()).is_none());
        assert_eq!(h.redo().unwrap().bpm, 100.0);
        assert_eq!(h.redo().unwrap().bpm, 90.0);
        assert!(h.redo().is_none());
    }

    #[test]
    fn uncommitted_edit_is_undoable() {
        let p0 = Project::default();
        let mut h = History::new(&p0);
        let p1 = Project {
            bpm: 140.0,
            ..p0.clone()
        };
        // Undo straight after an edit that hasn't been committed yet.
        assert_eq!(h.undo(&p1).unwrap().bpm, p0.bpm);
        assert_eq!(h.redo().unwrap().bpm, 140.0);
    }
}
