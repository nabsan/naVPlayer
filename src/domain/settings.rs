use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub loop_playback: bool,
    pub remember_last_files: bool,
    pub thumbnail_interval_sec: f64,
    pub thumbnail_guard_sec: f64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            loop_playback: false,
            remember_last_files: true,
            thumbnail_interval_sec: 5.0,
            thumbnail_guard_sec: 3.0,
        }
    }
}
