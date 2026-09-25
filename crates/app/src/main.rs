//! Orchestre: a beginner-friendly music maker where every sound is synthesized.

mod app;
mod history;
mod input;
mod io;
#[cfg(not(target_arch = "wasm32"))]
mod midi;
mod record;
mod settings;
mod sfx;
mod theme;
mod ui;

use app::OrchestreApp;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Orchestre")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([1220.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Orchestre",
        options,
        Box::new(|cc| Ok(Box::new(OrchestreApp::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use wasm_bindgen::JsCast;
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("no window")
            .document()
            .expect("no document");
        let canvas = document
            .get_element_by_id("orchestre_canvas")
            .expect("missing #orchestre_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#orchestre_canvas is not a canvas");
        let result = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(OrchestreApp::new(cc)))),
            )
            .await;
        if let Some(loading) = document.get_element_by_id("loading") {
            match result {
                Ok(()) => loading.remove(),
                Err(e) => {
                    loading.set_inner_html(&format!("<p>Orchestre failed to start: {e:?}</p>"))
                }
            }
        }
    });
}
