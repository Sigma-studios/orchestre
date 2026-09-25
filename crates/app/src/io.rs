//! Saving, loading and exporting, native and web.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use orchestre_core::Project;
use orchestre_core::sfx::file::{EXTENSION as SOUND_EXTENSION, parse_sound, sound_to_json};
use orchestre_core::sfx::{PlayOpts, Sound};
use orchestre_dsp::{Song, render_song, render_sound};

use crate::app::OrchestreApp;

pub use orchestre_core::file::{EXTENSION, parse_project, project_to_json};

const EXPORT_RATE: u32 = 44100;
/// How many files "Export variations" writes.
pub const VARIATIONS: usize = 8;

/// Interleaved stereo samples as a 16-bit WAV file in memory.
fn wav_bytes(samples: &[f32]) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: EXPORT_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut w = hound::WavWriter::new(&mut cursor, spec).map_err(|e| e.to_string())?;
        for &s in samples {
            w.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
                .map_err(|e| e.to_string())?;
        }
        w.finalize().map_err(|e| e.to_string())?;
    }
    Ok(cursor.into_inner())
}

/// Render the project to a 16-bit stereo WAV file in memory.
pub fn render_wav(p: &Project) -> Result<Vec<u8>, String> {
    wav_bytes(&render_song(&Song::from_project(p), EXPORT_RATE, 6.0))
}

/// Render one play of a sound to a WAV file in memory.
pub fn render_sound_wav(s: &Sound, opts: &PlayOpts) -> Result<Vec<u8>, String> {
    wav_bytes(&render_sound(s, opts, EXPORT_RATE))
}

/// `count` different plays of the sound, as (file name, WAV bytes). Engines
/// that pick randomly among variants get variation without Orchestre.
pub fn render_variations(
    s: &Sound,
    opts: PlayOpts,
    count: usize,
) -> Result<Vec<(String, Vec<u8>)>, String> {
    let stem = crate::sfx::file_stem(&s.name);
    (1..=count)
        .map(|i| {
            let opts = PlayOpts {
                seed: opts.seed.wrapping_add(i as u32 * 7919),
                ..opts
            };
            Ok((format!("{stem}_{i:02}.wav"), render_sound_wav(s, &opts)?))
        })
        .collect()
}

/// Results of file operations, handled on the next frame.
pub enum Incoming {
    Song(Result<String, String>),
    /// The file's path (desktop only) and its contents.
    Sound(Result<(Option<PathBuf>, String), String>),
}

#[derive(Clone, Default)]
pub struct Inbox(Rc<RefCell<Vec<Incoming>>>);

impl Inbox {
    fn push(&self, item: Incoming) {
        self.0.borrow_mut().push(item);
    }
}

pub fn poll_inbox(app: &mut OrchestreApp) {
    let items: Vec<_> = app.inbox.0.borrow_mut().drain(..).collect();
    for item in items {
        match item {
            Incoming::Song(r) => match r.and_then(|json| parse_project(&json)) {
                Ok(p) => {
                    app.replace_project(p);
                    app.notify("Song opened");
                }
                Err(e) => app.notify(format!("Could not open: {e}")),
            },
            Incoming::Sound(r) => {
                match r.and_then(|(path, json)| Ok((parse_sound(&json)?, path))) {
                    Ok((s, path)) => {
                        let name = s.name.clone();
                        app.request_sound(s, path);
                        app.notify(format!("Opened “{name}”"));
                    }
                    Err(e) => app.notify(format!("Could not open: {e}")),
                }
            }
        }
    }
}

/// Ask which preset (or nothing) to start a new sound from. It goes in the
/// folder of the sound being edited, else the top of the sound folder.
pub fn new_sound(app: &mut OrchestreApp) {
    #[cfg(not(target_arch = "wasm32"))]
    let dir = app.sfx.library.as_ref().map(|lib| {
        app.sfx
            .path
            .as_ref()
            .and_then(|p| p.parent())
            .filter(|d| d.starts_with(lib.dir()))
            .map_or_else(|| lib.dir().clone(), PathBuf::from)
    });
    #[cfg(target_arch = "wasm32")]
    let dir = None;
    app.sfx.action = Some(crate::sfx::FileAction::NewSound(dir, String::new()));
}

