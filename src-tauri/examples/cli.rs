//! GUI なしで decode→transcribe パイプラインを検証する CLI。
//! 使い方: cargo run --example cli -- <音声ファイル> <ggmlモデルパス> [言語]

use std::path::Path;

use whisper_desktop_lib::{audio, transcribe};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: cli <audio> <model.bin> [language]");
        std::process::exit(2);
    }
    let audio_path = &args[1];
    let model_path = &args[2];
    let language = args.get(3).map(|s| s.as_str()).unwrap_or("auto");

    eprintln!("decoding {audio_path} ...");
    let samples = audio::decode_to_mono_16k(audio_path).expect("decode failed");
    eprintln!(
        "decoded {} samples ({:.1}s @16k)",
        samples.len(),
        samples.len() as f32 / 16_000.0
    );

    eprintln!("transcribing with {model_path} (lang={language}) ...");
    let result = transcribe::run(Path::new(model_path), &samples, language, |p| {
        eprint!("\rprogress: {p}%   ");
    })
    .expect("transcribe failed");
    eprintln!("\n--- result ---");
    println!("{}", result.text);
    eprintln!("--- {} segments ---", result.segments.len());
}
