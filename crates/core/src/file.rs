//! The `.orch` song file format: the project as JSON, with a small header.

use serde::{Deserialize, Serialize};

use crate::project::Project;

const FORMAT: &str = "orchestre";
const VERSION: u32 = 1;
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
    fn json_roundtrip() {
        let p = Project::demo();
        let json = project_to_json(&p).unwrap();
        assert_eq!(parse_project(&json).unwrap(), p);
        assert_eq!(parse_project_bytes(json.as_bytes()).unwrap(), p);
        assert!(parse_project("{}").is_err());
        assert!(parse_project_bytes(&[0xff, 0xfe]).is_err());
    }
}
