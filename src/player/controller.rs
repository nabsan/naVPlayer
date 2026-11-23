use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;
use std::time::Duration;

use super::playback_worker::PlaybackWorker;
use crate::app::state::AppState;
use crate::domain::{
    playback::{PlaybackState, ViewMode},
    video::VideoItem,
};

#[derive(Clone, PartialEq)]
struct Options {
    selected: Option<usize>,
    sync: bool,
    loop_playback: bool,
    thumbnails: bool,
    interval: f64,
    guard: f64,
    view_mode: ViewMode,
}

impl Options {
    fn from_state(state: &AppState) -> Self {
        Self {
            selected: state.selected_index,
            sync: state.sync_enabled,
            loop_playback: state.settings.loop_playback,
            thumbnails: state.auto_thumbnail_enabled,
            interval: state.settings.thumbnail_interval_sec,
            guard: state.settings.thumbnail_guard_sec,
            view_mode: state.view_mode,
        }
    }
    fn apply(&self, state: &mut AppState) {
        state.selected_index = self.selected.filter(|id| *id < state.videos.len());
        state.sync_enabled = self.sync;
        state.settings.loop_playback = self.loop_playback;
        if self.thumbnails != state.auto_thumbnail_enabled {
            state.last_thumbnail_capture_sec = None;
        }
        state.auto_thumbnail_enabled = self.thumbnails;
        state.settings.thumbnail_interval_sec = self.interval;
        state.settings.thumbnail_guard_sec = self.guard;
        state.view_mode = self.view_mode;
    }
}

enum Command {
    Load(Vec<PathBuf>, f32),
    Play(f32),
    Pause,
    Stop,
    Close(usize),
    Seek(f64),
    Speed(f32),
    Audio(Option<usize>),
    Options(Options),
    Shutdown,
}

struct Snapshot {
    revision: u64,
    state: AppState,
}

/// One worker owns every libmpv call, including creation and destruction.
pub struct PlayerController {
    tx: mpsc::Sender<(u64, Command)>,
    latest: Arc<Mutex<Option<Snapshot>>>,
    worker: Option<JoinHandle<()>>,
    revision: u64,
    options: Option<Options>,
}

impl PlayerController {
    pub fn new(ctx: egui::Context) -> Self {
        let (tx, rx) = mpsc::channel();
        let latest = Arc::new(Mutex::new(None));
        let output = latest.clone();
        let worker = std::thread::Builder::new()
            .name("navplayer-playback".into())
            .spawn(move || {
                let mut player = PlaybackWorker::new();
                let mut state = AppState::default();
                let mut revision = 0;
                loop {
                    // Idle without files: no periodic timer or UI repaint.
                    let incoming = if state.videos.is_empty() {
                        rx.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected)
                    } else {
                        rx.recv_timeout(if state.playback_state == PlaybackState::Playing {
                            Duration::from_millis(100)
                        } else {
                            Duration::from_millis(250)
                        })
                    };
                    let mut commands = Vec::new();
                    match incoming {
                        Ok(command) => commands.push(command),
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                    commands.extend(rx.try_iter().take(63));
                    let mut commands = commands.into_iter().peekable();
                    while let Some((seq, command)) = commands.next() {
                        revision = seq;
                        // Coalesce consecutive slider updates without reordering controls.
                        if matches!(command, Command::Seek(_))
                            && commands
                                .peek()
                                .is_some_and(|(_, next)| matches!(next, Command::Seek(_)))
                        {
                            continue;
                        }
                        match command {
                            Command::Shutdown => return,
                            Command::Load(paths, speed) => {
                                state.speed = speed;
                                player.load_files(&mut state, paths);
                            }
                            Command::Play(speed) => {
                                state.speed = speed;
                                player.play_all(&mut state);
                            }
                            Command::Pause => player.pause_all(&mut state),
                            Command::Stop => player.stop_all(&mut state),
                            Command::Close(index) => {
                                state.selected_index = Some(index);
                                player.close_selected(&mut state);
                            }
                            Command::Seek(position) => {
                                state.shared_position_sec = position;
                                player.seek_all(&mut state);
                            }
                            Command::Speed(speed) => player.set_speed(&mut state, speed),
                            Command::Audio(index) => {
                                state.master_audio_index =
                                    index.filter(|id| *id < state.videos.len());
                                player.sync_audio(&mut state);
                            }
                            Command::Options(options) => options.apply(&mut state),
                        }
                    }
                    if !state.videos.is_empty() {
                        player.tick(&mut state);
                    }
                    *output.lock().expect("playback snapshot") = Some(Snapshot {
                        revision,
                        state: state.clone(),
                    });
                    ctx.request_repaint();
                }
            })
            .expect("start playback worker");
        Self {
            tx,
            latest,
            worker: Some(worker),
            revision: 0,
            options: None,
        }
    }

