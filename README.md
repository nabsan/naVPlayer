# naVPlayer

Lightweight Windows video player built with Rust, `eframe/egui`, and `libmpv`.

## Overview

`naVPlayer` is a Windows-focused video player for dance review, form checking, and side-by-side comparison.
The current architecture uses a Rust desktop app for state and UI, plus external `libmpv` playback windows for stable video rendering on Windows.

## What It Can Do

- Open `.mp4` and `.mov`
- Play videos in `Single` mode
- Compare up to 2 videos in `Multi` mode
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

- `Space`: play / pause
- Left click on the video area: play / pause
- `Left`: seek backward with start guard
- `Right`: seek forward with end guard
- `Up` / `Down`: volume up / down
- `f`: toggle fullscreen
- `Shift+f`: return to normal windowed mode
- `c`: save the current frame to `thumbnails`
- `n`: open the next `.mp4` / `.mov` in the same folder
- `p`: open the previous `.mp4` / `.mov` in the same folder
- Window `X`: close the playback window

## On-Screen Feedback

`naVPlayer` currently shows lightweight OSD feedback in the playback window for key actions:

- `<<` when seeking backward
- `>>` when seeking forward
- `Now playing: <filename>` when moving to the next / previous file with `n` / `p`
- `Saved thumbnail: <path>` after manual thumbnail capture

## Current Runtime Behavior

- Playback happens in external `mpv` windows, not inside the egui panel
- When a video is launched from Explorer by file association, playback starts automatically
- Associated Explorer launches reuse the already-running `naVPlayer` instance
- The main `naVPlayer` window does not multiply in the background
- In associated `Single` playback, the main window stays minimized even after the child playback window is closed
- Launching `navplayer.exe` directly shows the main window for manual open and `Multi` workflows
- In `Multi` mode, up to 2 playback windows are initially auto-arranged, then can be moved manually

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
ver.yyyymmdd.hh.mm
```

The version appears in the main window title bar and in the toolbar.

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
powershell -ExecutionPolicy Bypass -File .\scripts\associate_navplayer.ps1 -ExePath "S:\tools\codex\naVPlayer\target\release\navplayer.exe"
```

Use the debug build instead:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\associate_navplayer.ps1
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
4. Zip those files for distribution

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
- `.mov` support depends on codec support in the bundled `libmpv` build
