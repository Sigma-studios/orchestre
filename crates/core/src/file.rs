//! The `.orch` song file format: the project as JSON, with a small header.

use serde::{Deserialize, Serialize};

use crate::project::Project;

const FORMAT: &str = "orchestre";
/// 2: the key moved from each track to the song (`Project::key`).
const VERSION: u32 = 2;
pub const EXTENSION: &str = "orch";

#[derive(Serialize, Deserialize)]
struct SongFile {
    format: String,
    version: u32,
    project: Project,
}

pub fn project_to_json(p: &Project) -> Result<String, String> {
    let file = SongFile {
        format: FORMAT.into(),
        version: VERSION,
        project: p.clone(),
    };
    serde_json::to_string_pretty(&file).map_err(|e| e.to_string())
}

pub fn parse_project(json: &str) -> Result<Project, String> {
    let file: SongFile =
        serde_json::from_str(json).map_err(|e| format!("not a valid song file: {e}"))?;
    if file.format != FORMAT {
        return Err("not an Orchestre song".into());
    }
    if file.version > VERSION {
        return Err("this song was made with a newer version of Orchestre".into());
    }
    let mut p = file.project;
    if file.version < 2 {
        migrate_track_keys(&mut p);
    }
    // Make sure fresh ids never collide with loaded ones.
    let max_id = p
        .tracks
        .iter()
        .flat_map(|t| std::iter::once(t.id).chain(t.notes.iter().map(|n| n.id)))
        .max()
        .unwrap_or(0);
    p.next_id = p.next_id.max(max_id + 1);
    Ok(p)
}

/// Version 1 locked each track to its own key. Use the most common one as
/// the song key; tracks locked to it follow it, others become unlocked.
fn migrate_track_keys(p: &mut Project) {
    let mut counts: Vec<(crate::theory::Key, usize)> = Vec::new();
    for k in p.tracks.iter().filter_map(|t| t.key_lock) {
        match counts.iter_mut().find(|(key, _)| *key == k) {
            Some((_, n)) => *n += 1,
            None => counts.push((k, 1)),
        }
    }
    p.key = counts.iter().max_by_key(|(_, n)| *n).map(|(k, _)| *k);
    for t in &mut p.tracks {
        t.follow_key = t.key_lock.is_some() && t.key_lock == p.key;
    }
    p.sync_keys();
}

/// Parse raw file bytes (as read from disk or an asset loader).
pub fn parse_project_bytes(bytes: &[u8]) -> Result<Project, String> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| "not a valid song file: not UTF-8".to_string())?;
    parse_project(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_1_track_keys_become_the_song_key() {
        use crate::theory::{Key, Scale};
        let mut old = Project::demo();
        old.key = None;
        let a_minor = Key::new(9, Scale::Minor);
        // Piano and bass locked to A minor, lead to another key, drums none.
        old.tracks[1].key_lock = Some(a_minor);
        old.tracks[2].key_lock = Some(a_minor);
        old.tracks[3].key_lock = Some(Key::new(2, Scale::Major));
        let mut json: serde_json::Value =
            serde_json::from_str(&project_to_json(&old).unwrap()).unwrap();
        json["version"] = 1.into();
        json["project"].as_object_mut().unwrap().remove("key");
        for t in json["project"]["tracks"].as_array_mut().unwrap() {
            t.as_object_mut().unwrap().remove("follow_key");
        }
        let p = parse_project(&json.to_string()).unwrap();
        assert_eq!(p.key, Some(a_minor));
        let follow: Vec<bool> = p.tracks.iter().map(|t| t.follow_key).collect();
        assert_eq!(follow, [false, true, true, false]);
        assert_eq!(p.tracks[3].key_lock, None);
        assert_eq!(p.tracks[1].key_lock, Some(a_minor));
    }

    #[test]
    fn json_roundtrip() {
        let p = Project::demo();
        let json = project_to_json(&p).unwrap();
        assert_eq!(parse_project(&json).unwrap(), p);
        assert_eq!(parse_project_bytes(json.as_bytes()).unwrap(), p);
        assert!(parse_project("{}").is_err());
        assert!(parse_project_bytes(&[0xff, 0xfe]).is_err());
    }
}
