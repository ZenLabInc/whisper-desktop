import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { open, save } from "@tauri-apps/plugin-dialog";

// ---- 型 ----
interface ModelInfo {
  id: string;
  label: string;
  approx_mb: number;
  note: string;
  installed: boolean;
}
interface Segment {
  start: number; // センチ秒
  end: number;
  text: string;
}
interface TranscriptResult {
  text: string;
  segments: Segment[];
}

// ---- 状態 ----
let models: ModelInfo[] = [];
let currentResult: TranscriptResult | null = null;
let busy = false;

// ---- DOM ----
const $ = <T extends HTMLElement>(sel: string) => document.querySelector(sel) as T;
const modelSelect = $("#model-select") as HTMLSelectElement;
const langSelect = $("#lang-select") as HTMLSelectElement;
const modelStatus = $("#model-status");
const dropzone = $("#dropzone");
const dzTitle = $("#dz-title");
const progressArea = $("#progress-area");
const progressText = $("#progress-text");
const progressPct = $("#progress-pct");
const progressBar = $("#progress-bar");
const resultArea = $("#result-area");
const resultText = $("#result-text");
const resultSegments = $("#result-segments");
const toast = $("#toast");

// ---- ユーティリティ ----
function showToast(msg: string, isError = false) {
  toast.textContent = msg;
  toast.classList.toggle("error", isError);
  toast.classList.remove("hidden");
  setTimeout(() => toast.classList.add("hidden"), 4000);
}

function fmtTimestamp(centiseconds: number, comma = true): string {
  const totalMs = centiseconds * 10;
  const ms = totalMs % 1000;
  const totalSec = Math.floor(totalMs / 1000);
  const s = totalSec % 60;
  const m = Math.floor(totalSec / 60) % 60;
  const h = Math.floor(totalSec / 3600);
  const pad = (n: number, w = 2) => String(n).padStart(w, "0");
  const sep = comma ? "," : ".";
  return `${pad(h)}:${pad(m)}:${pad(s)}${sep}${pad(ms, 3)}`;
}

function setProgress(pct: number | null, label: string) {
  progressArea.classList.remove("hidden");
  progressText.textContent = label;
  if (pct === null) {
    progressBar.classList.add("indeterminate");
    progressPct.textContent = "";
  } else {
    progressBar.classList.remove("indeterminate");
    progressBar.style.width = `${pct}%`;
    progressPct.textContent = `${Math.round(pct)}%`;
  }
}

// ---- モデル ----
async function refreshModels() {
  models = await invoke<ModelInfo[]>("list_models");
  modelSelect.innerHTML = "";
  for (const m of models) {
    const opt = document.createElement("option");
    opt.value = m.id;
    opt.textContent = m.installed
      ? `${m.label}`
      : `${m.label} — 未DL (${m.approx_mb}MB)`;
    modelSelect.appendChild(opt);
  }
  // 既定: base が無ければ先頭
  const base = models.find((m) => m.id === "base");
  modelSelect.value = base ? "base" : models[0]?.id ?? "";
  updateModelStatus();
}

function updateModelStatus() {
  const m = models.find((x) => x.id === modelSelect.value);
  if (!m) return;
  modelStatus.textContent = m.installed
    ? `✓ ${m.label}：DL済み — ${m.note}`
    : `⬇ ${m.label}：初回実行時に ${m.approx_mb}MB をダウンロードします`;
}

// ---- 書き起こし実行 ----
async function transcribe(path: string) {
  if (busy) return;
  busy = true;
  currentResult = null;
  resultArea.classList.add("hidden");
  dzTitle.textContent = fileName(path);

  const modelId = modelSelect.value;
  const language = langSelect.value;
  const model = models.find((m) => m.id === modelId);

  try {
    if (model && !model.installed) {
      setProgress(0, `モデル(${model.label})をダウンロード中…`);
    } else {
      setProgress(null, "音声を解析中…");
    }

    const result = await invoke<TranscriptResult>("transcribe_file", {
      path,
      modelId,
      language,
    });

    currentResult = result;
    renderResult(result);
    setProgress(100, "完了");
    setTimeout(() => progressArea.classList.add("hidden"), 800);
    await refreshModels();
  } catch (err) {
    progressArea.classList.add("hidden");
    showToast(`エラー: ${err}`, true);
  } finally {
    busy = false;
  }
}

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