/// Start a sound: saved right away as a new file in `dir`, or unsaved.
pub fn create_sound(app: &mut OrchestreApp, dir: Option<PathBuf>, mut sound: Sound) {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(dir) = dir {
        let (name, path) = crate::sfx::free_name(&dir, &sound.name);
        sound.name = name;
        let result = sound_to_json(&sound)
            .and_then(|json| std::fs::write(&path, json).map_err(|e| e.to_string()));
        match result {
            Ok(()) => {
                app.notify(format!("Created {}", path.display()));
                platform::refresh_library(app);
                app.request_sound(sound, Some(path));
            }
            Err(e) => app.notify(format!("Could not create the sound: {e}")),
        }
        return;
    }
    #[cfg(target_arch = "wasm32")]
    let _ = dir;
    if sound.name.trim().is_empty() {
        sound.name = "New sound".into();
    }
    app.request_sound(sound, None);
}

/// Record that the current sound is saved as it is now.
fn saved_sound(app: &mut OrchestreApp, path: Option<PathBuf>) {
    app.sfx.baseline = app.sfx.sound.clone();
    if path.is_some() {
        app.sfx.path = path;
    }
}

const FILE_STEM: &str = "my-song";

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    use super::*;

    pub fn save(app: &mut OrchestreApp) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Orchestre song", &[EXTENSION])
            .set_file_name(format!("{}.{EXTENSION}", FILE_STEM))
            .save_file()
        else {
            return;
        };
        let result = project_to_json(&app.project)
            .and_then(|json| std::fs::write(&path, json).map_err(|e| e.to_string()));
        match result {
            Ok(()) => app.notify(format!("Saved {}", path.display())),
            Err(e) => app.notify(format!("Could not save: {e}")),
        }
    }

    pub fn open(app: &mut OrchestreApp) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Orchestre song", &[EXTENSION])
            .pick_file()
        else {
            return;
        };
        let result = std::fs::read_to_string(&path).map_err(|e| e.to_string());
        app.inbox.push(Incoming::Song(result));
    }

    pub fn export_wav(app: &mut OrchestreApp) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("WAV audio", &["wav"])
            .set_file_name(format!("{}.wav", FILE_STEM))
            .save_file()
        else {
            return;
        };
        let result = render_wav(&app.project)
            .and_then(|bytes| std::fs::write(&path, bytes).map_err(|e| e.to_string()));
        match result {
            Ok(()) => app.notify(format!("Exported {}", path.display())),
            Err(e) => app.notify(format!("Export failed: {e}")),
        }
    }

    /// Where file dialogs for sounds start: the library folder, else the
    /// current sound's folder.
    fn sound_dir(app: &OrchestreApp) -> Option<PathBuf> {
        app.sfx
            .library
            .as_ref()
            .map(|l| l.dir().clone())
            .or_else(|| app.sfx.path.as_ref()?.parent().map(PathBuf::from))
    }

    fn dialog(app: &OrchestreApp) -> rfd::FileDialog {
        let d = rfd::FileDialog::new();
        match sound_dir(app) {
            Some(dir) => d.set_directory(dir),
            None => d,
        }
    }

    pub fn open_sound(app: &mut OrchestreApp) {
        let Some(path) = dialog(app)
            .add_filter("Orchestre sound effect", &[SOUND_EXTENSION])
            .pick_file()
        else {
            return;
        };
        open_sound_file(app, path);
    }

    pub fn open_sound_file(app: &mut OrchestreApp, path: PathBuf) {
        let result = std::fs::read_to_string(&path)
            .map(|text| (Some(path), text))
            .map_err(|e| e.to_string());
        app.inbox.push(Incoming::Sound(result));
    }

    /// Save to the sound's file, or ask where. Returns whether it was saved.
    pub fn save_sound(app: &mut OrchestreApp) -> bool {
        match app.sfx.path.clone() {
            Some(path) => write_sound(app, path),
            None => save_sound_as(app),
        }
    }

    pub fn save_sound_as(app: &mut OrchestreApp) -> bool {
        let Some(path) = dialog(app)
            .add_filter("Orchestre sound effect", &[SOUND_EXTENSION])
            .set_file_name(format!("{}.{SOUND_EXTENSION}", app.sfx.file_stem()))
            .save_file()
        else {
            return false;
        };
        write_sound(app, path)
    }

    fn write_sound(app: &mut OrchestreApp, path: PathBuf) -> bool {
        let result = sound_to_json(&app.sfx.sound)
            .and_then(|json| std::fs::write(&path, json).map_err(|e| e.to_string()));
        match result {
            Ok(()) => {
                app.notify(format!("Saved {}", path.display()));
                saved_sound(app, Some(path));
                refresh_library(app);
                true
            }
            Err(e) => {
                app.notify(format!("Could not save: {e}"));
                false
            }
        }
    }

    /// Save every sound with unsaved changes. Returns whether all were
    /// saved (a sound never saved asks where, and that can be cancelled).
    pub fn save_all_sounds(app: &mut OrchestreApp) -> bool {
        let mut ok = true;
        for (path, draft) in std::mem::take(&mut app.sfx.drafts) {
            let result = sound_to_json(&draft.sound)
                .and_then(|json| std::fs::write(&path, json).map_err(|e| e.to_string()));
            if let Err(e) = result {
                app.notify(format!("Could not save {}: {e}", path.display()));
                app.sfx.drafts.insert(path, draft);
                ok = false;
            }
        }
        if app.sfx.dirty() && !app.sfx.sound.layers.is_empty() {
            ok &= save_sound(app);
        }
        refresh_library(app);
        ok
    }

    pub fn refresh_library(app: &mut OrchestreApp) {
        if let Some(lib) = &mut app.sfx.library {
            lib.refresh();
        }
    }

    fn read_sound(path: &std::path::Path) -> Result<Sound, String> {
        std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|json| parse_sound(&json))
    }

    /// Copy a sound file next to itself ("Laser 2") and open the copy.
    pub fn duplicate_sound(app: &mut OrchestreApp, path: PathBuf) {
        match read_sound(&path) {
            Ok(sound) => {
                let dir = path.parent().map(PathBuf::from).unwrap_or_default();
                create_sound(app, Some(dir), sound);
            }
            Err(e) => app.notify(format!("Could not copy: {e}")),
        }
    }

    /// Give a sound a new name, renaming its file to match. Returns
    /// whether it was renamed.
    pub fn rename_sound(app: &mut OrchestreApp, path: PathBuf, name: &str) -> bool {
        let name = name.trim();
        let stem = crate::sfx::file_stem(name);
        let new_path = path.with_file_name(format!("{stem}.{SOUND_EXTENSION}"));
        if name.is_empty() {
            app.notify("A sound needs a name");
            return false;
        }
        if new_path != path && new_path.exists() {
            app.notify(format!(
                "There is already a sound called “{name}” in this folder ({stem}.{SOUND_EXTENSION})"
            ));
            return false;
        }
        let current = app.sfx.path.as_ref() == Some(&path);
        // The file keeps what was saved; only its name changes.
        let result = read_sound(&path).and_then(|mut sound| {
            sound.name = name.to_string();
            let json = sound_to_json(&sound)?;
            std::fs::write(&new_path, json).map_err(|e| e.to_string())?;
            if new_path != path {
                std::fs::remove_file(&path).map_err(|e| e.to_string())?;
            }
            Ok(())
        });
        match result {
            Ok(()) => {
                if current {
                    app.sfx.path = Some(new_path);
                    app.sfx.sound.name = name.to_string();
                    app.sfx.baseline.name = name.to_string();
                } else if let Some(mut draft) = app.sfx.drafts.remove(&path) {
                    draft.sound.name = name.to_string();
                    draft.baseline.name = name.to_string();
                    app.sfx.drafts.insert(new_path, draft);
                }
                refresh_library(app);
                true
            }
            Err(e) => {
                app.notify(format!("Could not rename: {e}"));
                false
            }
        }
    }

    pub fn delete_sound(app: &mut OrchestreApp, path: PathBuf) {
        match std::fs::remove_file(&path) {
            Ok(()) => {
                app.sfx.drafts.remove(&path);
                if app.sfx.path.as_ref() == Some(&path) {
                    app.set_sound(Sound::default(), None);
                }
                app.notify(format!("Deleted {}", path.display()));
                refresh_library(app);
            }
            Err(e) => app.notify(format!("Could not delete: {e}")),
        }
    }

    pub fn create_folder(app: &mut OrchestreApp, parent: PathBuf, name: &str) {
        let name = name.trim();
        if name.is_empty() || name.contains(['/', '\\']) {
            return;
        }
        match std::fs::create_dir(parent.join(name)) {
            Ok(()) => refresh_library(app),
            Err(e) => app.notify(format!("Could not create the folder: {e}")),
        }
    }

    /// Show a file or folder in the system's file browser.
    pub fn reveal(app: &mut OrchestreApp, path: &std::path::Path) {
        use std::process::Command;
        let result = if cfg!(target_os = "macos") {
            Command::new("open").arg("-R").arg(path).spawn()
        } else if cfg!(target_os = "windows") {
            Command::new("explorer")
                .arg(format!("/select,{}", path.display()))
                .spawn()
        } else {
            let dir = if path.is_dir() {
                path
            } else {
                path.parent().unwrap_or(path)
            };
            Command::new("xdg-open").arg(dir).spawn()
        };
        if let Err(e) = result {
            app.notify(format!("Could not open the file browser: {e}"));
        }
    }

    pub fn choose_sound_folder(app: &mut OrchestreApp) {
        if let Some(dir) = dialog(app).pick_folder() {
            app.sfx.library = Some(crate::sfx::Library::open(dir));
        }
    }

    pub fn export_sound_wav(app: &mut OrchestreApp) {
        let Some(path) = dialog(app)
            .add_filter("WAV audio", &["wav"])
            .set_file_name(format!("{}.wav", app.sfx.file_stem()))
            .save_file()
        else {
            return;
        };
        let opts = app.sfx.next_opts();
        let result = render_sound_wav(&app.sfx.sound, &opts)
            .and_then(|bytes| std::fs::write(&path, bytes).map_err(|e| e.to_string()));
        match result {
            Ok(()) => app.notify(format!("Exported {}", path.display())),
            Err(e) => app.notify(format!("Export failed: {e}")),
        }
    }

    pub fn export_sound_variations(app: &mut OrchestreApp) {
        let Some(dir) = dialog(app).pick_folder() else {
            return;
        };
        let opts = app.sfx.next_opts();
        let result = render_variations(&app.sfx.sound, opts, VARIATIONS).and_then(|files| {
            for (name, bytes) in files {
                std::fs::write(dir.join(name), bytes).map_err(|e| e.to_string())?;
            }
            Ok(())
        });
        match result {
            Ok(()) => app.notify(format!(
                "Exported {VARIATIONS} variations to {}",
                dir.display()
            )),
            Err(e) => app.notify(format!("Export failed: {e}")),
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod platform {
    use super::*;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;

    fn download(bytes: &[u8], name: &str, mime: &str) -> Result<(), JsValue> {
        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;
        let array = js_sys::Uint8Array::from(bytes);
        let parts = js_sys::Array::of1(&array);
        let opts = web_sys::BlobPropertyBag::new();
        opts.set_type(mime);
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts)?;
        let url = web_sys::Url::create_object_url_with_blob(&blob)?;
        let a: web_sys::HtmlAnchorElement = document.create_element("a")?.dyn_into()?;
        a.set_href(&url);
        a.set_download(name);
        a.click();
        web_sys::Url::revoke_object_url(&url)?;
        Ok(())
    }

    pub fn save(app: &mut OrchestreApp) {
        let name = format!("{}.{EXTENSION}", FILE_STEM);
        match project_to_json(&app.project) {
            Ok(json) => match download(json.as_bytes(), &name, "application/json") {
                Ok(()) => app.notify("Song downloaded"),
                Err(e) => app.notify(format!("Could not save: {e:?}")),
            },
            Err(e) => app.notify(format!("Could not save: {e}")),
        }
    }

    /// Ask the browser for a file; its text arrives in the inbox.
    fn pick_file(
        app: &mut OrchestreApp,
        accept: &str,
        wrap: fn(Result<String, String>) -> Incoming,
    ) {
        let inbox = app.inbox.clone();
        let result = (|| -> Result<(), JsValue> {
            let document = web_sys::window()
                .ok_or("no window")?
                .document()
                .ok_or("no document")?;
            let input: web_sys::HtmlInputElement = document.create_element("input")?.dyn_into()?;
            input.set_type("file");
            input.set_accept(accept);
            let input2 = input.clone();
            let onchange = Closure::once_into_js(move || {
                let Some(file) = input2.files().and_then(|f| f.get(0)) else {
                    return;
                };
                wasm_bindgen_futures::spawn_local(async move {
                    let text = wasm_bindgen_futures::JsFuture::from(file.text()).await;
                    let item = text
                        .map(|t| t.as_string().unwrap_or_default())
                        .map_err(|e| format!("{e:?}"));
                    inbox.push(wrap(item));
                });
            });
            input.set_onchange(Some(onchange.unchecked_ref()));
            input.click();
            Ok(())
        })();
        if let Err(e) = result {
            app.notify(format!("Could not open file picker: {e:?}"));
        }
    }

    pub fn open(app: &mut OrchestreApp) {
        pick_file(
            app,
            &format!(".{EXTENSION},application/json"),
            Incoming::Song,
        );
    }

    pub fn export_wav(app: &mut OrchestreApp) {
        let name = format!("{}.wav", FILE_STEM);
        match render_wav(&app.project) {
            Ok(bytes) => match download(&bytes, &name, "audio/wav") {
                Ok(()) => app.notify("WAV downloaded"),
                Err(e) => app.notify(format!("Export failed: {e:?}")),
            },
            Err(e) => app.notify(format!("Export failed: {e}")),
        }
    }

    pub fn open_sound(app: &mut OrchestreApp) {
        pick_file(app, &format!(".{SOUND_EXTENSION},application/json"), |r| {
            Incoming::Sound(r.map(|text| (None, text)))
        });
    }

    pub fn save_sound(app: &mut OrchestreApp) -> bool {
        let name = format!("{}.{SOUND_EXTENSION}", app.sfx.file_stem());
        let result = sound_to_json(&app.sfx.sound).and_then(|json| {
            download(json.as_bytes(), &name, "application/json").map_err(|e| format!("{e:?}"))
        });
        match result {
            Ok(()) => {
                app.notify("Sound downloaded");
                saved_sound(app, None);
                true
            }
            Err(e) => {
                app.notify(format!("Could not save: {e}"));
                false
            }
        }
    }

    pub fn save_sound_as(app: &mut OrchestreApp) -> bool {
        save_sound(app)
    }

    pub fn export_sound_wav(app: &mut OrchestreApp) {
        let name = format!("{}.wav", app.sfx.file_stem());
        let opts = app.sfx.next_opts();
        match render_sound_wav(&app.sfx.sound, &opts) {
            Ok(bytes) => match download(&bytes, &name, "audio/wav") {
                Ok(()) => app.notify("WAV downloaded"),
                Err(e) => app.notify(format!("Export failed: {e:?}")),
            },
            Err(e) => app.notify(format!("Export failed: {e}")),
        }
    }

    pub fn export_sound_variations(app: &mut OrchestreApp) {
        let opts = app.sfx.next_opts();
        let result = render_variations(&app.sfx.sound, opts, VARIATIONS).and_then(|files| {
            for (name, bytes) in files {
                download(&bytes, &name, "audio/wav").map_err(|e| format!("{e:?}"))?;
            }
            Ok(())
        });
        match result {
            Ok(()) => app.notify(format!("{VARIATIONS} WAV files downloaded")),
            Err(e) => app.notify(format!("Export failed: {e}")),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use platform::{
    choose_sound_folder, create_folder, delete_sound, duplicate_sound, open_sound_file,
    rename_sound, reveal, save_all_sounds,
};
pub use platform::{
    export_sound_variations, export_sound_wav, export_wav, open, open_sound, save, save_sound,
    save_sound_as,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_export() {
        let mut p = Project::demo();
        p.length_bars = 1;
        for t in &mut p.tracks {
            t.notes.retain(|n| n.end() <= p.time_sig.bar_ticks());
        }
        let bytes = render_wav(&p).unwrap();
        let reader = hound::WavReader::new(std::io::Cursor::new(bytes)).unwrap();
        assert_eq!(reader.spec().channels, 2);
        assert!(reader.duration() > EXPORT_RATE); // > 1s
    }

    #[test]
    fn sound_variations_export() {
        let s = orchestre_core::sfx::presets::find("Step on gravel")
            .unwrap()
            .sound();
        let files = render_variations(&s, PlayOpts::default(), 3).unwrap();
        let names: Vec<_> = files.iter().map(|f| f.0.as_str()).collect();
        assert_eq!(
            names,
            [
                "step-on-gravel_01.wav",
                "step-on-gravel_02.wav",
                "step-on-gravel_03.wav"
            ]
        );
        assert_ne!(files[0].1, files[1].1, "variations differ");
        let reader = hound::WavReader::new(std::io::Cursor::new(&files[0].1)).unwrap();
        assert_eq!(reader.spec().channels, 2);
        assert!(reader.duration() > EXPORT_RATE / 20);
    }
}
