use crate::domain::playback::{MultiLayout, PlaybackState, ViewMode};
use crate::domain::settings::AppSettings;
use crate::domain::video::VideoItem;
use crate::infra::recent::RecentSession;

#[derive(Debug)]
pub struct AppState {
    pub view_mode: ViewMode,
    pub layout: MultiLayout,
    pub videos: Vec<VideoItem>,
    pub selected_index: Option<usize>,
    pub playback_state: PlaybackState,
    pub sync_enabled: bool,
    pub speed: f32,
    pub master_audio_index: Option<usize>,
    pub shared_position_sec: f64,
    pub selected_volume: f64,
    pub selected_fullscreen: bool,
    pub auto_thumbnail_enabled: bool,
    pub last_thumbnail_capture_sec: Option<f64>,
    pub status_message: String,
    pub recent_sessions: Vec<RecentSession>,
    pub settings: AppSettings,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            view_mode: ViewMode::Single,
            layout: MultiLayout::Horizontal,
            videos: Vec::new(),
            selected_index: None,
            playback_state: PlaybackState::Stopped,
            sync_enabled: true,
            speed: 1.0,
            master_audio_index: None,
            shared_position_sec: 0.0,
            selected_volume: 100.0,
            selected_fullscreen: false,
            auto_thumbnail_enabled: false,
            last_thumbnail_capture_sec: None,
            status_message: "Ready".to_owned(),
            recent_sessions: Vec::new(),
            settings: AppSettings::default(),
        }
    }
}

impl AppState {
    pub fn visible_video_count(&self) -> usize {
        match self.view_mode {
            ViewMode::Single => self.selected_index.map(|_| 1).unwrap_or(0),
            ViewMode::Multi => {
                let limit = match self.layout {
                    MultiLayout::Horizontal | MultiLayout::Grid2 => 2,
                    MultiLayout::Grid4 => 4,
                };
                self.videos.len().min(limit)
            }
        }
    }

    pub fn max_duration_sec(&self) -> f64 {
        self.videos
            .iter()
            .map(|video| video.duration_sec)
            .fold(0.0, f64::max)
    }

    pub fn current_thumbnail_dir_path(&self) -> Option<std::path::PathBuf> {
        let index = self.selected_index?;
        let video = self.videos.get(index)?;
        let parent = video.path.parent()?;
        Some(parent.join("thumbnails"))
    }

    pub fn current_thumbnail_dir(&self) -> Option<String> {
        self.current_thumbnail_dir_path()
            .map(|path| path.display().to_string())
    }
}
