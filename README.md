# Hermes WebUI Desktop

Tauri 2 ベースの [Hermes WebUI](https://github.com/nesquena/hermes-webui) デスクトップアプリ。ネイティブウィンドウ内で Hermes WebUI サーバーを自動起動し、シームレスなデスクトップ体験を提供します。

## アーキテクチャ

```
Tauri App (Rust)
  ├── アプリ起動時に Python WebUI サーバーを子プロセスとしてspawn
  ├── /health エンドポイントでサーバー稼働を確認（最大60秒）
  ├── Webview をサーバー URL にリダイレクト
  └── ウィンドウクローズ時に子プロセスをkill
```

`hermes-webui` は Git submodule としてバンドルされ、リソースディレクトリに同梱されます。

## 必要環境

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 18+
- Python 3.11+（Hermes Agent とその依存関係が必要）
- **Linux 追加パッケージ:** `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`, `patchelf`

## セットアップ

```bash
git clone --recurse-submodules <repo-url>
cd hermes-webui-desktop

# submodule を更新（既存チェックアウトの場合）
git submodule update --init --recursive
```

## 開発

```bash
# 開発モードで起動（ホットリロードなし、デバッグビルド）
cargo tauri dev
```

デバッグビルドでは DevTools が自動的に開きます。

## ビルド

```bash
# リリースビルド
cargo tauri build
```

生成物:

| プラットフォーム | 出力 |
|---|---|
| macOS | `.app`, `.dmg` |
| Linux | `.deb`, `.AppImage` |
| Windows | `.msi`, `.exe` (NSIS) |

## リリース

GitHub Release を公開すると、`.github/workflows/release.yml` が自動的に macOS (Universal Binary)・Linux・Windows の3プラットフォームでビルドし、アーティファクトを Release にアップロードします。

```bash
git tag v0.1.0
git push origin v0.1.0
# → GitHub で Release を作成・公開するとワークフローが実行される
```

## アイコン

アプリアイコンは macOS 標準仕様（Squircle / superellipse, キャンバスの87.5%）に準拠しています。アイコンの再生成は以下の手順で行います:

```bash
# 1024x1024 のソース画像から全プラットフォーム分を一括生成
cargo tauri icon path/to/source-icon.png
```

## プロジェクト構成

```
hermes-webui-desktop/
├── src/main.rs          # Tauri シェル — サーバー起動・ヘルスチェック・IPC
├── frontend/index.html  # ローディング画面 → サーバーURLへリダイレクト
├── hermes-webui/        # WebUI サーバー (Git submodule)
├── icons/               # 全プラットフォーム対応アイコン
├── capabilities/        # Tauri 権限定義 (shell実行)
├── build.rs             # Tauri ビルドスクリプト
└── tauri.conf.json      # Tauri 設定
```

## 仕組み

1. **起動** — Tauri が空きポートを見つけて Python サーバーを spawn
2. **ヘルスチェック** — `/health` が 200 を返すまで待機（最大60秒）
3. **表示** — Webview が WebUI サーバーにリダイレクト
4. **終了** — ウィンドウクローズ時に子プロセスを kill
