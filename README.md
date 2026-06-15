# Whisper 書き起こし (Whisper Desktop)

ありきたりなノート PC（GPU 不要）で動く、**完全ローカル**の音声・動画 文字起こしデスクトップアプリ。
Windows / macOS の両方に配布できる。

- 🔒 **ローカル完結** — 音声はネットに送られない。書き起こしは端末内の whisper.cpp で実行
- 🧳 **自己完結** — ユーザー側に Python / ffmpeg のインストール不要（Symphonia で純 Rust デコード）
- 🪶 **軽量** — Tauri 製。インストーラは数 MB（モデルのみ初回 DL）
- 🎯 **対応形式** — mp3 / m4a / wav / flac / ogg / opus / aac / mp4 / mov / mkv / webm など
- 📝 **エクスポート** — テキスト / SRT / VTT、クリップボードコピー

## 使い方

### インストール（Mac）

1. Releases から `.dmg` をダウンロードする
2. `.dmg` を開き、アプリを「アプリケーション」へドラッグする
3. アプリを右クリック →「開く」→「開く」で起動する（初回のみ。未署名のため）

### インストール（Windows）

1. Releases から セットアップ用 `.exe`（または `.msi`）をダウンロードする
2. ファイルを実行し、画面に従ってインストールする（SmartScreen 警告は「詳細情報」→「実行」）
3. スタートメニューの「Whisper 書き起こし」を起動する

### 文字起こし（Mac / Windows 共通）

1. 画面上部で **モデル**（標準は Base）と **言語**（日本語など）を選ぶ
2. 音声・動画ファイルを枠に **ドラッグ＆ドロップ**（クリックで選択も可）
3. 自動で書き起こしが始まる（★初回だけモデルを自動ダウンロード。以降はオフラインで動作）
4. 結果を **テキスト / タイムスタンプ** タブで確認する
5. **.txt / .srt / .vtt** で保存、またはコピーする

## 仕組み

| 層 | 技術 |
|----|------|
| デスクトップ枠 | [Tauri 2](https://tauri.app)（Rust + WebView） |
| 音声デコード | [Symphonia](https://github.com/pdeljanov/Symphonia)（純 Rust）→ 16kHz モノラル |
| 推論 | [whisper-rs](https://github.com/tazz4843/whisper-rs)（whisper.cpp、CPU） |
| モデル | ggml 形式。初回実行時に HuggingFace から DL し、アプリのデータ領域に保存 |

モデルは `tiny` / `base`（推奨）/ `small` から選択。初回のみネットワークが必要で、以降はオフライン動作可。

## 開発

前提: [Node.js](https://nodejs.org)、[Rust](https://rustup.rs)、CMake、C コンパイラ。

```bash
npm install
npm run tauri dev      # 開発起動
```

## 配布ビルド

各 OS 上でビルドする（Tauri はクロスコンパイル非対応のため）。

```bash
npm run tauri build
```

成果物:
- **macOS**: `src-tauri/target/release/bundle/dmg/*.dmg`（および `.app`）
- **Windows**: `src-tauri\target\release\bundle\nsis\*-setup.exe`（および MSI）

### 両 OS の成果物を自動生成（推奨）

`.github/workflows/release.yml` を同梱。GitHub にタグ（例 `v0.1.0`）を push すると、
macOS（Intel/Apple Silicon）と Windows のインストーラを自動ビルドして Release に添付する。

```bash
git tag v0.1.0 && git push origin v0.1.0
```

## 署名について（配布時の注意）

- **macOS**: 未署名だと初回起動時に Gatekeeper の警告。Apple Developer 証明書での署名・公証を推奨
- **Windows**: 未署名だと SmartScreen 警告。コードサイニング証明書を推奨

社内・限定配布なら未署名でも動くが、上記の警告が出る。

## ライセンス

whisper.cpp / モデル（OpenAI Whisper）各々のライセンスに従う。
