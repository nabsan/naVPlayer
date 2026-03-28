# naVPlayer

Rust + `eframe/egui` + `libmpv` で作る、Windows 向けの軽量動画プレイヤーです。

## 概要

`naVPlayer` は、ダンス動画の確認、フォーム比較、2 画面比較を主用途にした Windows 向け動画プレイヤーです。
現在の構成では、Rust 側が状態管理と UI を担当し、実際の動画描画は外部 `libmpv` 再生ウィンドウで安定動作させています。

## 現在できること

- `.mp4` / `.mov` を開く
- `Single` モードで再生
- `Multi` モードで最大 2 本まで比較再生
- 共通の `play` / `pause` / `stop` / `seek`
- 複数動画の同期再生
- 1 本だけを master audio として再生
- 直近 30 件までの履歴機能
  - `Single` / `Multi` を記憶
  - 開いたファイル一覧を記憶
  - `navplayer.exe` と同じフォルダの `recent_files.toml` に保存
- `Single` モードで自動サムネ保存
- 再生ウィンドウから手動サムネ保存
- ツールバーからサムネ保存フォルダを直接開ける
- 起動引数で動画を直接開ける
- `.mp4` / `.mov` を `naVPlayer` に関連付けできる
- Explorer から開いたときに単一インスタンスで再利用する

## 再生ウィンドウの操作

各再生ウィンドウでは次が使えます。

- `Space`: 再生 / 一時停止
- 動画中央付近の左クリック: 再生 / 一時停止
- `Left`: 巻き戻し。先頭側ガードあり
- `Right`: 早送り。終端側ガードあり
- `Up` / `Down`: 音量変更
- `f`: フルスクリーン切替
- `Shift+f`: 通常ウィンドウ表示に戻す
- `c`: 現在フレームを `thumbnails` に保存
- `n`: 同じフォルダ内の次の `.mp4` / `.mov` を開く
- `p`: 同じフォルダ内の前の `.mp4` / `.mov` を開く
- ウィンドウ右上の `×`: 再生ウィンドウを閉じる

## 画面上のフィードバック

再生ウィンドウでは、主要操作に対して軽い OSD 表示が出ます。

- `<<`: 巻き戻し入力
- `>>`: 早送り入力
- `Now playing: <filename>`: `n` / `p` でファイル切替したとき
- `Saved thumbnail: <path>`: 手動サムネ保存したとき

## 現在の動作仕様

- 動画再生は `egui` パネル内ではなく、外部 `mpv` ウィンドウで行う
- Explorer の関連付け起動で動画を開くと、再生ウィンドウが前面に出て自動再生される
- Explorer から別の動画を開いても、既存の `naVPlayer` インスタンスを再利用する
- 背景で親ウィンドウが増殖しない
- 関連付け起動の `Single` 再生では、親の `naVPlayer` ウィンドウは最小化のまま維持される
- 子の再生ウィンドウを閉じても、親ウィンドウは自動復帰しない
- `navplayer.exe` を直接起動したときは、親ウィンドウが表示され、手動オープンや `Multi` モードに使える
- `Multi` モードでは 2 本までを初期配置し、その後は手動で移動できる

## 依存関係: `libmpv-2.dll`

実行・配布前に、次の Windows 用 `libmpv` パッケージを取得してください。

- [shinchiro mpv dev build (2026-03-07)](https://github.com/shinchiro/mpv-winbuild-cmake/releases/download/20260307/mpv-dev-x86_64-20260307-git-f9190e5.7z)

展開後、`libmpv-2.dll` を取り出して `navplayer.exe` と同じフォルダに置いてください。

実行フォルダの例:

```text
navplayer.exe
libmpv-2.dll
recent_files.toml
```

## バージョン表記

アプリのバージョンはビルド時に JST ベースで自動発番されます。
形式は次です。

```text
ver.yyyymmdd.hh.mm
```

表示場所は次の 2 箇所です。

- メインウィンドウのタイトルバー
- ツールバー右側

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

`build.rs` は次を自動で行います。

- バージョン文字列の生成
- プロジェクト直下の `libmpv-2.dll` を `target\debug` / `target\release` にコピー

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

## ファイル関連付け

`.mp4` / `.mov` をリリース版に関連付ける場合:

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

ツールバーには絶対パスが表示され、`Open Thumbnails Folder` で Explorer を開けます。
また、再生ウィンドウで `c` を押すと、同じフォルダに現在フレームを手動保存できます。

## 配布方法

最低限の配布物は次です。

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

## 注意

- 現状は Windows 専用
- 埋め込み OpenGL 描画は安定しなかったため、外部 `mpv` ウィンドウ方式を採用
- `.mov` の再生可否は、同梱する `libmpv` ビルド側のコーデック対応に依存する
