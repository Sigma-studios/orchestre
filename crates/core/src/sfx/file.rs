//! The `.orsfx` sound effect file format: one sound as JSON, with a small
//! header. One file per sound, so sounds can be copied between games.

use serde::{Deserialize, Serialize};

use super::Sound;

const FORMAT: &str = "orchestre-sfx";
const VERSION: u32 = 1;
pub const EXTENSION: &str = "orsfx";

#[derive(Serialize, Deserialize)]
struct SoundFile {
    format: String,
    version: u32,
    sound: Sound,
}

pub fn sound_to_json(s: &Sound) -> Result<String, String> {
    let file = SoundFile {
        format: FORMAT.into(),
        version: VERSION,
        sound: s.clone(),
    };
    serde_json::to_string_pretty(&file).map_err(|e| e.to_string())
}

pub fn parse_sound(json: &str) -> Result<Sound, String> {
    let file: SoundFile =
        serde_json::from_str(json).map_err(|e| format!("not a valid sound file: {e}"))?;
    if file.format != FORMAT {
        return Err("not an Orchestre sound effect".into());
    }
    if file.version > VERSION {
        return Err("this sound was made with a newer version of Orchestre".into());
    }
    let mut s = file.sound;
    // Make sure fresh ids never collide with loaded ones.
    let max_id = s.layers.iter().map(|l| l.id).max().unwrap_or(0);
    s.next_id = s.next_id.max(max_id + 1);
    Ok(s)
}

/// Parse raw file bytes (as read from disk or an asset loader).
pub fn parse_sound_bytes(bytes: &[u8]) -> Result<Sound, String> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| "not a valid sound file: not UTF-8".to_string())?;
    parse_sound(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sfx::presets::PRESETS;

    #[test]
    fn json_roundtrip() {
        for preset in PRESETS {
            let s = preset.sound();
            let json = sound_to_json(&s).unwrap();
            assert_eq!(parse_sound(&json).unwrap(), s);
            assert_eq!(parse_sound_bytes(json.as_bytes()).unwrap(), s);
        }
        assert!(parse_sound("{}").is_err());
        assert!(parse_sound_bytes(&[0xff, 0xfe]).is_err());
    }

    #[test]
    fn rejects_songs_and_newer_versions() {
        let song = crate::file::project_to_json(&crate::Project::demo()).unwrap();
        assert!(parse_sound(&song).is_err());
        let mut json: serde_json::Value =
            serde_json::from_str(&sound_to_json(&PRESETS[0].sound()).unwrap()).unwrap();
        json["version"] = (VERSION + 1).into();
        assert!(parse_sound(&json.to_string()).is_err());
    }

    #[test]
    fn stale_next_id_is_repaired() {
        let mut s = PRESETS[0].sound();
        s.next_id = 1;
        let parsed = parse_sound(&sound_to_json(&s).unwrap()).unwrap();
        let max = parsed.layers.iter().map(|l| l.id).max().unwrap();
        assert!(parsed.next_id > max);
    }
}
