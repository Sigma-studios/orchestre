//! Saving, loading and exporting, native and web.

use std::cell::RefCell;
use std::rc::Rc;

use orchestre_core::Project;
use orchestre_dsp::{Song, render_song};

use crate::app::OrchestreApp;

pub use orchestre_core::file::{EXTENSION, parse_project, project_to_json};

const EXPORT_RATE: u32 = 44100;

/// Render the project to a 16-bit stereo WAV file in memory.
pub fn render_wav(p: &Project) -> Result<Vec<u8>, String> {
    let samples = render_song(&Song::from_project(p), EXPORT_RATE, 6.0);
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: EXPORT_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut w = hound::WavWriter::new(&mut cursor, spec).map_err(|e| e.to_string())?;
        for s in samples {
            w.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
                .map_err(|e| e.to_string())?;
        }
        w.finalize().map_err(|e| e.to_string())?;
    }
    Ok(cursor.into_inner())
}

/// Results of asynchronous file operations (web file picker).
#[derive(Clone, Default)]
pub struct Inbox(Rc<RefCell<Vec<Result<String, String>>>>);

pub fn poll_inbox(app: &mut OrchestreApp) {
    let items: Vec<_> = app.inbox.0.borrow_mut().drain(..).collect();
    for item in items {
        match item.and_then(|json| parse_project(&json)) {
            Ok(p) => {
                app.replace_project(p);
                app.notify("Song opened");
            }
            Err(e) => app.notify(format!("Could not open: {e}")),
        }
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
        app.inbox.0.borrow_mut().push(result);
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

    pub fn open(app: &mut OrchestreApp) {
        let inbox = app.inbox.clone();
        let result = (|| -> Result<(), JsValue> {
            let document = web_sys::window()
                .ok_or("no window")?
                .document()
                .ok_or("no document")?;
            let input: web_sys::HtmlInputElement = document.create_element("input")?.dyn_into()?;
            input.set_type("file");
            input.set_accept(&format!(".{EXTENSION},application/json"));
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
                    inbox.0.borrow_mut().push(item);
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
}

pub use platform::{export_wav, open, save};

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
}
