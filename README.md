# naVPlayer

Lightweight Windows video player built with Rust, `eframe/egui`, and `libmpv`.

Japanese: [README_JP.md](./README_JP.md)

## Overview

`naVPlayer` is a Windows-focused video player for dance review, form checking, and side-by-side comparison.
The Rust desktop UI and a playback worker share one process. `libmpv` renders into separate native playback windows; no separate `mpv.exe` or browser runtime is needed.

## Quick Start

Current feature set: 2026-11-23. This is a Windows 11 video and still-image viewer with independent playback windows, synchronized comparison, safe image recycling, Explorer integration, and on-screen shortcut help.

If you already have the portable build, start `dist\navplayer.exe`. For a fresh checkout, follow **Dependency**, **Build**, and **Distribution** below; executables and the DLL are not stored in Git.

1. Use `Open File(s)` to open one video or image. Images display immediately; use `Play` for a video opened from the main UI.
2. For two-file comparison, select `Multi`, click `Open File(s)`, and hold `Ctrl` while selecting two files. Both may be videos, still images, or one of each.
3. Use `Play` for shared video playback. `Sync ON` synchronizes videos; images do not participate in the timeline or audio.
4. Focus a playback window and press `H` or `I` to see the controls. Press the same key again, or `Esc`, to hide the list.

Closing the final playback/image window exits the parent and worker as well.

## What It Can Do

- Open `.mp4`, `.mov`, `.mkv`, `.webm`, `.avi`, `.m4v`, `.wmv`, `.mpg`, `.mpeg`, `.ts`, `.mts`, `.m2ts`, `.flv`, `.ogv`, and `.3gp`
- View `.jpg`, `.jpeg`, `.png`, `.bmp`, `.webp`, `.tif`, and `.tiff` still images, including side-by-side image/video comparison
- Play videos in `Single` mode
- Compare two videos/images side by side in `Multi` mode; the loader accepts up to four files, but initial native-window placement arranges only the first two
- Shared `play` / `pause` / `stop` / `seek`
- Synchronized playback between loaded videos
- One active master audio source, others muted
- Recent session history for up to 30 entries
  - remembers `Single` / `Multi`
  - remembers opened file lists
  - stored as `recent_files.toml` next to `navplayer.exe`
- Auto thumbnail capture in `Single` mode
- Manual thumbnail capture from the playback window
- Open the thumbnail folder directly from the toolbar
- Accept video paths as launch arguments
- Associate `.mp4` / `.mov` with `naVPlayer`
- Reuse a single app instance when videos are launched from Explorer

## Playback Window Controls

Inside each playback window:

| Key | Video | Still image |
| --- | --- | --- |
| `Space` | Play / pause | Not needed |
| `Right` | Seek forward 10 seconds in the same file | Next still image; skip videos |
| `Left` | Seek backward 10 seconds in the same file | Previous still image; skip videos |
| `Up` / `Down` | Volume up / down | No audio |
| `Ctrl+b` | Return to the start | No timeline |
| `n` / `p` | Next / previous supported file | Next / previous supported file, including videos |
| `f` / `Shift+f` | Toggle fullscreen / return to windowed mode | Same |
| `c` | Save the current frame to `thumbnails` | Save the displayed frame to `thumbnails` |
| `e` / `E` | Open the containing Explorer folder and select this file | Same |
| `h` / `H` / `i` / `I` | Toggle the shortcut list | Same, with image-specific controls |
| `Esc` | Close the shortcut list, leaving the playback window open | Same |
| `Delete` | No file deletion | Show `削除しますか？ y/N` |
| `q`, `Alt+F4`, window `X` | Close this playback window | Same |

Video seeking retains start/end guards and never changes files, even at EOF. Still-image arrows use natural filename order and do not wrap at the ends.

During an image deletion confirmation, `y` / `Y` moves that file to the Recycle Bin. `n` / `N`, `Enter`, `Esc`, or either arrow cancels; 60 seconds without an answer also cancels. These prompt bindings override navigation while the prompt is open.

## On-Screen Feedback

`naVPlayer` currently shows lightweight OSD feedback in the playback window for key actions:

- `<<` when seeking backward
- `>>` when seeking forward
- `Now playing: <filename>` when moving to the next / previous file with `n` / `p`
- `Saved thumbnail: <path>` after manual thumbnail capture
- A translucent Japanese shortcut panel, with different video/image controls, toggled by `H` / `I` and resized with the window
- The image deletion confirmation and Explorer/file-operation results

## Current Runtime Behavior

