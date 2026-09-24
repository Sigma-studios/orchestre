//! C-ABI wrapper around the engine for the browser AudioWorklet.
//! See `web/worklet.js` for the other side of this interface.

use orchestre_dsp::{Cmd, Engine, Event};

pub struct Worklet {
    engine: Engine,
    out: Vec<f32>,
    pending: Vec<Event>,
    encoded: Vec<u8>,
}

/// # Safety
/// Called once by the worklet; the pointer is passed back to every other call.
#[unsafe(no_mangle)]
pub extern "C" fn engine_new(sample_rate: f32) -> *mut Worklet {
    Box::into_raw(Box::new(Worklet {
        engine: Engine::new(sample_rate),
        out: vec![0.0; 256],
        pending: Vec::with_capacity(256),
        encoded: Vec::with_capacity(4096),
    }))
}

/// Allocate `len` bytes for the worklet to copy a command into.
#[unsafe(no_mangle)]
pub extern "C" fn buf_alloc(len: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len);
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// Decode and apply a postcard-encoded [`Cmd`]; frees the buffer.
///
/// # Safety
/// `ptr` must come from [`buf_alloc`] with the same `len`, fully written.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn engine_cmd(w: *mut Worklet, ptr: *mut u8, len: usize) {
    let w = unsafe { &mut *w };
    let bytes = unsafe { Vec::from_raw_parts(ptr, len, len) };
    if let Ok(cmd) = postcard::from_bytes::<Cmd>(&bytes) {
        w.engine.handle(cmd);
        drop(w.engine.take_garbage());
    }
}

/// Render `frames` frames; returns a pointer to `frames` left samples
/// followed by `frames` right samples.
///
/// # Safety
/// `w` must come from [`engine_new`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn engine_process(w: *mut Worklet, frames: usize) -> *const f32 {
    let w = unsafe { &mut *w };
    if w.out.len() < frames * 2 {
        w.out.resize(frames * 2, 0.0);
    }
    let (l, r) = w.out.split_at_mut(frames);
    w.engine.process(l, &mut r[..frames]);
    w.pending.extend(w.engine.drain_events());
    w.out.as_ptr()
}

/// Encode pending events; returns the byte length (0 if none). The bytes
/// are then available at [`engine_events_ptr`].
///
/// # Safety
/// `w` must come from [`engine_new`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn engine_events(w: *mut Worklet) -> usize {
    let w = unsafe { &mut *w };
    if w.pending.is_empty() {
        return 0;
    }
    w.encoded = postcard::to_allocvec(&w.pending).unwrap_or_default();
    w.pending.clear();
    w.encoded.len()
}

/// # Safety
/// `w` must come from [`engine_new`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn engine_events_ptr(w: *mut Worklet) -> *const u8 {
    unsafe { (*w).encoded.as_ptr() }
}
