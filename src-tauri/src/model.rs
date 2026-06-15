//! Whisper モデル（ggml 形式）の管理。
//! アプリのデータディレクトリに保存し、無ければ HuggingFace から DL する。

use std::path::PathBuf;

use futures_util::StreamExt;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;

/// 利用可能なモデルの定義。
pub struct ModelDef {
    pub id: &'static str,
    pub label: &'static str,
    pub file: &'static str,
    pub approx_mb: u32,
    pub note: &'static str,
}

/// 同梱候補のモデル一覧。ありきたりなノート PC を想定し軽量〜中量のみ。
pub const MODELS: &[ModelDef] = &[
    ModelDef {
        id: "tiny",
        label: "Tiny（最速・精度低）",
        file: "ggml-tiny.bin",
        approx_mb: 75,
        note: "下書きや高速確認向け",
    },
    ModelDef {
        id: "base",
        label: "Base（標準・推奨）",
        file: "ggml-base.bin",
        approx_mb: 142,
        note: "速度と精度のバランス",
    },
    ModelDef {
        id: "small",
        label: "Small（高精度・低速）",
        file: "ggml-small.bin",
        approx_mb: 466,
        note: "日本語の精度を重視する場合",
    },
];

const HF_BASE: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

pub fn find_model(id: &str) -> Option<&'static ModelDef> {
    MODELS.iter().find(|m| m.id == id)
}

/// モデル保存ディレクトリ（無ければ作成）。
pub fn models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("データディレクトリを取得できません: {e}"))?
        .join("models");
    std::fs::create_dir_all(&dir).map_err(|e| format!("ディレクトリを作成できません: {e}"))?;
    Ok(dir)
}

/// 指定モデルのローカルパス。
pub fn model_path(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    let def = find_model(id).ok_or_else(|| format!("未知のモデル: {id}"))?;
    Ok(models_dir(app)?.join(def.file))
}

/// ダウンロード進捗イベントのペイロード。
#[derive(Clone, Serialize)]
pub struct DownloadProgress {
    pub model: String,
    pub downloaded: u64,
    pub total: u64,
}

/// モデルを HuggingFace からストリーミング DL する。途中経過は
/// `model://download-progress` イベントで通知。一時ファイルに書いてから rename。
pub async fn download(app: &AppHandle, id: &str) -> Result<(), String> {
    let def = find_model(id).ok_or_else(|| format!("未知のモデル: {id}"))?;
    let dest = model_path(app, id)?;
    if dest.exists() {
        return Ok(());
    }

    let url = format!("{HF_BASE}/{}", def.file);
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("ダウンロード開始に失敗: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("ダウンロード失敗 (HTTP {})", resp.status()));
    }

    let total = resp.content_length().unwrap_or(0);
    let tmp = dest.with_extension("download");
    let mut file = tokio::fs::File::create(&tmp)
        .await
        .map_err(|e| format!("一時ファイル作成に失敗: {e}"))?;

    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();
    let mut last_emit: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("受信エラー: {e}"))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("書き込みエラー: {e}"))?;
        downloaded += chunk.len() as u64;

        // 約 1MB ごとにイベントを間引いて通知。
        if downloaded - last_emit >= 1_000_000 || downloaded == total {
            last_emit = downloaded;
            let _ = app.emit(
                "model://download-progress",
                DownloadProgress {
                    model: id.to_string(),
                    downloaded,
                    total,
                },
            );
        }
    }

    file.flush()
        .await
        .map_err(|e| format!("フラッシュ失敗: {e}"))?;
    drop(file);

    tokio::fs::rename(&tmp, &dest)
        .await
        .map_err(|e| format!("ファイル確定に失敗: {e}"))?;

    Ok(())
}
