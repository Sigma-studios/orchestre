// AudioWorklet side of Orchestre's web audio backend. Runs the Rust engine
// (engine.wasm, built from crates/dsp-worklet) on the audio rendering thread.
class OrchestreProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();
    try {
      const module = new WebAssembly.Module(options.processorOptions.wasm);
      this.wasm = new WebAssembly.Instance(module, {}).exports;
      this.engine = this.wasm.engine_new(sampleRate);
    } catch (e) {
      this.port.postMessage(String(e));
      this.wasm = null;
      return;
    }
    this.port.onmessage = (e) => {
      const bytes = e.data;
      const ptr = this.wasm.buf_alloc(bytes.length);
      new Uint8Array(this.wasm.memory.buffer, ptr, bytes.length).set(bytes);
      this.wasm.engine_cmd(this.engine, ptr, bytes.length);
    };
  }

  process(_inputs, outputs) {
    if (!this.wasm) return true;
    const out = outputs[0];
    const n = out[0].length;
    const ptr = this.wasm.engine_process(this.engine, n);
    // Views must be created after the call: memory may have grown.
    const mem = this.wasm.memory.buffer;
    out[0].set(new Float32Array(mem, ptr, n));
    if (out.length > 1) out[1].set(new Float32Array(mem, ptr + n * 4, n));
    const len = this.wasm.engine_events(this.engine);
    if (len > 0) {
      const evPtr = this.wasm.engine_events_ptr(this.engine);
      this.port.postMessage(new Uint8Array(this.wasm.memory.buffer, evPtr, len).slice());
    }
    return true;
  }
}

registerProcessor("orchestre", OrchestreProcessor);
