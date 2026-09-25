//! Sound effects (`.orsfx` files), synthesized live: every play is a little
//! different, and a whole library of sounds costs a few kilobytes.
//!
//! The simplest way to play one is the same as any Bevy audio:
//!
//! ```no_run
//! # use bevy::prelude::*;
//! # use orchestre_bevy::OrchestreSound;
//! # fn system(mut commands: Commands, assets: Res<AssetServer>) {
//! let boom: Handle<OrchestreSound> = assets.load("sfx/explosion.orsfx");
//! commands.spawn((AudioPlayer(boom), PlaybackSettings::DESPAWN));
//! # }
//! ```
//!
//! [`PlaySfx`] adds what games usually need on top: intensity, pitch, a
//! fixed variation (seed), and voice limits so a burst of the same sound
//! doesn't pile up. It plays recorded files (`.ogg`, `.wav`…) too, with
//! random pitch and volume variation, so a synthesized placeholder can be
//! swapped for a recording without changing any code:
//!
//! ```no_run
//! # use bevy::prelude::*;
//! # use orchestre_bevy::{OrchestreSound, PlaySfx};
//! # fn system(mut commands: Commands, assets: Res<AssetServer>) {
//! let step: Handle<OrchestreSound> = assets.load("sfx/step.orsfx");
//! commands.spawn(PlaySfx::new(step).intensity(0.4));
//!
//! let door: Handle<AudioSource> = assets.load("sfx/door.ogg");
//! commands.spawn(PlaySfx::new(door));
//! # }
//! ```

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, LoadContext, UntypedAssetId};
use bevy::audio::{AudioSource, ChannelCount, Decodable, Sample, SampleRate, Source, Volume};
use bevy::prelude::*;
use orchestre_core::sfx::{PlayOpts, Sound, intensity_gain};
use orchestre_dsp::SoundPlayer;

use crate::{BLOCK_FRAMES, SAMPLE_RATE};

