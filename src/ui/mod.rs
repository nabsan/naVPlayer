pub mod controls;
pub mod player_grid;
pub mod single_view;
pub mod toolbar;

use eframe::egui::{self, CentralPanel, TopBottomPanel};

use crate::app::state::AppState;
use crate::player::controller::PlayerController;

pub fn render_app(ctx: &egui::Context, state: &mut AppState, controller: &mut PlayerController) {
    TopBottomPanel::top("toolbar").show(ctx, |ui| {
        toolbar::show(ui, state, controller);
    });

    CentralPanel::default().show(ctx, |ui| {
        match state.view_mode {
            crate::domain::playback::ViewMode::Single => single_view::show(ui, state, controller),
            crate::domain::playback::ViewMode::Multi => player_grid::show(ui, state, controller),
        }
    });

    TopBottomPanel::bottom("controls").show(ctx, |ui| {
        controls::show(ui, state, controller);
    });
}