- Playback happens in external `mpv` windows, not inside the egui panel
- When a video is launched from Explorer by file association, playback starts automatically
- Associated Explorer launches reuse the already-running `naVPlayer` instance
- The main `naVPlayer` window does not multiply in the background
- File launches minimize the main window while playback windows are open
- Closing one playback window keeps the others running and preserves the surviving master audio
- Closing the last playback window, with `X`, `q`, or `Close Selected`, exits the entire app, including the parent and worker
- Reaching normal end-of-file keeps the playback window open; a load/decoder error restores the main window for recovery
- Launching `navplayer.exe` directly shows the main window for manual open and `Multi` workflows
- In `Multi` mode, up to 2 playback windows are initially auto-arranged, then can be moved manually

## Responsiveness and Format Support

Use `Open File(s)` to select videos or still images; the dialog includes combined, video-only and image-only filters. Images appear immediately without pressing Play and remain open until closed. `Multi` also accepts two images or an image alongside a video. Still images have no playback timeline, are excluded from sync correction and automatic thumbnails, and never become the master audio source. Video playback/seek controls are disabled when only still images are open. Closing the last image exits the app, just like closing the last video. The seven image extensions above were decoded with the bundled DLL; animated images and multi-page TIFF navigation are outside this still-image feature.

For still-image arrow navigation, videos are skipped and images are sorted by natural filename order (`photo2.jpg` before `photo10.jpg`); it does not read an Explorer window's current sort/filter selection. Navigation stops at the first/last image. The `n` / `p` keys retain mixed video/image navigation. After confirmed deletion, the player opens the next image, falls back to the previous one if necessary, or closes that playback window if no images remain. Only that window's displayed regular image file is targeted. A file change during confirmation cancels deletion. Failed recycle operations restore the image and show an error; permanent deletion is vetoed by the Windows Shell progress callback when recycling is unavailable.

All libmpv initialization, commands, screenshots, and destruction run on one playback worker rather than the UI thread. With no video loaded, the app waits for UI/IPC events without periodic polling. Active playback is sampled every 100 ms, paused playback every 250 ms. Window handles are cached, duration is refreshed on file changes, consecutive slider seeks are coalesced, and sync corrections are rate-limited and skipped during a seek. `hwdec=auto-safe` remains enabled; hardware acceleration depends on the codec, GPU, and driver.

The file picker, command-line opening, and next/previous navigation use the same extension list. Extensions describe containers, not codecs. Actual decoding is provided by the bundled DLL, which already contains common decoders; this change removes the app's MP4/MOV-only restriction.

The bundled `mpv v0.41.0-259-gf9190e57f` decoded short generated fixtures to EOF with these combinations:

| Video | Container | Audio |
| --- | --- | --- |
| H.264 / AVC | MP4 | AAC |
| H.265 / HEVC | MKV | AAC |
| VP9 | WebM | Opus |
| AV1 | MKV | Opus |
| MPEG-4 Part 2 | AVI | MP3 |
| MPEG-2 | MPG | MP2 |
| WMV8 | WMV | WMA2 |
| ProRes | MOV | PCM |

These are software-decoding checks with null video/audio outputs, not a guarantee for every codec profile, damaged file, or GPU. A separate native-window smoke test covers playback-window creation and process exit. No system codec pack is required for the tested combinations. The association scripts still target only `.mp4` / `.mov`; other containers can be opened from the file picker or launch arguments.

## Dependency: `libmpv-2.dll`

Before running or distributing the app, download the Windows `libmpv` package from:

