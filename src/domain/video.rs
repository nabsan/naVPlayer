use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct VideoItem {
    pub id: usize,
    pub path: PathBuf,
    pub title: String,
    pub duration_sec: f64,
    pub position_sec: f64,
    pub muted: bool,
}

impl VideoItem {
    pub fn is_image(&self) -> bool {
        crate::domain::media::is_image_path(&self.path)
    }

    pub fn new(id: usize, path: PathBuf) -> Self {
        let title = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Untitled")
            .to_owned();

        Self {
            id,
            path,
            title,
            duration_sec: 0.0,
            position_sec: 0.0,
            muted: true,
        }
    }
}
