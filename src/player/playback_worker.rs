use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use tracing::{error, info};

use crate::app::state::AppState;
use crate::domain::playback::{PlaybackState, ViewMode};
use crate::domain::video::VideoItem;
use crate::player::mpv_player::{BackendTick, LibMpvBackend, PlayerBackend};

pub struct PlaybackWorker {
    backend: Box<dyn PlayerBackend>,
}

impl PlaybackWorker {
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
        state.master_audio_index = state
            .videos
            .iter()
            .find(|video| !video.is_image())
            .map(|video| video.id);
        state.shared_position_sec = 0.0;
        state.selected_volume = 100.0;
        state.selected_fullscreen = false;
        state.last_thumbnail_capture_sec = None;
        state.exit_requested = false;
        state.playback_state = PlaybackState::Stopped;

        let success = format!(
            "Loaded {} video(s) via {}",
            state.videos.len(),
            self.backend.backend_name()
        );
        let result =
            self.backend
                .replace_playlist(&mut state.videos, state.master_audio_index, state.speed);
        if result.is_err() {
            state.videos.clear();
            state.selected_index = None;
            state.master_audio_index = None;
        }
        self.handle_result(state, result, success);
    }

    pub fn play_all(&mut self, state: &mut AppState) {
        if state.videos.is_empty() {
            return;
        }
        if state.videos.iter().all(VideoItem::is_image) {
            state.playback_state = PlaybackState::Paused;
            state.status_message = "Displaying image(s)".into();
            return;
        }
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
        if !state.videos.iter().any(|video| video.id == index) {
            return;
        }
        let result = self.backend.close_player(index);
        self.handle_result(state, result, format!("Closing player {index}"));
        self.remove_players_from_state(state, &[index]);
    }

    pub fn seek_all(&mut self, state: &mut AppState) {
        for video in &mut state.videos {
            if !video.is_image() {
                video.position_sec = state.shared_position_sec;
            }
        }
        let result = self.backend.seek(state.shared_position_sec);
        self.handle_result(
            state,
            result,
            format!("Seeked to {:.2}s", state.shared_position_sec),
        );
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
        // n/p can switch a window between a still image and a video.
        let audio = state
            .master_audio_index
            .filter(|id| state.videos.get(*id).is_some_and(|video| !video.is_image()))
            .or_else(|| {
                state
                    .videos
                    .iter()
                    .find(|video| !video.is_image())
                    .map(|video| video.id)
            });
        if state.master_audio_index != audio {
            state.master_audio_index = audio;
            let _ = self.backend.set_master_audio(audio);
            for video in &mut state.videos {
                video.muted = Some(video.id) != audio;
            }
        }
        self.capture_thumbnail_if_needed(state);
    }

    fn apply_backend_snapshot(&mut self, state: &mut AppState, snapshot: BackendTick) {
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
        if !snapshot.closed_player_ids.is_empty() {
            self.remove_players_from_state(state, &snapshot.closed_player_ids);
            if snapshot.failed {
                state.exit_requested = false;
            }
        }
        if let Some(render_status) = snapshot.selected_render_status {
            state.status_message = render_status;
        }
    }

    fn remove_players_from_state(&mut self, state: &mut AppState, ids: &[usize]) {
        let had_players = !state.videos.is_empty();
        let remap = |old: Option<usize>| {
            old.and_then(|old| {
                state
                    .videos
                    .iter()
                    .filter(|video| !ids.contains(&video.id))
                    .position(|video| video.id == old)
            })
        };
        let selected = remap(state.selected_index);
        let audio = remap(state.master_audio_index);
        self.backend.remove_players(ids);
        state.videos.retain(|video| !ids.contains(&video.id));
        for (index, video) in state.videos.iter_mut().enumerate() {
            video.id = index;
        }
        state.selected_index = selected.or_else(|| state.videos.first().map(|video| video.id));
        state.master_audio_index = audio
            .filter(|id| !state.videos[*id].is_image())
            .or_else(|| {
                state
                    .videos
                    .iter()
                    .find(|video| !video.is_image())
                    .map(|video| video.id)
            });
        if !state.videos.is_empty() {
            self.sync_audio(state);
        }
        if state.videos.is_empty() {
            state.master_audio_index = None;
            state.playback_state = PlaybackState::Stopped;
            state.shared_position_sec = 0.0;
            state.exit_requested = had_players;
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
        if video.is_image() {
            return;
        }
        let position_sec = video.position_sec.max(0.0);
        let duration_sec = video.duration_sec.max(0.0);
        let interval_sec = state.settings.thumbnail_interval_sec.max(1.0);
        let guard_sec = state.settings.thumbnail_guard_sec.max(0.5);

        if duration_sec > 0.0 && duration_sec - position_sec <= guard_sec {
            return;
        }

        if let Some(last_sec) = state.last_thumbnail_capture_sec
            && position_sec - last_sec < interval_sec
        {
            return;
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
    let filename = format!(
        "{}_{}.jpg",
        safe_stem,
        format_thumbnail_timestamp(position_sec)
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    fn two_videos() -> AppState {
        AppState {
            videos: vec![
                VideoItem::new(0, "a.mp4".into()),
                VideoItem::new(1, "b.mkv".into()),
            ],
            selected_index: Some(1),
            master_audio_index: Some(1),
            playback_state: PlaybackState::Playing,
            ..AppState::default()
        }
    }
    fn closed(ids: Vec<usize>, failed: bool) -> BackendTick {
        BackendTick {
            closed_player_ids: ids,
            failed,
            anchor_position: None,
            playback_state: None,
            selected_volume: None,
            selected_fullscreen: None,
            selected_render_status: failed.then(|| "Playback error: unsupported data".into()),
        }
    }

    #[test]
    fn closing_first_keeps_second_and_remaps_audio_then_last_requests_exit() {
        let mut player = PlaybackWorker::new();
        let mut state = two_videos();
        player.apply_backend_snapshot(&mut state, closed(vec![0], false));
        assert!(!state.exit_requested);
        assert_eq!(state.videos[0].path, PathBuf::from("b.mkv"));
        assert_eq!(state.selected_index, Some(0));
        assert_eq!(state.master_audio_index, Some(0));
        assert!(!state.videos[0].muted);
        player.apply_backend_snapshot(&mut state, closed(vec![0], false));
        assert!(state.exit_requested);
        assert_eq!(state.playback_state, PlaybackState::Stopped);
    }

    #[test]
    fn simultaneous_close_exits_but_decoder_failure_keeps_error_visible() {
        let mut player = PlaybackWorker::new();
        let mut state = two_videos();
        player.apply_backend_snapshot(&mut state, closed(vec![0, 1], false));
        assert!(state.exit_requested);
        let mut state = two_videos();
        player.apply_backend_snapshot(&mut state, closed(vec![0, 1], true));
        assert!(!state.exit_requested);
        assert!(state.videos.is_empty());
        assert!(state.status_message.starts_with("Playback error:"));
        assert!(!AppState::default().exit_requested);
    }

    #[test]
    fn closing_video_keeps_image_open_without_master_audio() {
        let mut player = PlaybackWorker::new();
        let mut state = two_videos();
        state.videos[0].path = "still.png".into();
        player.apply_backend_snapshot(&mut state, closed(vec![1], false));
        assert!(!state.exit_requested);
        assert!(state.videos[0].is_image());
        assert_eq!(state.master_audio_index, None);
        player.play_all(&mut state);
        assert_eq!(state.playback_state, PlaybackState::Paused);
        player.apply_backend_snapshot(&mut state, closed(vec![0], false));
        assert!(state.exit_requested);
    }
}
