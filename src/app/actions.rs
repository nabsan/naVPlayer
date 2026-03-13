use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::app::state::AppState;
use crate::domain::playback::{MultiLayout, ViewMode};
use crate::infra::recent;
use crate::player::controller::PlayerController;

pub fn open_files(
    state: &mut AppState,
    controller: &mut PlayerController,
    paths: Vec<PathBuf>,
) {
    let record_paths = paths.clone();
    controller.load_files(state, paths);
    refresh_recent_sessions(state, record_paths);
}

pub fn open_recent(state: &mut AppState, controller: &mut PlayerController, index: usize) {
    let Some(session) = state.recent_sessions.get(index).cloned() else {
        return;
    };
    state.view_mode = session.view_mode;
    let paths = session.paths;
    controller.load_files(state, paths.clone());
    refresh_recent_sessions(state, paths);
}

pub fn set_view_mode(state: &mut AppState, mode: ViewMode) {
    state.view_mode = mode;
}

pub fn set_layout(state: &mut AppState, layout: MultiLayout) {
    state.layout = layout;
}

pub fn set_speed(state: &mut AppState, controller: &mut PlayerController, speed: f32) {
    controller.set_speed(state, speed);
}

pub fn toggle_sync(state: &mut AppState) {
    state.sync_enabled = !state.sync_enabled;
}

pub fn play(state: &mut AppState, controller: &mut PlayerController) {
    controller.play_all(state);
}

pub fn pause(state: &mut AppState, controller: &mut PlayerController) {
    controller.pause_all(state);
}

pub fn stop(state: &mut AppState, controller: &mut PlayerController) {
    controller.stop_all(state);
}

pub fn seek(state: &mut AppState, controller: &mut PlayerController, position: f64) {
    state.shared_position_sec = position.clamp(0.0, state.max_duration_sec());
    controller.seek_all(state);
}

pub fn select_video(state: &mut AppState, index: usize) {
    state.selected_index = Some(index);
    if state.view_mode == ViewMode::Single {
        state.master_audio_index = Some(index);
    }
}

pub fn set_master_audio(state: &mut AppState, controller: &mut PlayerController, index: usize) {
    state.master_audio_index = Some(index);
    controller.sync_audio(state);
}


pub fn close_selected(state: &mut AppState, controller: &mut PlayerController) {
    controller.close_selected(state);
}



pub fn toggle_auto_thumbnail(state: &mut AppState) {
    state.auto_thumbnail_enabled = !state.auto_thumbnail_enabled;
    state.last_thumbnail_capture_sec = None;
}


pub fn open_thumbnail_folder(state: &mut AppState) {
    let Some(dir) = state.current_thumbnail_dir_path() else {
        state.status_message = "Thumbnail folder unavailable".to_owned();
        return;
    };

    if let Err(err) = fs::create_dir_all(&dir) {
        state.status_message = format!("Failed to create thumbnail folder: {err}");
        return;
    }

    match Command::new("explorer.exe").arg(&dir).spawn() {
        Ok(_) => state.status_message = format!("Opened thumbnail folder: {}", dir.display()),
        Err(err) => state.status_message = format!("Failed to open thumbnail folder: {err}"),
    }
}


fn refresh_recent_sessions(state: &mut AppState, paths: Vec<PathBuf>) {
    if !state.settings.remember_last_files {
        return;
    }

    match recent::record_recent_session(state.view_mode, &paths) {
        Ok(sessions) => state.recent_sessions = sessions,
        Err(err) => state.status_message = format!("Recent files save error: {err}") ,
    }
}
