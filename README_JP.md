# naVPlayer

Rust + `eframe/egui` + `libmpv` で作る、Windows 向けの軽量動画プレイヤーです。

English: [README.md](./README.md)

## 概要

`naVPlayer` は、ダンス動画の確認、フォーム比較、2 画面比較を主用途にした Windows 向け動画プレイヤーです。
Rust の操作画面と再生ワーカーが同じプロセス内で動き、`libmpv` が独立したネイティブ再生ウィンドウに描画します。別の `mpv.exe` やブラウザー実行環境は使用しません。

## 使い始めるには

2026-11-23 時点の機能です。Windows 11 向けに、独立した再生画面、同期比較、静止画のごみ箱移動、Explorer 連携、画面上のショートカット一覧を備えています。

ポータブル版がある場合は `dist\navplayer.exe` を起動します。新しく取得したソースから使う場合は、後述の「依存関係」「コンパイル方法」「配布方法」を参照してください。実行ファイルと DLL は Git に含めていません。

1. `Open File(s)` で動画・画像を開きます。画像はすぐに表示され、操作画面から開いた動画は `Play` で再生します。
2. 2 ファイルを比較するときは `Multi` を選び、`Open File(s)` で `Ctrl` を押しながら 2 ファイルを選択します。動画同士、静止画同士、動画と静止画の組み合わせに対応します。
3. `Play` で動画をまとめて再生します。`Sync ON` で動画を同期します。静止画は再生時間軸や音声の同期対象にしません。
4. 再生画面にフォーカスを移し、`H` または `I` で操作一覧を表示します。同じキーをもう一度押すか、`Esc` で一覧を閉じます。

最後の動画・画像の表示画面を閉じると、親画面とワーカーも終了します。

## 現在できること

- `.mp4` / `.mov` / `.mkv` / `.webm` / `.avi` / `.m4v` / `.wmv` / `.mpg` / `.mpeg` / `.ts` / `.mts` / `.m2ts` / `.flv` / `.ogv` / `.3gp` を開く
- `.jpg` / `.jpeg` / `.png` / `.bmp` / `.webp` / `.tif` / `.tiff` の静止画を表示し、画像同士や動画との比較にも使える
- `Single` モードで再生
- `Multi` モードで動画・画像を 2 画面比較。読み込みは最大 4 ファイルですが、ネイティブ再生画面の初期配置は先頭 2 本のみ
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

| キー | 動画 | 静止画 |
| --- | --- | --- |
| `Space` | 再生 / 一時停止 | 操作不要 |
| `Right` | 同じ動画を 10 秒早送り | 次の静止画。動画は飛ばす |
| `Left` | 同じ動画を 10 秒巻き戻し | 前の静止画。動画は飛ばす |
| `Up` / `Down` | 音量を上げる / 下げる | 音声なし |
| `Ctrl+b` | 先頭へ戻る | 再生時間軸なし |
| `n` / `p` | 次 / 前の対応ファイル | 動画も含めた次 / 前の対応ファイル |
| `f` / `Shift+f` | フルスクリーン切替 / 通常表示へ戻る | 同左 |
| `c` | 現在フレームを `thumbnails` に保存 | 表示中のフレームを `thumbnails` に保存 |
| `e` / `E` | Explorer で保存先を開き、ファイルを選択 | 同左 |
| `h` / `H` / `i` / `I` | ショートカット一覧の表示 / 非表示 | 静止画用の内容で表示 / 非表示 |
| `Esc` | 再生画面を残してショートカット一覧を閉じる | 同左 |
| `Delete` | ファイル削除なし | `削除しますか？ y/N` を表示 |
| `q`・`Alt+F4`・右上の `×` | この再生画面を閉じる | 同左 |

動画のシークには先頭・終端ガードがあり、終端で押しても別ファイルには移動しません。静止画の矢印移動は名前の自然順で、先頭・末尾では折り返しません。

静止画の削除確認中は、`y` / `Y` でそのファイルをごみ箱へ移動します。`n` / `N`・`Enter`・`Esc`・左右どちらかの矢印でキャンセルし、60 秒の無操作でもキャンセルします。確認中はこれらのキーを通常のファイル移動より優先します。

## 画面上のフィードバック

再生ウィンドウでは、主要操作に対して軽い OSD 表示が出ます。

- `<<`: 巻き戻し入力
- `>>`: 早送り入力
- `Now playing: <filename>`: `n` / `p` でファイル切替したとき
- `Saved thumbnail: <path>`: 手動サムネ保存したとき
- `H` / `I` で切り替える半透明の日本語ショートカット一覧。動画・静止画で内容が変わり、ウィンドウのサイズ変更に追従
- 静止画の削除確認と、Explorer・ファイル操作の結果

