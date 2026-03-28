pub mod actions;
pub mod layout;
pub mod state;

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use eframe::{
    CreationContext,
    egui::{self, ViewportCommand},
};

use crate::infra::config;
use crate::infra::ipc::IpcMessage;
use crate::infra::recent;
use crate::player::controller::PlayerController;
use crate::ui;

use self::state::AppState;

pub const APP_VERSION: &str = env!("NAVPLAYER_VERSION");

pub struct NaVPlayerApp {
    pub state: AppState,
    pub controller: PlayerController,
    main_window_minimized: bool,
    auto_minimize_single_launch: bool,
    ipc_rx: Receiver<IpcMessage>,
}

impl NaVPlayerApp {
    pub fn new(
        _cc: &CreationContext<'_>,
        launch_paths: Vec<PathBuf>,
        ipc_rx: Receiver<IpcMessage>,
    ) -> Self {
        let mut controller = PlayerController::new();
        let mut state = AppState::default();
        let config_note = config::app_config_path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "config path unavailable".to_owned());
        state.status_message = format!(
            "Backend: {} | {} | external mpv window mode",
            controller.backend_name(),
            config_note
        );
        state.recent_sessions = recent::load_recent_sessions();

        let launched_with_files = !launch_paths.is_empty();

        if launched_with_files {
            let record_paths = launch_paths.clone();
            controller.load_files(&mut state, launch_paths);
            controller.play_all(&mut state);
            if state.settings.remember_last_files {
                if let Ok(sessions) = recent::record_recent_session(state.view_mode, &record_paths) {
                    state.recent_sessions = sessions;
                }
            }
        }

        Self {
            state,
            controller,
            main_window_minimized: false,
            auto_minimize_single_launch: launched_with_files,
            ipc_rx,
        }
    }

    fn sync_parent_window_visibility(&mut self, ctx: &egui::Context) {
        if self.auto_minimize_single_launch {
            if !self.main_window_minimized {
                ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                self.main_window_minimized = true;
            }
            return;
        }

        if self.main_window_minimized {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(ViewportCommand::Focus);
            self.main_window_minimized = false;
        }
    }

    fn handle_ipc_messages(&mut self, ctx: &egui::Context) {
        while let Ok(message) = self.ipc_rx.try_recv() {
            match message.command.as_str() {
                "open" => {
                    let paths = message.paths;
                    if paths.is_empty() {
                        continue;
                    }
                    self.state.view_mode = if paths.len() > 1 {
                        crate::domain::playback::ViewMode::Multi
                    } else {
                        crate::domain::playback::ViewMode::Single
                    };
                    self.controller.load_files(&mut self.state, paths.clone());
                    self.controller.play_all(&mut self.state);
                    self.auto_minimize_single_launch = true;
                    if self.state.settings.remember_last_files {
                        if let Ok(sessions) =
                            recent::record_recent_session(self.state.view_mode, &paths)
                        {
                            self.state.recent_sessions = sessions;
                        }
                    }
                }
                "show" => {
                    self.auto_minimize_single_launch = false;
                    ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(ViewportCommand::Focus);
                    self.main_window_minimized = false;
                }
                _ => {}
            }
        }
    }
}

impl eframe::App for NaVPlayerApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        self.handle_ipc_messages(ctx);
        self.controller.tick(&mut self.state);
        self.sync_parent_window_visibility(ctx);
        ui::render_app(ctx, &mut self.state, &mut self.controller);
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}