// ---- 結果描画 ----
function renderResult(result: TranscriptResult) {
  resultArea.classList.remove("hidden");
  resultText.textContent = result.text || "（文字起こし結果が空でした）";

  resultSegments.innerHTML = "";
  for (const seg of result.segments) {
    const row = document.createElement("div");
    row.className = "seg-row";
    const time = document.createElement("span");
    time.className = "seg-time";
    time.textContent = fmtTimestamp(seg.start, false).slice(0, 8);
    const txt = document.createElement("span");
    txt.className = "seg-text";
    txt.textContent = seg.text;
    row.appendChild(time);
    row.appendChild(txt);
    resultSegments.appendChild(row);
  }
}

// ---- エクスポート ----
function toSrt(segs: Segment[]): string {
  return segs
    .map((s, i) => {
      return `${i + 1}\n${fmtTimestamp(s.start)} --> ${fmtTimestamp(
        s.end
      )}\n${s.text}\n`;
    })
    .join("\n");
}
function toVtt(segs: Segment[]): string {
  const body = segs
    .map(
      (s) =>
        `${fmtTimestamp(s.start, false)} --> ${fmtTimestamp(s.end, false)}\n${
          s.text
        }`
    )
    .join("\n\n");
  return `WEBVTT\n\n${body}\n`;
}

async function exportAs(kind: "txt" | "srt" | "vtt") {
  if (!currentResult) return;
  const content =
    kind === "txt"
      ? currentResult.text
      : kind === "srt"
      ? toSrt(currentResult.segments)
      : toVtt(currentResult.segments);

  const path = await save({
    defaultPath: `transcript.${kind}`,
    filters: [{ name: kind.toUpperCase(), extensions: [kind] }],
  });
  if (!path) return;
  try {
    await invoke("write_text_file", { path, content });
    showToast(`保存しました: ${fileName(path)}`);
  } catch (err) {
    showToast(`保存失敗: ${err}`, true);
  }
}

// ---- ファイル選択 ----
async function browseFile() {
  if (busy) return;
  const selected = await open({
    multiple: false,
    filters: [
      {
        name: "音声・動画",
        extensions: [
          "mp3", "m4a", "wav", "flac", "ogg", "opus", "aac",
          "mp4", "mov", "mkv", "webm", "m4v",
        ],
      },
    ],
  });
  if (typeof selected === "string") {
    transcribe(selected);
  }
}

// ---- 初期化 ----
window.addEventListener("DOMContentLoaded", async () => {
  await refreshModels();

  modelSelect.addEventListener("change", updateModelStatus);

  dropzone.addEventListener("click", browseFile);
  dropzone.addEventListener("keydown", (e) => {
    if ((e as KeyboardEvent).key === "Enter") browseFile();
  });

  // Tauri のネイティブ ドラッグ&ドロップ
  const webview = getCurrentWebviewWindow();
  await webview.onDragDropEvent((event) => {
    const p = event.payload;
    if (p.type === "over") {
      dropzone.classList.add("dragover");
    } else if (p.type === "drop") {
      dropzone.classList.remove("dragover");
      const file = p.paths?.[0];
      if (file) transcribe(file);
    } else {
      dropzone.classList.remove("dragover");
    }
  });

  // 進捗イベント（書き起こし）
  await listen<{ progress: number }>("transcribe://progress", (e) => {
    setProgress(e.payload.progress, "書き起こし中…");
  });

  // 進捗イベント（モデルDL）
  await listen<{ model: string; downloaded: number; total: number }>(
    "model://download-progress",
    (e) => {
      const { downloaded, total } = e.payload;
      const pct = total > 0 ? (downloaded / total) * 100 : null;
      const mb = (n: number) => (n / 1_000_000).toFixed(0);
      setProgress(
        pct,
        `モデルをダウンロード中… ${mb(downloaded)} / ${mb(total)} MB`
      );
    }
  );

  // タブ切替
  document.querySelectorAll(".tab").forEach((tab) => {
    tab.addEventListener("click", () => {
      document.querySelectorAll(".tab").forEach((t) => t.classList.remove("active"));
      tab.classList.add("active");
      const view = (tab as HTMLElement).dataset.view;
      resultText.classList.toggle("hidden", view !== "text");
      resultSegments.classList.toggle("hidden", view !== "segments");
    });
  });

  // エクスポート / コピー
  $("#export-txt").addEventListener("click", () => exportAs("txt"));
  $("#export-srt").addEventListener("click", () => exportAs("srt"));
  $("#export-vtt").addEventListener("click", () => exportAs("vtt"));
  $("#copy-btn").addEventListener("click", async () => {
    if (!currentResult) return;
    await navigator.clipboard.writeText(currentResult.text);
    showToast("クリップボードにコピーしました");
  });
});
