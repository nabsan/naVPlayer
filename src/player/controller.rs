use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use egui::Ui;
use tracing::{error, info};

use crate::app::state::AppState;
use crate::domain::playback::{PlaybackState, ViewMode};
use crate::domain::video::VideoItem;
use crate::player::mpv_player::{BackendTick, LibMpvBackend, PlayerBackend};

pub struct PlayerController {
    backend: Box<dyn PlayerBackend>,
}

impl PlayerController {
    pub fn new() -> Self {
        Self {
            backend: Box::new(LibMpvBackend::default()),
        }
    }

    pub fn load_files(&mut self, state: &mut AppState, paths: Vec<PathBuf>) {
        let capped_paths: Vec<_> = paths.into_iter().take(4).collect();
        state.videos = capped_paths
            .into_iter()
            .enumerate()
            .map(|(index, path)| VideoItem::new(index, path))
            .collect();
        state.selected_index = state.videos.first().map(|video| video.id);
        state.master_audio_index = state.selected_index;
        state.shared_position_sec = 0.0;
        state.selected_volume = 100.0;
        state.selected_fullscreen = false;
        state.last_thumbnail_capture_sec = None;

        let success = format!(
            "Loaded {} video(s) via {}",
            state.videos.len(),
            self.backend.backend_name()
        );
        let result = self
            .backend
            .replace_playlist(&mut state.videos, state.master_audio_index, state.speed);
        self.handle_result(state, result, success);
    }

    pub fn play_all(&mut self, state: &mut AppState) {
        let result = self.backend.play(state.speed);
        let ok = result.is_ok();
        self.handle_result(state, result, "Playing".to_owned());
        if ok {
            state.playback_state = PlaybackState::Playing;
        }
        info!("play all");
    }

    pub fn pause_all(&mut self, state: &mut AppState) {
        let result = self.backend.pause();
        let ok = result.is_ok();
        self.handle_result(state, result, "Paused".to_owned());
        if ok {
            state.playback_state = PlaybackState::Paused;
        }
    }

    pub fn stop_all(&mut self, state: &mut AppState) {
        let result = self.backend.stop();
        let ok = result.is_ok();
        for video in &mut state.videos {
            video.position_sec = 0.0;
        }
        state.shared_position_sec = 0.0;
        self.handle_result(state, result, "Stopped".to_owned());
        if ok {
            state.playback_state = PlaybackState::Stopped;
        }
    }

    pub fn close_selected(&mut self, state: &mut AppState) {
        let Some(index) = state.selected_index else {
            return;
        };
        let result = self.backend.close_player(index);
        self.handle_result(state, result, format!("Closing player {index}"));
        self.remove_players_from_state(state, &[index]);
    }

    pub fn seek_all(&mut self, state: &mut AppState) {
        for video in &mut state.videos {
            video.position_sec = state.shared_position_sec;
        }
        let result = self.backend.seek(state.shared_position_sec);
        self.handle_result(state, result, format!("Seeked to {:.2}s", state.shared_position_sec));
    }

    pub fn set_speed(&mut self, state: &mut AppState, speed: f32) {
        state.speed = speed;
        let result = self.backend.set_speed(speed);
        self.handle_result(state, result, format!("Speed {:.2}x", speed));
    }

    pub fn sync_audio(&mut self, state: &mut AppState) {
        for video in &mut state.videos {
            video.muted = Some(video.id) != state.master_audio_index;
        }
        let result = self.backend.set_master_audio(state.master_audio_index);
        self.handle_result(state, result, "Updated master audio".to_owned());
    }

    pub fn tick(&mut self, state: &mut AppState) {
        let snapshot = self.backend.tick(
            &mut state.videos,
            state.playback_state,
            state.selected_index,
            state.sync_enabled,
            state.settings.loop_playback,
        );
        self.apply_backend_snapshot(state, snapshot);
        self.capture_thumbnail_if_needed(state);
    }

    pub fn paint_video(&self, ui: &mut Ui, index: usize, rect: egui::Rect) {
        if let Some(handle) = self.backend.render_handle(index) {
            ui.painter().add(handle.paint_callback(rect));
        }
    }

