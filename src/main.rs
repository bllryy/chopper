mod app;
mod engine;
mod util;

use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use eframe::egui;
use engine::{Cmd, Engine, SharedState};
use util::{load_wav, slice_transient, synth_break};

fn main() -> eframe::Result<()> {
    // ── Audio setup ───────────────────────────────────────────────────────
    let host = cpal::default_host();
    let device = host.default_output_device().expect("no output device");
    let config = device.default_output_config().expect("no default config");
    let sr = config.sample_rate() as f32;
    let channels = config.channels() as usize;

    let (sample, src_ratio, sample_name) = match std::env::args().nth(1) {
        Some(p) => {
            let (s, wsr) = load_wav(&p).expect("failed to load wav");
            let ratio = wsr as f32 / sr;
            let name = std::path::Path::new(&p)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or(p.clone());
            (s, ratio, name)
        }
        None => (synth_break(sr), 1.0, String::from("<synth break>")),
    };

    let slices = slice_transient(&sample, sr * src_ratio);
    let slice_count = slices.len();
    eprintln!("{} slices detected", slice_count);

    let shared = SharedState::new();
    let (mut tx, mut rx) = rtrb::RingBuffer::<Cmd>::new(64);

    let mut eng = Engine::new(
        Arc::clone(&sample),
        slices.clone(),
        sr,
        src_ratio,
        165.0,
        Arc::clone(&shared),
    );

    // Seed a default pattern
    use engine::Note;
    let n = |slice: u8, semis: i8| Some(Note { slice, semis });
    let pat: [Option<Note>; 16] = [
        n(0, 0),  None,      n(4, 0),  n(2, 0),
        n(8, -5), None,      n(4, 0),  n(10, 0),
        n(0, 0),  n(12, 0),  n(4, 7),  n(2, 0),
        n(6, 0),  n(4, 12),  n(14, 0), n(8, -12),
    ];
    tx.push(Cmd::SetPattern(pat)).ok();
    tx.push(Cmd::SetBpm(165.0)).ok();

    let stream = device
        .build_output_stream(
            config.into(),
            move |data: &mut [f32], _| {
                while let Ok(cmd) = rx.pop() {
                    eng.apply(cmd);
                }
                eng.render(data, channels);
            },
            |err| eprintln!("stream error: {err}"),
            None,
        )
        .expect("build stream");
    stream.play().expect("play");

    // ── GUI ───────────────────────────────────────────────────────────────
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("CHOPPER")
            .with_inner_size([1040.0, 700.0])
            .with_min_inner_size([800.0, 560.0]),
        ..Default::default()
    };

    let app = app::ChopperApp::new(tx, shared, &sample, slices, sample_name, 165.0);

    eframe::run_native(
        "CHOPPER",
        native_options,
        Box::new(|_cc| Ok(Box::new(app))),
    )?;

    drop(stream);
    Ok(())
}