## 現在の動作仕様

- 動画再生は `egui` パネル内ではなく、外部 `mpv` ウィンドウで行う
- Explorer の関連付け起動で動画を開くと、再生ウィンドウが前面に出て自動再生される
- Explorer から別の動画を開いても、既存の `naVPlayer` インスタンスを再利用する
- 背景で親ウィンドウが増殖しない
- ファイルから起動すると、再生ウィンドウが開いている間は親ウィンドウを最小化する
- 1 本閉じても残りの再生を継続し、残った動画の master audio を維持する
- 最後の再生ウィンドウを `×` / `q` / `Close Selected` で閉じると、親・ワーカーを含めアプリ全体が終了する
- 通常の動画終端では再生画面を残す。読み込み・デコード失敗時は操作画面を復帰させ、エラーから回復できるようにする
- `navplayer.exe` を直接起動したときは、親ウィンドウが表示され、手動オープンや `Multi` モードに使える
- `Multi` モードでは 2 本までを初期配置し、その後は手動で移動できる

## 応答性と対応形式

`Open File(s)` で動画・静止画を選べます。ファイル選択には動画と画像の混在・動画のみ・画像のみのフィルターがあります。画像は Play を押さずに表示され、閉じるまで表示を維持します。`Multi` では画像 2 枚や、画像と動画の比較も可能です。静止画には再生時間軸を持たせず、同期補正・自動サムネ保存・master audio の対象から除外します。画像だけを開いているときは動画の再生・シーク操作を無効にします。最後の画像を閉じると、動画の場合と同じくアプリ全体が終了します。上記 7 拡張子の画像を同梱 DLL でデコード検証済みです。アニメーション画像や複数ページ TIFF のページ移動は今回の静止画機能の対象外です。

静止画の矢印移動は動画を飛ばし、同じフォルダの画像を名前の自然順（`photo2.jpg` → `photo10.jpg`）で表示します。Explorer ウィンドウ側の並び順やフィルターとは連動しません。先頭・末尾では移動を止めます。`n` / `p` は従来どおり動画・画像を混ぜた移動です。削除確定後は次の画像、次がなければ前の画像を開き、画像が残っていなければその再生画面を閉じます。対象はその画面に表示中の通常の画像ファイル 1 件だけです。確認中にファイルが変わった場合は削除を取り消します。ごみ箱移動に失敗した場合は画像を表示し直してエラーを出し、ごみ箱を使用できない場合の完全削除は Windows Shell のコールバックで拒否します。

libmpv の初期化・操作・サムネ保存・破棄は、UI スレッドから分離した 1 本の再生ワーカーで実行します。動画を開いていない間は定期ポーリングを行わず、UI / IPC の通知を待ちます。再生中は 100 ms、一時停止中は 250 ms 間隔で状態を取得します。ウィンドウハンドルを保持し、再生時間の長さはファイル変更時に更新します。連続したシーク要求をまとめ、同期補正の頻度を制限し、シーク中の再補正を避けます。`hwdec=auto-safe` は維持しており、ハードウェア支援の可否はコーデック・GPU・ドライバーによります。

ファイル選択・起動引数・次／前の動画移動は、共通の拡張子一覧を使用します。拡張子はコンテナの種類であり、コーデックとは別です。デコードは同梱 DLL が担当します。一般的なデコーダーはすでに同梱されており、今回はアプリ側の MP4 / MOV 限定を解除しています。

同梱の `mpv v0.41.0-259-gf9190e57f` で、次の組み合わせの短い生成動画を最後までデコードできました。

| 映像 | コンテナ | 音声 |
| --- | --- | --- |
| H.264 / AVC | MP4 | AAC |
| H.265 / HEVC | MKV | AAC |
| VP9 | WebM | Opus |
| AV1 | MKV | Opus |
| MPEG-4 Part 2 | AVI | MP3 |
| MPEG-2 | MPG | MP2 |
| WMV8 | WMV | WMA2 |
| ProRes | MOV | PCM |