    pub fn backend_name(&self) -> &'static str {
        self.backend.backend_name()
    }

    fn apply_backend_snapshot(&mut self, state: &mut AppState, snapshot: BackendTick) {
        if !snapshot.closed_player_ids.is_empty() {
            self.remove_players_from_state(state, &snapshot.closed_player_ids);
        }
        if let Some(anchor_pos) = snapshot.anchor_position {
            state.shared_position_sec = anchor_pos;
        }
        if let Some(playback_state) = snapshot.playback_state {
            state.playback_state = playback_state;
        }
        if let Some(volume) = snapshot.selected_volume {
            state.selected_volume = volume;
        }
        if let Some(fullscreen) = snapshot.selected_fullscreen {
            state.selected_fullscreen = fullscreen;
        }
        if let Some(render_status) = snapshot.selected_render_status {
            state.status_message = render_status;
        }
    }

    fn remove_players_from_state(&mut self, state: &mut AppState, ids: &[usize]) {
        self.backend.remove_players(ids);
        state.videos.retain(|video| !ids.contains(&video.id));
        for (index, video) in state.videos.iter_mut().enumerate() {
            video.id = index;
        }
        state.selected_index = state.videos.first().map(|video| video.id);
        if let Some(current_audio) = state.master_audio_index {
            if ids.contains(&current_audio) {
                state.master_audio_index = state.selected_index;
            } else if current_audio >= state.videos.len() {
                state.master_audio_index = state.selected_index;
            }
        }
        if state.videos.is_empty() {
            state.master_audio_index = None;
            state.playback_state = PlaybackState::Stopped;
            state.shared_position_sec = 0.0;
        }
    }

    fn capture_thumbnail_if_needed(&mut self, state: &mut AppState) {
        if !state.auto_thumbnail_enabled || state.view_mode != ViewMode::Single {
            return;
        }
        if state.playback_state != PlaybackState::Playing {
            return;
        }
        let Some(index) = state.selected_index else {
            return;
        };
        let Some(video) = state.videos.get(index) else {
            return;
        };
        let position_sec = video.position_sec.max(0.0);
        let duration_sec = video.duration_sec.max(0.0);
        let interval_sec = state.settings.thumbnail_interval_sec.max(1.0);
        let guard_sec = state.settings.thumbnail_guard_sec.max(0.5);

        if duration_sec > 0.0 && duration_sec - position_sec <= guard_sec {
            return;
        }

        if let Some(last_sec) = state.last_thumbnail_capture_sec {
            if position_sec - last_sec < interval_sec {
                return;
            }
        }

        let Ok(output_path) = thumbnail_output_path(video, position_sec) else {
            return;
        };

        let result = self.backend.capture_frame(index, &output_path);
        match result {
            Ok(()) => {
                state.last_thumbnail_capture_sec = Some(position_sec);
                state.status_message = format!("Saved thumbnail: {}", output_path.display());
            }
            Err(err) => {
                error!("thumbnail capture error: {err:#}");
                state.status_message = format!("Thumbnail error: {err:#}");
                state.auto_thumbnail_enabled = false;
            }
        }
    }

    fn handle_result(&self, state: &mut AppState, result: Result<()>, success: String) {
        match result {
            Ok(()) => state.status_message = success,
            Err(err) => {
                error!("player backend error: {err:#}");
                state.status_message = format!("Playback error: {err:#}");
            }
        }
    }
}


fn thumbnail_output_path(video: &VideoItem, position_sec: f64) -> Result<PathBuf> {
    let parent = video
        .path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("video parent directory unavailable"))?;
    let output_dir = parent.join("thumbnails");
    fs::create_dir_all(&output_dir)?;

    let stem = video
        .path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("thumbnail");
    let safe_stem = sanitize_filename(stem);
    let filename = format!("{}_{}.jpg", safe_stem, format_thumbnail_timestamp(position_sec));
    Ok(output_dir.join(filename))
}

fn format_thumbnail_timestamp(position_sec: f64) -> String {
    let millis = (position_sec.max(0.0) * 1000.0).round() as u64;
    let minutes = millis / 60_000;
    let seconds = (millis % 60_000) / 1000;
    let ms = millis % 1000;
    format!("{minutes:02}m{seconds:02}s{ms:03}ms")
}

fn sanitize_filename(input: &str) -> String {
    let sanitized: String = input
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => ch,
        })
        .collect();
    sanitized.trim().trim_matches('.').to_owned()
}