    fn send(&mut self, state: &mut AppState, command: Command) {
        self.revision += 1;
        if self.tx.send((self.revision, command)).is_err() {
            state.loading = false;
            state.status_message = "Playback worker stopped. Please restart naVPlayer.".into();
        }
    }

    pub fn load_files(&mut self, state: &mut AppState, paths: Vec<PathBuf>) {
        let paths: Vec<_> = paths.into_iter().take(4).collect();
        if paths.is_empty() {
            return;
        }
        state.videos = paths
            .iter()
            .enumerate()
            .map(|(id, path)| VideoItem::new(id, path.clone()))
            .collect();
        state.selected_index = Some(0);
        state.master_audio_index = Some(0);
        state.exit_requested = false;
        state.loading = true;
        state.playback_state = PlaybackState::Stopped;
        state.shared_position_sec = 0.0;
        state.last_thumbnail_capture_sec = None;
        state.status_message = "Loading video(s)...".into();
        self.send(state, Command::Load(paths, state.speed));
        self.options = None;
    }
    pub fn play_all(&mut self, state: &mut AppState) {
        self.send(state, Command::Play(state.speed));
    }
    pub fn pause_all(&mut self, state: &mut AppState) {
        self.send(state, Command::Pause);
    }
    pub fn stop_all(&mut self, state: &mut AppState) {
        self.send(state, Command::Stop);
    }
    pub fn close_selected(&mut self, state: &mut AppState) {
        if let Some(index) = state.selected_index {
            self.send(state, Command::Close(index));
        }
    }
    pub fn seek_all(&mut self, state: &mut AppState) {
        self.send(state, Command::Seek(state.shared_position_sec));
    }
    pub fn set_speed(&mut self, state: &mut AppState, speed: f32) {
        state.speed = speed;
        self.send(state, Command::Speed(speed));
    }
    pub fn sync_audio(&mut self, state: &mut AppState) {
        self.send(state, Command::Audio(state.master_audio_index));
    }

    pub fn tick(&mut self, state: &mut AppState) {
        // Send local UI edits before accepting an older worker snapshot.
        let options = Options::from_state(state);
        if self.options.as_ref() != Some(&options) {
            self.send(state, Command::Options(options.clone()));
            self.options = Some(options);
        }
        let snapshot = self.latest.lock().expect("playback snapshot").take();
        // Old results may arrive after loading another file or dragging the seek bar.
        if let Some(snapshot) = snapshot.filter(|snapshot| snapshot.revision == self.revision) {
            let result = snapshot.state;
            state.videos = result.videos;
            state.selected_index = result.selected_index;
            state.master_audio_index = result.master_audio_index;
            state.playback_state = result.playback_state;
            state.shared_position_sec = result.shared_position_sec;
            state.selected_volume = result.selected_volume;
            state.selected_fullscreen = result.selected_fullscreen;
            state.last_thumbnail_capture_sec = result.last_thumbnail_capture_sec;
            state.status_message = result.status_message;
            state.auto_thumbnail_enabled = result.auto_thumbnail_enabled;
            state.exit_requested = result.exit_requested;
            state.loading = false;
            self.options = Some(Options::from_state(state));
        }
    }
    pub fn paint_video(&self, _ui: &mut egui::Ui, _index: usize, _rect: egui::Rect) {}
    pub fn backend_name(&self) -> &'static str {
        "libmpv-window (worker)"
    }
}

impl Drop for PlayerController {
    fn drop(&mut self) {
        let _ = self.tx.send((self.revision, Command::Shutdown));
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn wait_snapshot(controller: &PlayerController) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while controller
            .latest
            .lock()
            .unwrap()
            .as_ref()
            .is_none_or(|snapshot| snapshot.revision != controller.revision)
        {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn local_thumbnail_toggle_survives_an_in_flight_snapshot() {
        let mut controller = PlayerController::new(egui::Context::default());
        let mut state = AppState::default();
        controller.tick(&mut state);
        wait_snapshot(&controller);
        state.auto_thumbnail_enabled = true;
        controller.tick(&mut state);
        assert!(state.auto_thumbnail_enabled);
        // A fast worker may already have published and been consumed by tick.
        // Request another acknowledged command rather than relying on timing.
        controller.pause_all(&mut state);
        wait_snapshot(&controller);
        controller.tick(&mut state);
        assert!(state.auto_thumbnail_enabled);
    }

    #[test]
    fn old_session_cannot_exit_a_new_session() {
        let mut controller = PlayerController::new(egui::Context::default());
        let mut state = AppState::default();
        controller.tick(&mut state);
        wait_snapshot(&controller);
        let mut stale = controller.latest.lock().unwrap().take().unwrap();
        stale.state.exit_requested = true;
        controller.pause_all(&mut state);
        *controller.latest.lock().unwrap() = Some(stale);
        controller.tick(&mut state);
        assert!(!state.exit_requested);
    }
}
