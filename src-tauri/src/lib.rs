//! Whisper 書き起こしデスクトップアプリ（ローカル動作・Win/Mac 配布）。
//! フロントエンドから呼ばれる Tauri コマンドを定義する。

pub mod audio;
pub mod model;
pub mod transcribe;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use transcribe::TranscriptResult;

/// フロントに返すモデル情報（DL 済みかどうかを含む）。
#[derive(Serialize)]
struct ModelInfo {
    id: String,
    label: String,
    approx_mb: u32,
    note: String,
    installed: bool,
}

/// 利用可能なモデル一覧と DL 状態を返す。
#[tauri::command]
fn list_models(app: AppHandle) -> Result<Vec<ModelInfo>, String> {
    let dir = model::models_dir(&app)?;
    let infos = model::MODELS
        .iter()
        .map(|m| ModelInfo {
            id: m.id.to_string(),
            label: m.label.to_string(),
            approx_mb: m.approx_mb,
            note: m.note.to_string(),
            installed: dir.join(m.file).exists(),
        })
        .collect();
    Ok(infos)
}

/// モデルをダウンロードする（既にあれば即時 Ok）。
#[tauri::command]
async fn download_model(app: AppHandle, model_id: String) -> Result<(), String> {
    model::download(&app, &model_id).await
}

/// ファイルを書き起こす。重い処理は blocking スレッドで実行する。
#[tauri::command]
async fn transcribe_file(
    app: AppHandle,
    path: String,
    model_id: String,
    language: String,
) -> Result<TranscriptResult, String> {
    // モデルが無ければ先に DL（ネットワーク前提）。
    let model_path = model::model_path(&app, &model_id)?;
    if !model_path.exists() {
        model::download(&app, &model_id).await?;
    }

    let app_for_cb = app.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<TranscriptResult, String> {
        let samples = audio::decode_to_mono_16k(&path)?;
        transcribe::run(&model_path, &samples, &language, move |p| {
            let _ = app_for_cb.emit("transcribe://progress", serde_json::json!({ "progress": p }));
        })
    })
    .await
    .map_err(|e| format!("処理スレッドが異常終了: {e}"))??;

    Ok(result)
}

/// テキストを指定パスへ書き出す（保存ダイアログで得たパスを受け取る）。
#[tauri::command]
fn write_text_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|e| format!("保存に失敗: {e}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            list_models,
            download_model,
            transcribe_file,
            write_text_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