/// A new seed for every play, so each one varies.
fn next_seed() -> u32 {
    static COUNTER: AtomicU32 = AtomicU32::new(0x2545_f491);
    // Weyl sequence through a mixer: consecutive plays get unrelated seeds.
    let mut x = COUNTER.fetch_add(0x9e37_79b9, Ordering::Relaxed);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

/// A sound effect loaded from a `.orsfx` file. Each play of it with
/// `AudioPlayer(handle)` picks a new random variation; use [`PlaySfx`] for
/// more control.
#[derive(Asset, TypePath, Clone)]
pub struct OrchestreSound {
    sound: Arc<Sound>,
}

impl OrchestreSound {
    /// Build a sound effect in code.
    pub fn from_sound(sound: Sound) -> Self {
        OrchestreSound {
            sound: Arc::new(sound),
        }
    }

    pub fn sound(&self) -> &Sound {
        &self.sound
    }
}

impl Decodable for OrchestreSound {
    type Decoder = SoundDecoder;

    fn decoder(&self) -> SoundDecoder {
        SoundDecoder::new(&self.sound, &PlayOpts::seeded(next_seed()))
    }
}

/// Controls shared between a playing sound and the game (see [`SfxVoice`]).
#[derive(Debug)]
pub struct SfxControl {
    /// f32 bits.
    intensity: AtomicU32,
    pitch: AtomicU32,
    rate: AtomicU32,
    released: AtomicBool,
}

impl SfxControl {
    fn new(opts: &PlayOpts) -> Self {
        SfxControl {
            intensity: AtomicU32::new(opts.intensity.to_bits()),
            pitch: AtomicU32::new(opts.pitch.to_bits()),
            rate: AtomicU32::new(opts.rate.to_bits()),
            released: AtomicBool::new(false),
        }
    }

    fn set(&self, opts: &PlayOpts) {
        self.intensity
            .store(opts.intensity.to_bits(), Ordering::Relaxed);
        self.pitch.store(opts.pitch.to_bits(), Ordering::Relaxed);
        self.rate.store(opts.rate.to_bits(), Ordering::Relaxed);
    }

    fn live(&self, base: &PlayOpts) -> PlayOpts {
        let get = |a: &AtomicU32| f32::from_bits(a.load(Ordering::Relaxed));
        PlayOpts {
            intensity: get(&self.intensity),
            pitch: get(&self.pitch),
            rate: get(&self.rate),
            ..*base
        }
    }

    fn released(&self) -> bool {
        self.released.load(Ordering::Relaxed)
    }
}

/// One play of a sound with fixed options. [`PlaySfx`] creates these; they
/// are freed when the sound ends.
#[derive(Asset, TypePath, Clone)]
pub struct SoundPlayback {
    sound: Arc<Sound>,
    opts: PlayOpts,
    control: Arc<SfxControl>,
}

impl Decodable for SoundPlayback {
    type Decoder = SoundDecoder;

    fn decoder(&self) -> SoundDecoder {
        let mut d = SoundDecoder::new(&self.sound, &self.opts);
        d.control = Some(self.control.clone());
        d
    }
}

/// Streams one play of a sound effect.
pub struct SoundDecoder {
    player: SoundPlayer,
    control: Option<Arc<SfxControl>>,
    left: Vec<f32>,
    right: Vec<f32>,
    frame: usize,
    channel: usize,
    done: bool,
}

impl SoundDecoder {
    fn new(sound: &Sound, opts: &PlayOpts) -> Self {
        SoundDecoder {
            player: SoundPlayer::new(sound, opts, SAMPLE_RATE as f32),
            control: None,
            left: vec![0.0; BLOCK_FRAMES],
            right: vec![0.0; BLOCK_FRAMES],
            frame: BLOCK_FRAMES,
            channel: 0,
            done: false,
        }
    }
}

impl Iterator for SoundDecoder {
    type Item = Sample;

    fn next(&mut self) -> Option<Sample> {
        if self.frame >= BLOCK_FRAMES {
            if let Some(c) = &self.control {
                self.player.set_live(&c.live(&PlayOpts::default()));
                if c.released() {
                    self.player.release();
                }
            }
            if self.done || !self.player.process(&mut self.left, &mut self.right) {
                self.done = true;
                return None;
            }
            self.frame = 0;
        }
        let s = if self.channel == 0 {
            self.left[self.frame]
        } else {
            self.right[self.frame]
        };
        self.channel ^= 1;
        if self.channel == 0 {
            self.frame += 1;
        }
        Some(s as Sample)
    }
}

impl Source for SoundDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(2).unwrap()
    }

    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(SAMPLE_RATE).unwrap()
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

/// What [`PlaySfx`] plays: a synthesized sound or a recorded file.
#[derive(Clone, Debug)]
pub enum SfxSource {
    Synth(Handle<OrchestreSound>),
    Audio(Handle<AudioSource>),
}

impl From<Handle<OrchestreSound>> for SfxSource {
    fn from(h: Handle<OrchestreSound>) -> Self {
        SfxSource::Synth(h)
    }
}

impl From<Handle<AudioSource>> for SfxSource {
    fn from(h: Handle<AudioSource>) -> Self {
        SfxSource::Audio(h)
    }
}

impl SfxSource {
    fn id(&self) -> UntypedAssetId {
        match self {
            SfxSource::Synth(h) => h.id().untyped(),
            SfxSource::Audio(h) => h.id().untyped(),
        }
    }
}

/// Play a sound effect: spawn an entity with this component. It starts as
/// soon as the sound is loaded, and the entity is despawned when it ends.
///
/// Changing `intensity` on the component while a synthesized sound plays
/// changes it live (an engine revving). Looping sounds (made with *Hold* or
/// *Repeat* in the editor) play until [`SfxVoice::release`] is called, or
/// until the entity is despawned (an abrupt stop).
#[derive(Component, Clone, Debug)]
pub struct PlaySfx {
    pub sound: SfxSource,
    /// How hard the sound is played, 0..2. 1 plays it as designed; lower
    /// is softer (and, for synthesized sounds, darker and shorter).
    pub intensity: f32,
    /// Semitones.
    pub pitch: f32,
    /// Linear gain.
    pub volume: f32,
    /// `None` picks a new random variation on every play.
    pub seed: Option<u32>,
    /// Scales the random pitch and volume changes of recorded sounds
    /// (see [`SfxSettings`]). Synthesized sounds vary as they were designed.
    pub variation: f32,
    /// Values of the sound's named controls ("rpm", "rate of fire"…), as
    /// defined in the editor. Changing them while it plays changes the sound.
    pub controls: Vec<(String, f32)>,
}

