# naVPlayer

Rust + `eframe/egui` + `libmpv` で作る、Windows 向けの軽量動画プレイヤーです。

## 概要

`naVPlayer` は、ダンス動画やフォーム比較用途を想定した Windows 専用プレイヤーです。
現在の実装では、Rust 製の親アプリから外部 `libmpv` 再生ウィンドウを制御します。

## 現在できること

- `.mp4` / `.mov` を開く
- `Single` モード再生
- `Multi` モードで最大 2 本まで同時再生
- 共通の `play` / `pause` / `stop` / `seek`
- 複数動画の同期再生
- 1 本だけを master audio として再生
- 直近 30 件までの履歴機能
  - 開いたときのモード (`Single` / `Multi`) を記憶
  - 開いたファイル一覧を記憶
  - `navplayer.exe` と同じフォルダの `recent_files.toml` に保存
  - ツールバーの `Recent` から再オープン可能
- 再生ウィンドウ側のキー・マウス操作
  - `Space`: 再生 / 一時停止
  - 動画中央付近の左クリック: 再生 / 一時停止
  - `Left` / `Right`: 巻き戻し / 早送り、先頭と終端にガードあり
  - `Up` / `Down`: 音量変更
  - `f`: フルスクリーン切替
  - `Shift+f`: 通常ウィンドウ表示に戻す
  - `c`: 現在フレームを `thumbnails` フォルダへ保存
- 再生ウィンドウ右上の `×` で正常に閉じられる
- `Single` モードで自動サムネ保存
- ツールバーからサムネ保存先フォルダを開ける
- 起動引数で動画ファイルを直接開ける
- `.mp4` / `.mov` のファイル関連付け用スクリプト付き
- Explorer からの関連付け起動を単一インスタンスで処理
  - 別の動画をダブルクリックしても既存の naVPlayer インスタンスを再利用
  - 背後で第1画面が増殖しない

## 現在の動作仕様

- 動画再生は `egui` パネル内ではなく、外部 `mpv` 子ウィンドウで行う
- Explorer の関連付け起動で動画を開くと、再生ウィンドウが前面に出て自動再生される
- 関連付け起動の `Single` 再生では、親の naVPlayer ウィンドウはバックグラウンドで最小化のまま維持され、子ウィンドウを閉じても復帰しない
- `navplayer.exe` を直接起動したときは、親の naVPlayer ウィンドウが表示され、手動オープンや `Multi` モードを使える
- `Multi` モードでは最大 2 本を初期配置し、その後は手動で移動できる

## 依存関係: libmpv DLL

実行・配布前に、次の Windows 用 `libmpv` パッケージを取得してください。

- [shinchiro mpv dev build (2026-03-07)](https://github.com/shinchiro/mpv-winbuild-cmake/releases/download/20260307/mpv-dev-x86_64-20260307-git-f9190e5.7z)

展開後、`libmpv-2.dll` を取り出して `navplayer.exe` と同じフォルダに置いてください。

実行フォルダの例:

```text
navplayer.exe
libmpv-2.dll
recent_files.toml
```

## コンパイル方法

デバッグビルド:

```powershell
cargo build
```

生成先:

```text
S:\tools\codex\naVPlayer\target\debug\navplayer.exe
```

リリースビルド:

```powershell
cargo build --release
```

生成先:

```text
S:\tools\codex\naVPlayer\target\release\navplayer.exe
```

`build.rs` により、プロジェクト直下の `libmpv-2.dll` はビルド時に自動で対応する `target\debug` または `target\release` にコピーされます。

## 起動方法

ソースツリー上で起動:

```powershell
cargo run
```

リリース版を直接起動:

```powershell
.\target\release\navplayer.exe
```

特定ファイルを直接開く場合:

```powershell
cargo run -- "C:\videos\sample.mov"
```

または:

```powershell
.\target\release\navplayer.exe "C:\videos\sample.mov"
```

## 配布方法

配布物として最低限必要なのは次です。

```text
navplayer.exe
libmpv-2.dll
README.md
README_JP.md
```

実行中に生成される補助ファイル:

```text
recent_files.toml
```

配布手順の推奨:

1. `cargo build --release` を実行
2. `target\release\navplayer.exe` があることを確認
3. `target\release\libmpv-2.dll` があることを確認
4. その内容を zip 化して配布

配布フォルダ例:

```text
naVPlayer_release\
  navplayer.exe
  libmpv-2.dll
  README.md
  README_JP.md
```

## ファイル関連付け

`.mp4` / `.mov` を naVPlayer に関連付ける場合:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\associate_navplayer.ps1 -ExePath "S:\tools\codex\naVPlayer\target\release\navplayer.exe"
```

デバッグ版に関連付ける場合:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\associate_navplayer.ps1
```

関連付けを解除する場合:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\unassociate_navplayer.ps1
```

## サムネ保存

`Single` モードでツールバーの `Get Thumbnail` を ON にすると、自動サムネ保存が動きます。
保存先は次です。

```text
<元動画フォルダ>\thumbnails
```

ツールバーには絶対パス表示が出て、`Open Thumbnails Folder` で Explorer を開けます。
また、再生ウィンドウで `c` を押すと、同じフォルダに現在フレームを手動保存できます。

## 注意

- 現状は Windows 専用
- 埋め込み OpenGL 描画は安定しなかったため、外部 `mpv` ウィンドウ方式を採用
- `.mov` の再生可否は、同梱する `libmpv` ビルド側のコーデック対応に依存する
