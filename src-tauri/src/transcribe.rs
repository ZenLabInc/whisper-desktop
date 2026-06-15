//! whisper-rs（whisper.cpp バインディング）による書き起こし本体。
//! CPU で動作し、外部ランタイム不要。進捗は Tauri イベントで通知する。

use std::path::Path;

use serde::Serialize;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// 1 セグメント（タイムスタンプ付き）。start/end はセンチ秒（1/100 秒）。
#[derive(Clone, Serialize)]
pub struct Segment {
    pub start: i64,
    pub end: i64,
    pub text: String,
}

/// 書き起こし結果。
#[derive(Clone, Serialize)]
pub struct TranscriptResult {
    pub text: String,
    pub segments: Vec<Segment>,
}

/// 16kHz モノラル f32 を書き起こす。`language` が "auto" なら自動判定。
/// `on_progress` は 0-100 の進捗を受け取るコールバック（UI 通知用に注入する）。
pub fn run<F>(
    model_path: &Path,
    audio: &[f32],
    language: &str,
    on_progress: F,
) -> Result<TranscriptResult, String>
where
    F: FnMut(i32) + 'static,
{
    let model_str = model_path
        .to_str()
        .ok_or_else(|| "モデルパスが不正です".to_string())?;

    let ctx = WhisperContext::new_with_params(model_str, WhisperContextParameters::default())
        .map_err(|e| format!("モデル読み込み失敗: {e}"))?;

    let mut state = ctx
        .create_state()
        .map_err(|e| format!("状態初期化失敗: {e}"))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4) as i32;
    params.set_n_threads(threads);
    params.set_translate(false);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    if language != "auto" {
        params.set_language(Some(language));
    }

    // 進捗コールバック（呼び出し側が UI 通知などに利用）。
    params.set_progress_callback_safe(on_progress);

    state
        .full(params, audio)
        .map_err(|e| format!("書き起こし処理に失敗: {e}"))?;

    let n = state
        .full_n_segments()
        .map_err(|e| format!("セグメント数取得に失敗: {e}"))?;

    let mut segments = Vec::with_capacity(n as usize);
    let mut full = String::new();

    for i in 0..n {
        let text = state
            .full_get_segment_text(i)
            .map_err(|e| format!("セグメント取得に失敗: {e}"))?;
        let start = state
            .full_get_segment_t0(i)
            .map_err(|e| format!("開始時刻取得に失敗: {e}"))?;
        let end = state
            .full_get_segment_t1(i)
            .map_err(|e| format!("終了時刻取得に失敗: {e}"))?;

        let trimmed = text.trim().to_string();
        if !trimmed.is_empty() {
            full.push_str(&trimmed);
            full.push('\n');
        }
        segments.push(Segment {
            start,
            end,
            text: trimmed,
        });
    }

    Ok(TranscriptResult {
        text: full.trim_end().to_string(),
        segments,
    })
}
