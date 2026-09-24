//! Browser backend: the engine runs in an AudioWorklet (see `web/worklet.js`
//! and the `orchestre-dsp-worklet` crate), isolated from UI jank on the main
//! thread. Commands and events cross over the node's MessagePort as postcard
//! bytes, so no SharedArrayBuffer (and no special HTTP headers) is needed.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use js_sys::{Array, ArrayBuffer, Object, Reflect, Uint8Array};
use orchestre_dsp::{Cmd, Event};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AudioContext, AudioContextState, AudioWorkletNode, AudioWorkletNodeOptions, MessageEvent,
    Response,
};

#[derive(Default)]
struct Shared {
    node: Option<AudioWorkletNode>,
    queue: Vec<Cmd>,
    events: VecDeque<Event>,
    error: Option<String>,
    _onmessage: Option<Closure<dyn FnMut(MessageEvent)>>,
}

pub struct Backend {
    ctx: AudioContext,
    shared: Rc<RefCell<Shared>>,
}

fn js_err(e: JsValue) -> String {
    e.as_string()
        .or_else(|| js_sys::JSON::stringify(&e).ok().and_then(|s| s.as_string()))
        .unwrap_or_else(|| format!("{e:?}"))
}

fn post(node: &AudioWorkletNode, cmd: &Cmd) {
    match postcard::to_allocvec(cmd) {
        Ok(bytes) => {
            if let Ok(port) = node.port() {
                let arr = Uint8Array::from(bytes.as_slice());
                let _ = port.post_message(&arr);
            }
        }
        Err(e) => log::error!("encode cmd: {e}"),
    }
}

async fn setup(ctx: AudioContext, shared: Rc<RefCell<Shared>>) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let worklet = ctx.audio_worklet()?;
    JsFuture::from(worklet.add_module("./worklet.js")?).await?;
    let resp: Response = JsFuture::from(window.fetch_with_str("./engine.wasm"))
        .await?
        .dyn_into()?;
    if !resp.ok() {
        return Err(format!("engine.wasm: HTTP {}", resp.status()).into());
    }
    let bytes: ArrayBuffer = JsFuture::from(resp.array_buffer()?).await?.dyn_into()?;

    let proc_opts = Object::new();
    Reflect::set(&proc_opts, &"wasm".into(), &bytes)?;
    let opts = AudioWorkletNodeOptions::new();
    opts.set_number_of_inputs(0);
    opts.set_number_of_outputs(1);
    opts.set_output_channel_count(&Array::of1(&2.into()));
    opts.set_processor_options(Some(&proc_opts));
    let node = AudioWorkletNode::new_with_options(&ctx, "orchestre", &opts)?;
    node.connect_with_audio_node(&ctx.destination())?;

    let weak = Rc::downgrade(&shared);
    let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
        let Some(shared) = weak.upgrade() else { return };
        let data = e.data();
        if let Some(msg) = data.as_string() {
            log::error!("audio worklet: {msg}");
            shared.borrow_mut().error = Some(msg);
            return;
        }
        let bytes = Uint8Array::new(&data).to_vec();
        match postcard::from_bytes::<Vec<Event>>(&bytes) {
            Ok(evs) => shared.borrow_mut().events.extend(evs),
            Err(err) => log::error!("decode events: {err}"),
        }
    });
    node.port()?
        .set_onmessage(Some(onmessage.as_ref().unchecked_ref()));

    let mut s = shared.borrow_mut();
    for cmd in s.queue.drain(..) {
        post(&node, &cmd);
    }
    s.node = Some(node);
    s._onmessage = Some(onmessage);
    Ok(())
}

impl Backend {
    pub fn new() -> Result<Backend, String> {
        let ctx = AudioContext::new().map_err(js_err)?;
        let shared = Rc::new(RefCell::new(Shared::default()));
        let (c, s) = (ctx.clone(), shared.clone());
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(e) = setup(c, s.clone()).await {
                let msg = js_err(e);
                log::error!("audio setup failed: {msg}");
                s.borrow_mut().error = Some(msg);
            }
        });
        Ok(Backend { ctx, shared })
    }

    /// Browsers start audio suspended until the user interacts with the page.
    fn ensure_running(&self) {
        if self.ctx.state() != AudioContextState::Running {
            let _ = self.ctx.resume();
        }
    }

    pub fn send(&mut self, cmd: Cmd) {
        if matches!(cmd, Cmd::Play | Cmd::LiveNoteOn { .. }) {
            self.ensure_running();
        }
        let mut s = self.shared.borrow_mut();
        match &s.node {
            Some(node) => post(node, &cmd),
            None => {
                // Only the latest song matters while we're still loading.
                if matches!(cmd, Cmd::SetSong(_)) {
                    s.queue.retain(|c| !matches!(c, Cmd::SetSong(_)));
                }
                s.queue.push(cmd);
            }
        }
    }

    pub fn poll_events(&mut self, f: &mut dyn FnMut(Event)) {
        let evs: Vec<Event> = self.shared.borrow_mut().events.drain(..).collect();
        for e in evs {
            f(e);
        }
    }

    pub fn error(&self) -> Option<String> {
        self.shared.borrow().error.clone()
    }
}
