//! 任意の音声/動画ファイルを Whisper 用の 16kHz モノラル f32 PCM にデコードする。
//! Symphonia（純 Rust）でデコードするため、ユーザー環境に ffmpeg は不要。

use std::path::Path;

use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::conv::IntoSample;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Whisper が要求するサンプリングレート。
const TARGET_SAMPLE_RATE: u32 = 16_000;

/// ファイルをデコードしてモノラル 16kHz の f32 サンプル列を返す。
pub fn decode_to_mono_16k(path: &str) -> Result<Vec<f32>, String> {
    let (samples, sample_rate, channels) = decode_any(path)?;
    if samples.is_empty() {
        return Err("音声データが空でした。対応していないコーデックか、無音の可能性があります。".into());
    }
    let mono = downmix_to_mono(&samples, channels);
    let resampled = resample_linear(&mono, sample_rate, TARGET_SAMPLE_RATE);
    Ok(resampled)
}

/// Symphonia でデコードし、(インターリーブ f32, サンプルレート, チャンネル数) を返す。
fn decode_any(path: &str) -> Result<(Vec<f32>, u32, usize), String> {
    let file = std::fs::File::open(path).map_err(|e| format!("ファイルを開けません: {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = Path::new(path).extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions {
                enable_gapless: true,
                ..Default::default()
            },
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("フォーマットを判別できません: {e}"))?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| "音声トラックが見つかりません".to_string())?;
    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(TARGET_SAMPLE_RATE);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("デコーダを初期化できません: {e}"))?;

    let mut samples: Vec<f32> = Vec::new();
    // チャンネル数は probe 時点では不明なコーデック（AAC 等）があるため、
    // 実際にデコードしたバッファの spec から確定させる。
    let mut channels: usize = 0;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            // ストリーム終端などはここに来るのでループを抜ける。
            Err(SymphoniaError::IoError(_)) => break,
            Err(SymphoniaError::ResetRequired) => break,
            Err(e) => return Err(format!("読み込みエラー: {e}")),
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => append_samples(decoded, &mut samples, &mut channels),
            // 部分的なデコードエラーはスキップして続行（壊れたパケット耐性）。
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(SymphoniaError::IoError(_)) => break,
            Err(e) => return Err(format!("デコードエラー: {e}")),
        }
    }

    Ok((samples, sample_rate, channels.max(1)))
}

/// 各サンプル型の AudioBuffer を f32 に変換して push する。
/// 併せて、デコード実体のチャンネル数を `channels` に反映する。
fn append_samples(decoded: AudioBufferRef<'_>, out: &mut Vec<f32>, channels: &mut usize) {
    *channels = decoded.spec().channels.count();
    macro_rules! convert {
        ($buf:expr) => {{
            let buf = $buf;
            let spec = *buf.spec();
            let channels = spec.channels.count();
            let frames = buf.frames();
            // インターリーブで詰める（フレーム順 × チャンネル順）。
            for frame in 0..frames {
                for ch in 0..channels {
                    let s: f32 = buf.chan(ch)[frame].into_sample();
                    out.push(s);
                }
            }
        }};
    }

    match decoded {
        AudioBufferRef::U8(b) => convert!(b.as_ref()),
        AudioBufferRef::U16(b) => convert!(b.as_ref()),
        AudioBufferRef::U24(b) => convert!(b.as_ref()),
        AudioBufferRef::U32(b) => convert!(b.as_ref()),
        AudioBufferRef::S8(b) => convert!(b.as_ref()),
        AudioBufferRef::S16(b) => convert!(b.as_ref()),
        AudioBufferRef::S24(b) => convert!(b.as_ref()),
        AudioBufferRef::S32(b) => convert!(b.as_ref()),
        AudioBufferRef::F32(b) => convert!(b.as_ref()),
        AudioBufferRef::F64(b) => convert!(b.as_ref()),
    }
}

/// インターリーブされたサンプルをチャンネル平均でモノラル化する。
fn downmix_to_mono(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    let frames = interleaved.len() / channels;
    let mut mono = Vec::with_capacity(frames);
    for f in 0..frames {
        let base = f * channels;
        let mut sum = 0.0f32;
        for ch in 0..channels {
            sum += interleaved[base + ch];
        }
        mono.push(sum / channels as f32);
    }
    mono
}

/// 線形補間によるリサンプリング。音声認識用途では十分な品質。
fn resample_linear(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || input.is_empty() {
        return input.to_vec();
    }
    let ratio = to_rate as f64 / from_rate as f64;
    let out_len = ((input.len() as f64) * ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_pos = i as f64 / ratio;
        let idx = src_pos.floor() as usize;
        let frac = (src_pos - idx as f64) as f32;
        let a = input[idx.min(input.len() - 1)];
        let b = if idx + 1 < input.len() {
            input[idx + 1]
        } else {
            a
        };
        out.push(a + (b - a) * frac);
    }
    out
}
