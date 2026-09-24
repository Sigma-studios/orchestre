use orchestre_core::Project;
use orchestre_dsp::{Song, render_song};
fn main() {
    let p = Project::demo();
    let song = Song::from_project(&p);
    let t = std::time::Instant::now();
    let out = render_song(&song, 48000, 3.0);
    let secs = out.len() as f64 / 2.0 / 48000.0;
    let el = t.elapsed().as_secs_f64();
    let json = serde_json::to_string(&p).unwrap();
    println!(
        "rendered {secs:.1}s of audio in {el:.3}s = {:.1}% of one core; json {} bytes; wav would be {} bytes",
        el / secs * 100.0,
        json.len(),
        out.len() * 2
    );
}
