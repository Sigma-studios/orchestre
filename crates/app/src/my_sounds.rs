//! "My sounds": instruments the user tweaked and saved, to reuse in any
//! song. Kept with the app's settings (so it works in the browser too).

use orchestre_core::{ArpParams, Instrument};
use serde::{Deserialize, Serialize};

const STORAGE_KEY: &str = "orchestre_my_sounds";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MySound {
    pub name: String,
    pub instrument: Instrument,
    #[serde(default)]
    pub arp: ArpParams,
}

pub fn load(storage: Option<&dyn eframe::Storage>) -> Vec<MySound> {
    storage
        .and_then(|s| s.get_string(STORAGE_KEY))
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

pub fn save(sounds: &[MySound], storage: &mut dyn eframe::Storage) {
    if let Ok(json) = serde_json::to_string(sounds) {
        storage.set_string(STORAGE_KEY, json);
    }
}

/// Add a sound, replacing one with the same name.
pub fn add(sounds: &mut Vec<MySound>, sound: MySound) {
    match sounds.iter_mut().find(|s| s.name == sound.name) {
        Some(slot) => *slot = sound,
        None => sounds.push(sound),
    }
}
