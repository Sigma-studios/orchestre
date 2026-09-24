//! Plays `assets/music/demo.orch` in a loop without opening a window.
//!
//! cargo run --release -p orchestre-bevy --example play

use std::time::Duration;

use bevy::app::ScheduleRunnerPlugin;
use bevy::asset::LoadState;
use bevy::audio::AudioPlugin;
use bevy::prelude::*;
use orchestre_bevy::{OrchestrePlugin, OrchestreSong};

#[derive(Resource)]
struct Song(Handle<OrchestreSong>);

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
                1.0 / 30.0,
            ))),
            AssetPlugin::default(),
            AudioPlugin::default(),
            OrchestrePlugin,
        ))
        .add_systems(Startup, play)
        .add_systems(Update, report)
        .run();
}

fn play(mut commands: Commands, assets: Res<AssetServer>) {
    let song: Handle<OrchestreSong> = assets.load("music/demo.orch");
    commands.spawn(AudioPlayer(song.clone()));
    commands.insert_resource(Song(song));
}

/// Say once whether the song loaded.
fn report(
    assets: Res<AssetServer>,
    songs: Res<Assets<OrchestreSong>>,
    song: Res<Song>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    match assets.load_state(&song.0) {
        LoadState::Loaded => {
            let secs = songs.get(&song.0).map_or(0.0, |s| s.duration_secs());
            println!("Playing music/demo.orch ({secs:.1}s, loops forever) — Ctrl+C to stop");
            *done = true;
        }
        LoadState::Failed(err) => {
            eprintln!("Could not load music/demo.orch: {err}");
            *done = true;
        }
        _ => {}
    }
}
