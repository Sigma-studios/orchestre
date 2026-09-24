use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};
use orchestre_dsp::{Cmd, Engine, Event, Song};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::{Arc, Mutex};

pub struct Backend {
    _stream: cpal::Stream,
    cmds: Producer<Cmd>,
    events: Consumer<Event>,
    garbage: Consumer<Box<Song>>,
    error: Arc<Mutex<Option<String>>>,
}

struct Callback {
    engine: Engine,
    cmds: Consumer<Cmd>,
    events: Producer<Event>,
    garbage: Producer<Box<Song>>,
    scratch: Vec<f32>,
}

impl Callback {
    fn run(&mut self, out: &mut [f32], channels: usize) {
        while let Ok(cmd) = self.cmds.pop() {
            self.engine.handle(cmd);
            if let Some(old) = self.engine.take_garbage() {
                // Freed on the UI thread; if the queue is full, drop it here.
                let _ = self.garbage.push(old);
            }
        }
        self.engine.process_interleaved(out, channels);
        for e in self.engine.drain_events() {
            let _ = self.events.push(e);
        }
    }
}

impl Backend {
    pub fn new() -> Result<Backend, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("no audio output device")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let format = supported.sample_format();
        let config: StreamConfig = supported.into();

        let (cmd_tx, cmd_rx) = RingBuffer::new(1024);
        let (ev_tx, ev_rx) = RingBuffer::new(4096);
        let (gb_tx, gb_rx) = RingBuffer::new(64);
        let cb = Callback {
            engine: Engine::new(config.sample_rate as f32),
            cmds: cmd_rx,
            events: ev_tx,
            garbage: gb_tx,
            scratch: Vec::new(),
        };
        let error = Arc::new(Mutex::new(None));
        let stream = match format {
            SampleFormat::F32 => build::<f32>(&device, &config, cb, error.clone()),
            SampleFormat::I16 => build::<i16>(&device, &config, cb, error.clone()),
            SampleFormat::U16 => build::<u16>(&device, &config, cb, error.clone()),
            SampleFormat::I32 => build::<i32>(&device, &config, cb, error.clone()),
            SampleFormat::F64 => build::<f64>(&device, &config, cb, error.clone()),
            other => return Err(format!("unsupported sample format {other}")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        log::info!(
            "audio: {} Hz, {} channels, {format}",
            config.sample_rate,
            config.channels
        );
        Ok(Backend {
            _stream: stream,
            cmds: cmd_tx,
            events: ev_rx,
            garbage: gb_rx,
            error,
        })
    }

    pub fn send(&mut self, cmd: Cmd) {
        if self.cmds.push(cmd).is_err() {
            log::warn!("audio command queue full");
        }
    }

    pub fn poll_events(&mut self, f: &mut dyn FnMut(Event)) {
        while let Ok(e) = self.events.pop() {
            f(e);
        }
        while self.garbage.pop().is_ok() {}
    }

    pub fn error(&self) -> Option<String> {
        self.error.lock().ok()?.clone()
    }
}

fn build<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut cb: Callback,
    error: Arc<Mutex<Option<String>>>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32> + 'static,
{
    let channels = config.channels as usize;
    device
        .build_output_stream(
            *config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                // Render in f32, then convert. The scratch buffer only grows
                // on the first callbacks, not in steady state.
                if cb.scratch.len() < data.len() {
                    cb.scratch.resize(data.len(), 0.0);
                }
                let mut scratch = std::mem::take(&mut cb.scratch);
                cb.run(&mut scratch[..data.len()], channels);
                for (d, s) in data.iter_mut().zip(&scratch) {
                    *d = T::from_sample(*s);
                }
                cb.scratch = scratch;
            },
            move |err| {
                log::error!("audio stream error: {err}");
                if let Ok(mut e) = error.lock() {
                    *e = Some(err.to_string());
                }
            },
            None,
        )
        .map_err(|e| e.to_string())
}