impl PlaySfx {
    pub fn new(sound: impl Into<SfxSource>) -> Self {
        PlaySfx {
            sound: sound.into(),
            intensity: 1.0,
            pitch: 0.0,
            volume: 1.0,
            seed: None,
            variation: 1.0,
            controls: Vec::new(),
        }
    }

    /// Set one of the sound's named controls, e.g. `.control("rpm", 3000.0)`.
    pub fn control(mut self, name: &str, value: f32) -> Self {
        match self
            .controls
            .iter_mut()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
        {
            Some(c) => c.1 = value,
            None => self.controls.push((name.to_string(), value)),
        }
        self
    }

    /// Options for a synthesized sound, with the controls applied.
    fn opts(&self, sound: &Sound, seed: u32) -> PlayOpts {
        let base = PlayOpts {
            seed,
            intensity: self.intensity,
            pitch: self.pitch,
            volume: self.volume,
            ..PlayOpts::default()
        };
        sound.with_controls(base, self.controls.iter().map(|(n, v)| (n.as_str(), *v)))
    }

    pub fn intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity;
        self
    }

    pub fn pitch(mut self, semitones: f32) -> Self {
        self.pitch = semitones;
        self
    }

    pub fn volume(mut self, volume: f32) -> Self {
        self.volume = volume;
        self
    }

    pub fn seed(mut self, seed: u32) -> Self {
        self.seed = Some(seed);
        self
    }

    pub fn variation(mut self, variation: f32) -> Self {
        self.variation = variation;
        self
    }
}

/// Limits and variation for [`PlaySfx`].
#[derive(Resource, Clone, Debug)]
pub struct SfxSettings {
    /// Most sound effects playing at once; the oldest stops first.
    pub max_voices: usize,
    /// Most copies of one recorded sound playing at once (synthesized
    /// sounds carry their own limit, set in the editor).
    pub audio_max_voices: u8,
    /// Random pitch change of recorded sounds on each play, ± semitones.
    pub audio_pitch_jitter: f32,
    /// Random volume change of recorded sounds on each play, ± decibels.
    pub audio_volume_jitter: f32,
}

impl Default for SfxSettings {
    fn default() -> Self {
        SfxSettings {
            max_voices: 32,
            audio_max_voices: 8,
            audio_pitch_jitter: 0.5,
            audio_volume_jitter: 1.5,
        }
    }
}

/// A [`PlaySfx`] that has started: control it while it plays.
#[derive(Component)]
pub struct SfxVoice {
    source: UntypedAssetId,
    order: u64,
    control: Arc<SfxControl>,
    recorded: bool,
    /// The sound being played (synthesized only), to map control names.
    sound: Option<Arc<Sound>>,
}

impl SfxVoice {
    /// End the sound gracefully: looping layers fade out, repeats stop.
    /// Recorded sounds stop right away.
    pub fn release(&self) {
        self.control.released.store(true, Ordering::Relaxed);
    }

    pub fn is_released(&self) -> bool {
        self.control.released()
    }

    /// Change the intensity while playing (synthesized sounds only). The
    /// `intensity` field of [`PlaySfx`] does the same when it changes.
    pub fn set_intensity(&self, intensity: f32) {
        self.control
            .intensity
            .store(intensity.to_bits(), Ordering::Relaxed);
    }

    /// The sound's named controls, if it is synthesized.
    pub fn controls(&self) -> &[orchestre_core::sfx::Control] {
        self.sound.as_deref().map_or(&[], |s| &s.controls)
    }
}

