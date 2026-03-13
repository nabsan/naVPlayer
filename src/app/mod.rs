pub mod actions;
pub mod layout;
pub mod state;

use std::path::PathBuf;
use std::time::Duration;

use eframe::{CreationContext, egui::{self, ViewportCommand}};

use crate::domain::playback::ViewMode;
use crate::infra::config;
use crate::infra::recent;
use crate::player::controller::PlayerController;
use crate::ui;

use self::state::AppState;

pub const APP_VERSION: &str = env!("NAVPLAYER_VERSION");

pub struct NaVPlayerApp {
    pub state: AppState,
    pub controller: PlayerController,
    main_window_minimized: bool,
}

impl NaVPlayerApp {
    pub fn new(_cc: &CreationContext<'_>, launch_paths: Vec<PathBuf>) -> Self {
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

        if !launch_paths.is_empty() {
            let record_paths = launch_paths.clone();
            controller.load_files(&mut state, launch_paths);
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
        }
    }

    fn sync_parent_window_visibility(&mut self, ctx: &egui::Context) {
        let should_minimize = self.state.view_mode == ViewMode::Single && !self.state.videos.is_empty();

        if should_minimize && !self.main_window_minimized {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
            self.main_window_minimized = true;
            return;
        }

        if !should_minimize && self.main_window_minimized {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(ViewportCommand::Focus);
            self.main_window_minimized = false;
        }
    }
}

impl eframe::App for NaVPlayerApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        self.controller.tick(&mut self.state);
        self.sync_parent_window_visibility(ctx);
        ui::render_app(ctx, &mut self.state, &mut self.controller);
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}
