# naVPlayer

Lightweight Windows video player built with Rust, `eframe/egui`, and `libmpv`.

## Overview

`naVPlayer` is a Windows-focused video player for dance and form comparison workflows.
The current implementation uses external `libmpv` playback windows controlled by a Rust desktop app.

## Current Features

- Open `.mp4` and `.mov`
- `Single` mode playback
- `Multi` mode playback for up to 2 videos
- Shared `play` / `pause` / `stop` / `seek`
- Sync playback between loaded videos
- Master audio selection for one active video
- Recent session history for up to 30 entries
  - remembers the mode (`Single` / `Multi`)
  - remembers the opened file list
  - stored as `recent_files.toml` next to `navplayer.exe`
  - selectable from the toolbar `Recent` menu
- Keyboard and mouse control in playback windows
  - `Space`: play/pause
  - Left click on the video area: play/pause
  - `Left` / `Right`: seek backward/forward with start/end guards
  - `Up` / `Down`: volume
  - `f`: toggle fullscreen
  - `Shift+f`: return to normal windowed mode
  - `c`: capture current frame into the `thumbnails` folder
- Close playback windows with the normal window `X`
- Auto thumbnail capture in `Single` mode
- Open thumbnail output folder from the toolbar
- Accept video file paths as launch arguments
- Windows file association helper script for `.mp4` / `.mov`

## Current Behavior

- Playback happens in external `mpv` child windows, not inside the main egui panel
- In `Single` mode, when a video is open, the main naVPlayer window minimizes automatically
- When the single playback window is closed and no video remains, the main window is restored
- In `Multi` mode, up to 2 playback windows are auto-arranged initially, then can be moved manually

## Dependency: libmpv DLL

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

`build.rs` automatically copies `libmpv-2.dll` from the project root into the matching `target\debug` or `target\release` folder during build.

## Run

Run from source tree:

```powershell
cargo run
```

Run release build directly:

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

## Distribution

Recommended distribution contents:

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
4. Zip the contents for distribution

Suggested release folder:

```text
naVPlayer_release\
  navplayer.exe
  libmpv-2.dll
  README.md
  README_JP.md
```

## File Association

Associate `.mp4` and `.mov` with naVPlayer:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\associate_navplayer.ps1 -ExePath "S:\tools\codex\naVPlayer\target\release\navplayer.exe"
```

If you want to use the debug build instead:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\associate_navplayer.ps1
```

Remove the file association:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\unassociate_navplayer.ps1
```

## Thumbnail Output

In `Single` mode, enable `Get Thumbnail` in the toolbar.
Captured images are stored in:

```text
<video folder>\thumbnails
```

The toolbar shows the absolute save path and includes `Open Thumbnails Folder`.
You can also press `c` in a playback window to save the current frame manually into the same folder.

## Notes

- The app is currently Windows-only
- The embedded OpenGL rendering path was abandoned in favor of stable external `mpv` windows
- `.mov` support depends on codec support inside the bundled `libmpv` build