/// Pass intensity changes on to playing sounds, and stop released
/// recorded sounds.
pub(crate) fn control_sfx(
    mut commands: Commands,
    changed: Query<(&PlaySfx, &SfxVoice), Changed<PlaySfx>>,
    voices: Query<(Entity, &SfxVoice)>,
) {
    for (sfx, voice) in &changed {
        match &voice.sound {
            Some(sound) => voice.control.set(&sfx.opts(sound, 0)),
            None => voice.set_intensity(sfx.intensity),
        }
    }
    for (e, voice) in &voices {
        if voice.recorded && voice.is_released() {
            commands.entity(e).despawn();
        }
    }
}

/// -1..1 from a seed.
fn signed(seed: u32) -> f32 {
    seed as f32 / u32::MAX as f32 * 2.0 - 1.0
}

/// What a started [`PlaySfx`] plays.
enum Start {
    Synth(Handle<SoundPlayback>),
    Audio(Handle<AudioSource>),
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn start_sfx(
    mut commands: Commands,
    pending: Query<(Entity, &PlaySfx), Without<SfxVoice>>,
    playing: Query<(Entity, &SfxVoice)>,
    sounds: Res<Assets<OrchestreSound>>,
    audio: Res<Assets<AudioSource>>,
    mut playbacks: ResMut<Assets<SoundPlayback>>,
    server: Res<AssetServer>,
    settings: Res<SfxSettings>,
) {
    static ORDER: AtomicU64 = AtomicU64::new(0);
    let mut voices: Vec<(Entity, UntypedAssetId, u64)> = playing
        .iter()
        .map(|(e, v)| (e, v.source, v.order))
        .collect();
    for (entity, sfx) in &pending {
        let id = sfx.sound.id();
        if server.load_state(id).is_failed() {
            // The asset server has already logged why.
            commands.entity(entity).despawn();
            continue;
        }
        let seed = sfx.seed.unwrap_or_else(next_seed);
        let mut playing = None;
        let mut control = Arc::new(SfxControl::new(&PlayOpts {
            intensity: sfx.intensity,
            ..PlayOpts::default()
        }));
        let (start, playback, limit) = match &sfx.sound {
            SfxSource::Synth(h) => {
                let Some(s) = sounds.get(h) else { continue };
                let opts = sfx.opts(&s.sound, seed);
                control = Arc::new(SfxControl::new(&opts));
                let sound = s.sound.clone();
                playing = Some(sound.clone());
                let limit = sound.max_voices;
                let h = playbacks.add(SoundPlayback {
                    sound,
                    opts,
                    control: control.clone(),
                });
                (Start::Synth(h), PlaybackSettings::DESPAWN, limit)
            }
            SfxSource::Audio(h) => {
                if audio.get(h).is_none() {
                    continue;
                }
                // Recorded sounds vary by pitch and volume only.
                let v = sfx.variation.max(0.0);
                let semis = sfx.pitch + settings.audio_pitch_jitter * v * signed(seed);
                let db =
                    settings.audio_volume_jitter * v * signed(seed.rotate_left(16) ^ 0x297a_2d39);
                let gain = sfx.volume * intensity_gain(sfx.intensity) * 10f32.powf(db / 20.0);
                let playback = PlaybackSettings::DESPAWN
                    .with_speed(2.0f32.powf(semis / 12.0))
                    .with_volume(Volume::Linear(gain));
                (Start::Audio(h.clone()), playback, settings.audio_max_voices)
            }
        };
        cut_voices(
            &mut commands,
            &mut voices,
            id,
            limit.max(1) as usize,
            settings.max_voices,
        );
        let order = ORDER.fetch_add(1, Ordering::Relaxed);
        let mut e = commands.entity(entity);
        let recorded = matches!(start, Start::Audio(_));
        e.insert((
            playback,
            SfxVoice {
                source: id,
                order,
                control,
                recorded,
                sound: playing,
            },
        ));
        match start {
            Start::Synth(h) => e.insert(AudioPlayer(h)),
            Start::Audio(h) => e.insert(AudioPlayer(h)),
        };
        voices.push((entity, id, order));
    }
}

/// Make room for one more voice of `source`: stop the oldest copies over
/// its limit, and the oldest sounds overall over the global limit.
fn cut_voices(
    commands: &mut Commands,
    voices: &mut Vec<(Entity, UntypedAssetId, u64)>,
    source: UntypedAssetId,
    per_sound: usize,
    total: usize,
) {
    voices.sort_by_key(|v| v.2);
    while voices.iter().filter(|v| v.1 == source).count() >= per_sound {
        let i = voices.iter().position(|v| v.1 == source).unwrap();
        commands.entity(voices.remove(i).0).despawn();
    }
    while !voices.is_empty() && voices.len() >= total.max(1) {
        commands.entity(voices.remove(0).0).despawn();
    }
}

#[derive(Debug)]
pub enum SoundLoadError {
    Io(std::io::Error),
    Format(String),
}

impl std::fmt::Display for SoundLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SoundLoadError::Io(e) => write!(f, "could not read sound: {e}"),
            SoundLoadError::Format(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SoundLoadError {}

#[derive(Default, TypePath)]
pub struct OrchestreSoundLoader;

impl AssetLoader for OrchestreSoundLoader {
    type Asset = OrchestreSound;
    type Settings = ();
    type Error = SoundLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<OrchestreSound, SoundLoadError> {
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .await
            .map_err(SoundLoadError::Io)?;
        let sound =
            orchestre_core::sfx::file::parse_sound_bytes(&bytes).map_err(SoundLoadError::Format)?;
        Ok(OrchestreSound::from_sound(sound))
    }

    fn extensions(&self) -> &[&str] {
        &[orchestre_core::sfx::file::EXTENSION]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestre_core::sfx::presets;

    fn sound(name: &str) -> OrchestreSound {
        OrchestreSound::from_sound(presets::find(name).unwrap().sound())
    }

    #[test]
    fn each_play_ends_and_varies() {
        let s = sound("Pistol shot");
        let a: Vec<Sample> = s.decoder().collect();
        let b: Vec<Sample> = s.decoder().collect();
        assert!(!a.is_empty() && a.len().is_multiple_of(2));
        assert!(a.iter().any(|x| x.abs() > 0.05));
        assert!(a.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
        assert_ne!(a, b, "two plays should differ");
        let secs = a.len() as f32 / 2.0 / SAMPLE_RATE as f32;
        assert!(secs < s.sound().length() * 1.5 + 4.2, "{secs}s");
    }

    #[test]
    fn fixed_seed_plays_the_same() {
        let s = sound("Laser");
        let play = |seed| SoundPlayback {
            sound: s.sound.clone(),
            opts: PlayOpts::seeded(seed),
            control: Arc::new(SfxControl::new(&PlayOpts::default())),
        };
        let a: Vec<Sample> = play(4).decoder().collect();
        let b: Vec<Sample> = play(4).decoder().collect();
        assert_eq!(a, b);
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<OrchestreSound>()
            .init_asset::<SoundPlayback>()
            .init_asset::<AudioSource>()
            .init_resource::<SfxSettings>()
            .add_systems(Update, (start_sfx, control_sfx).chain());
        app
    }

    fn voices(app: &mut App) -> Vec<Entity> {
        let mut q = app.world_mut().query::<(Entity, &SfxVoice)>();
        let mut v: Vec<_> = q.iter(app.world()).map(|(e, v)| (v.order, e)).collect();
        v.sort();
        v.into_iter().map(|(_, e)| e).collect()
    }

    #[test]
    fn per_sound_limit_cuts_the_oldest() {
        let mut app = app();
        let mut s = sound("Coin");
        Arc::make_mut(&mut s.sound).max_voices = 2;
        let h = app
            .world_mut()
            .resource_mut::<Assets<OrchestreSound>>()
            .add(s);
        let first = app.world_mut().spawn(PlaySfx::new(h.clone())).id();
        app.update();
        let second = app.world_mut().spawn(PlaySfx::new(h.clone())).id();
        app.update();
        assert_eq!(voices(&mut app), [first, second]);
        let third = app.world_mut().spawn(PlaySfx::new(h.clone())).id();
        app.update();
        assert_eq!(voices(&mut app), [second, third]);
        assert!(app.world().get_entity(first).is_err(), "oldest despawned");
        let player = app.world().get::<AudioPlayer<SoundPlayback>>(third);
        assert!(player.is_some(), "playing through a playback asset");
    }

    #[test]
    fn global_limit_and_recorded_sounds() {
        let mut app = app();
        app.world_mut().resource_mut::<SfxSettings>().max_voices = 3;
        let sounds: Vec<_> = ["Coin", "Laser", "Punch", "Jump"]
            .into_iter()
            .map(|n| {
                app.world_mut()
                    .resource_mut::<Assets<OrchestreSound>>()
                    .add(sound(n))
            })
            .collect();
        let spawned: Vec<Entity> = sounds
            .iter()
            .map(|h| {
                let e = app.world_mut().spawn(PlaySfx::new(h.clone())).id();
                app.update();
                e
            })
            .collect();
        assert_eq!(voices(&mut app), spawned[1..]);

        // A recorded sound starts with its own varied speed.
        let audio = AudioSource {
            bytes: Arc::from(&[][..]),
        };
        let h = app
            .world_mut()
            .resource_mut::<Assets<AudioSource>>()
            .add(audio);
        let e = app
            .world_mut()
            .spawn(PlaySfx::new(h).pitch(12.0).variation(0.0))
            .id();
        app.update();
        assert!(app.world().get::<AudioPlayer>(e).is_some());
        let speed = app.world().get::<PlaybackSettings>(e).unwrap().speed;
        assert!((speed - 2.0).abs() < 1e-5, "{speed}");
    }

    #[test]
    fn waits_until_loaded() {
        let mut app = app();
        let h = Handle::<OrchestreSound>::default();
        let e = app.world_mut().spawn(PlaySfx::new(h)).id();
        app.update();
        assert!(app.world().get::<SfxVoice>(e).is_none());
    }

    #[test]
    fn looping_sounds_run_until_released() {
        let s = sound("Engine");
        let control = Arc::new(SfxControl::new(&PlayOpts::default()));
        let playback = SoundPlayback {
            sound: s.sound.clone(),
            opts: PlayOpts::default(),
            control: control.clone(),
        };
        let mut d = playback.decoder();
        // Ten seconds in, still going.
        let n = SAMPLE_RATE as usize * 2 * 10;
        assert_eq!(d.by_ref().take(n).count(), n);
        control.intensity.store(1.8f32.to_bits(), Ordering::Relaxed);
        control.released.store(true, Ordering::Relaxed);
        let rest = d.count();
        assert!(rest > 0 && rest < SAMPLE_RATE as usize * 2 * 6, "{rest}");
    }

    #[test]
    fn intensity_changes_reach_playing_sounds() {
        let mut app = app();
        let h = app
            .world_mut()
            .resource_mut::<Assets<OrchestreSound>>()
            .add(sound("Engine"));
        let e = app.world_mut().spawn(PlaySfx::new(h)).id();
        app.update();
        app.world_mut().get_mut::<PlaySfx>(e).unwrap().intensity = 1.7;
        app.update();
        let voice = app.world().get::<SfxVoice>(e).unwrap();
        assert_eq!(voice.control.live(&PlayOpts::default()).intensity, 1.7);
        voice.release();
        assert!(voice.is_released());
    }

    #[test]
    fn named_controls_reach_playing_sounds() {
        let mut app = app();
        let h = app
            .world_mut()
            .resource_mut::<Assets<OrchestreSound>>()
            .add(sound("Machine gun"));
        let e = app
            .world_mut()
            .spawn(PlaySfx::new(h).control("Rate of fire", 1334.0))
            .id();
        app.update();
        let rate = |app: &App| {
            let v = app.world().get::<SfxVoice>(e).unwrap();
            v.control.live(&PlayOpts::default()).rate
        };
        assert!((rate(&app) - 2.0).abs() < 0.01, "{}", rate(&app));
        app.world_mut().get_mut::<PlaySfx>(e).unwrap().controls[0].1 = 667.0;
        app.update();
        assert!((rate(&app) - 1.0).abs() < 0.01, "{}", rate(&app));
        assert_eq!(app.world().get::<SfxVoice>(e).unwrap().controls().len(), 1);
    }
}