- [shinchiro mpv dev build (2026-03-07)](https://github.com/shinchiro/mpv-winbuild-cmake/releases/download/20260307/mpv-dev-x86_64-20260307-git-f9190e5.7z)

Extract the archive and take `libmpv-2.dll`.
Place `libmpv-2.dll` in the same folder as `navplayer.exe`.

Example runtime layout:

```text
navplayer.exe
libmpv-2.dll
recent_files.toml
```

## Versioning

The app version is generated automatically at build time in JST.
Format:

```text
ver.yyyymmdd.hh.mm.ss
```

The version appears in the main window title bar, every video/still-image playback window title bar, and the toolbar. Playback titles use `naVPlayer ver.yyyymmdd.hh.mm.ss | <full file path>` and update automatically when the displayed file changes. Internal player/process numbers are not displayed. The version identifies the running build, including the JST build date and time down to seconds. An already-open older process keeps its original version until restarted.

## Build

Debug build:

```powershell
cargo build
```

Output:

```text
S:\tools\codex\naVPlayer\target\debug\navplayer.exe
```

Release build:

```powershell
cargo build --release
```

Output:

```text
S:\tools\codex\naVPlayer\target\release\navplayer.exe
```

`build.rs` automatically:

- generates the version string
- copies `libmpv-2.dll` from the project root into `target\debug` or `target\release`

## Run

The prepared portable build is `dist\navplayer.exe`, with `libmpv-2.dll` beside it. Revised builds use a separate single-instance endpoint from the legacy player, so launching the updated executable cannot silently forward to the old playback core. Existing legacy windows keep running their old code; use the newly opened window for the updated controls. Existing Windows file associations are not changed automatically.

After an update, close all running naVPlayer windows before reopening the executable: replacing the file on disk does not update an already-running process. Compare the version in the main and playback title bars. The revised default endpoint is `127.0.0.1:43873`; the April legacy build used `127.0.0.1:43871`. Tests and isolated portable sessions may override it with a loopback `NAVPLAYER_IPC_ADDR`.

```powershell
.\dist\navplayer.exe
```

Run from the source tree:

```powershell
cargo run
```

Run the release build directly:

```powershell
.\target\release\navplayer.exe
```

Open a specific file directly:

```powershell
cargo run -- "C:\videos\sample.mov"
```

Or:

```powershell
.\target\release\navplayer.exe "C:\videos\sample.mov"
```

## File Association

Associate `.mp4` and `.mov` with the release build:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\associate_navplayer.ps1 -ExePath "S:\tools\codex\naVPlayer\dist\navplayer.exe"
```

The helper defaults to `dist\navplayer.exe` when `-ExePath` is omitted. Windows may still require you to select naVPlayer in **Open with** or **Default apps**; the helper does not overwrite protected `UserChoice` settings. If double-click launches an old version, check the registered executable or shortcut target as well as the title-bar version.

Use the debug build instead:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\associate_navplayer.ps1 -ExePath "S:\tools\codex\naVPlayer\target\debug\navplayer.exe"
```

Remove the association:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\unassociate_navplayer.ps1
```

## Thumbnail Output

In `Single` mode, enable `Get Thumbnail` in the toolbar to save thumbnails automatically while playing.
Captured images are stored in:

```text
<video folder>\thumbnails
```

The toolbar shows the absolute save path and includes `Open Thumbnails Folder`.
Pressing `c` in a playback window saves the current frame into the same folder.

## Distribution

Recommended release contents:

```text
navplayer.exe
libmpv-2.dll
README.md
README_JP.md
```

Optional runtime-generated file:

```text
recent_files.toml
```

Recommended flow:

1. Run `cargo build --release`
2. Confirm `target\release\navplayer.exe` exists
3. Confirm `target\release\libmpv-2.dll` exists
4. Create the portable folder below, then zip its contents if needed

```powershell
New-Item -ItemType Directory -Path .\dist -Force
Copy-Item -LiteralPath .\target\release\navplayer.exe,.\target\release\libmpv-2.dll,.\README.md,.\README_JP.md -Destination .\dist -Force
```

Close the player before replacing an existing portable executable. `dist`, `target`, the DLL, generated test media, and personal `recent_files.toml` history are ignored by Git; do not add runtime history or previous executable backups to a release archive.

Suggested release folder:

```text
naVPlayer_release\
  navplayer.exe
  libmpv-2.dll
  README.md
  README_JP.md
```

## Notes

- Windows-only for now
- Stable external `mpv` windows are used instead of the abandoned embedded OpenGL path
- Codec/profile support depends on the bundled `libmpv` build
- Initial native-window placement supports two windows; the `2x2` UI layout does not arrange four external playback windows

## Verification

Run from the project root. The codec script requires FFmpeg on PATH and creates synthetic images, short codec samples, and a small 40-second seek fixture under ignored `target\validation`. Rust tests cover media filtering, close/audio remapping, stale worker results, real libmpv navigation/seeking, and confirmed recycling/recovery.

```powershell
python scripts/verify_codecs.py
cargo test --target-dir target/optimized -- --include-ignored --test-threads=1
cargo clippy --target-dir target/optimized --all-targets -- -D warnings
cargo build --release --target-dir target/optimized
python scripts/smoke_windows.py target/optimized/release/navplayer.exe
```

The Windows smoke test uses a separate loopback IPC endpoint through `NAVPLAYER_IPC_ADDR`; it only closes windows belonging to its own test processes. Its deletion and arrow-selection cases recycle generated disposable images and need normal desktop access to the Windows Recycle Bin. Results are saved in `target\validation\codec-results.json` and `target\validation\windows-smoke-results.json`. Add `--offline` to Cargo commands when dependencies are already cached. The short idle CPU sample is not a comparison benchmark against the old version; no 4K performance guarantee is implied.
