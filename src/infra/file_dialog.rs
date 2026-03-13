use std::path::PathBuf;

use rfd::FileDialog;

pub fn pick_video_files() -> Vec<PathBuf> {
    FileDialog::new()
        .add_filter("Video", &["mp4", "mov"])
        .set_title("Open Videos")
        .pick_files()
        .unwrap_or_default()
}