この表は映像・音声出力を null にしたソフトウェアデコード検証であり、すべてのプロファイル・破損ファイル・GPU の動作を保証するものではありません。別途、ネイティブ再生ウィンドウの生成とプロセス終了を確認しています。検証した組み合わせにシステムのコーデックパックは不要です。関連付けスクリプトの対象は従来どおり `.mp4` / `.mov` です。他の形式はファイル選択や起動引数から開けます。

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
ver.yyyymmdd.hh.mm.ss
```

表示場所は次の 3 箇所です。実行中ビルドの JST 日時を秒まで表示します。再生画面のタイトルは `naVPlayer ver.yyyymmdd.hh.mm.ss | <ファイルのフルパス>` で、表示ファイルを切り替えると自動更新します。内部の画面番号・プロセス番号は表示しません。開いたままの旧プロセスは、再起動するまで元のバージョンを表示します。

- メインウィンドウのタイトルバー
- 動画・静止画の各再生ウィンドウのタイトルバー
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

作成済みのポータブル版は `dist\navplayer.exe` です。同じフォルダに `libmpv-2.dll` を置いています。改修版は旧プレイヤーとは別の単一起動接続先を使い、新版を起動したつもりが旧版に転送されることを防ぎます。開いたままの旧版ウィンドウの処理は変わらないため、新しい操作は新しく開いた画面で使用してください。既存の Windows ファイル関連付けは自動変更しません。

更新後は、起動中の naVPlayer をすべて閉じてから開き直してください。ディスク上の exe を置き換えても、実行中のプロセスは更新されません。親画面と再生画面のタイトルバーでバージョンを確認できます。改修版の標準接続先は `127.0.0.1:43873`、4 月の旧版は `127.0.0.1:43871` です。テストや分離したポータブル環境では、`NAVPLAYER_IPC_ADDR` でループバック接続先を変更できます。

```powershell
.\dist\navplayer.exe
```

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
powershell -ExecutionPolicy Bypass -File .\scripts\associate_navplayer.ps1 -ExePath "S:\tools\codex\naVPlayer\dist\navplayer.exe"
```

`-ExePath` を省略したときの既定値は `dist\navplayer.exe` です。Windows の「プログラムから開く」「既定のアプリ」で naVPlayer を選ぶ必要がある場合があります。スクリプトは保護された `UserChoice` 設定を上書きしません。ダブルクリックで旧版が起動するときは、タイトルのバージョンに加え、関連付けの実行ファイルやショートカットのリンク先も確認してください。

デバッグ版に関連付ける場合:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\associate_navplayer.ps1 -ExePath "S:\tools\codex\naVPlayer\target\debug\navplayer.exe"
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
4. 次のコマンドでポータブル版フォルダを作り、必要ならその内容を zip 化

```powershell
New-Item -ItemType Directory -Path .\dist -Force
Copy-Item -LiteralPath .\target\release\navplayer.exe,.\target\release\libmpv-2.dll,.\README.md,.\README_JP.md -Destination .\dist -Force
```

既存のポータブル版を置き換えるときは、先にプレイヤーを終了してください。`dist`・`target`・DLL・生成したテスト素材・個人の `recent_files.toml` は Git 管理対象外です。配布用の zip には個人の履歴や旧 exe のバックアップを入れないでください。

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
- コーデック・プロファイルの対応は、同梱する `libmpv` ビルドに依存する
- ネイティブ再生画面の初期配置は 2 画面まで。操作画面の `2x2` を選んでも、外部再生画面 4 枚の自動配置にはならない

## 検証

プロジェクト直下で実行します。コーデック検証には PATH 上の FFmpeg が必要です。合成画像・短いコーデック検証動画・小さな 40 秒のシーク検証動画を、Git 管理対象外の `target\validation` に生成します。Rust テストは対応形式の判定、終了と音声の再割り当て、古いワーカー結果の除外、実際の libmpv による移動・シーク、ごみ箱移動と失敗時の復帰を検証します。

```powershell
python scripts/verify_codecs.py
cargo test --target-dir target/optimized -- --include-ignored --test-threads=1
cargo clippy --target-dir target/optimized --all-targets -- -D warnings
cargo build --release --target-dir target/optimized
python scripts/smoke_windows.py target/optimized/release/navplayer.exe
```

Windows の起動テストは `NAVPLAYER_IPC_ADDR` で別のローカル IPC 接続先を使い、自分で起動したテストプロセスのウィンドウだけを閉じます。削除・矢印選択の検証では自作の使い捨て画像をごみ箱へ移すため、Windows のごみ箱を利用できる通常のデスクトップ実行環境が必要です。結果は `target\validation\codec-results.json` と `target\validation\windows-smoke-results.json` に保存します。依存関係がキャッシュ済みなら Cargo コマンドに `--offline` を追加できます。短時間の待機 CPU 計測は旧版との比較ベンチマークではなく、4K 再生性能を保証するものでもありません。
