use eframe::egui::{Slider, Ui};

use crate::app::actions;
use crate::app::state::AppState;
use crate::player::controller::PlayerController;
use crate::util::time::format_timestamp;

pub fn show(ui: &mut Ui, state: &mut AppState, controller: &mut PlayerController) {
    let has_video = state.videos.iter().any(|video| !video.is_image());
    ui.vertical(|ui| {
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(has_video, egui::Button::new("Play"))
                .clicked()
            {
                actions::play(state, controller);
            }
            if ui
                .add_enabled(has_video, egui::Button::new("Pause"))
                .clicked()
            {
                actions::pause(state, controller);
            }
            if ui
                .add_enabled(has_video, egui::Button::new("Stop"))
                .clicked()
            {
                actions::stop(state, controller);
            }
            if ui.button("Close Selected").clicked() {
                actions::close_selected(state, controller);
            }

            let mut position = state.shared_position_sec;
            let max_duration = state.max_duration_sec().max(1.0);
            let slider_width = (ui.available_width() - 280.0).max(200.0);
            ui.add_enabled_ui(has_video, |ui| {
                ui.add_sized(
                    [slider_width, 18.0],
                    Slider::new(&mut position, 0.0..=max_duration).show_value(false),
                )
            });
            if (position - state.shared_position_sec).abs() > f64::EPSILON {
                actions::seek(state, controller, position);
            }

            if has_video {
                ui.label(format!(
                    "{} / {}",
                    format_timestamp(state.shared_position_sec),
                    format_timestamp(max_duration)
                ));
            } else if !state.videos.is_empty() {
                ui.label("Still image");
            }
        });

        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Selected volume: {:.0}", state.selected_volume));
            ui.separator();
            ui.label(format!(
                "Fullscreen: {}",
                if state.selected_fullscreen {
                    "ON"
                } else {
                    "OFF"
                }
            ));
            ui.separator();
            ui.label(format!(
                "State: {}",
                match state.playback_state {
                    crate::domain::playback::PlaybackState::Playing => "Playing",
                    crate::domain::playback::PlaybackState::Paused => "Paused",
                    crate::domain::playback::PlaybackState::Stopped => "Stopped",
                }
            ));
        });

        if !state.videos.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.label("Audio");
                let items: Vec<(usize, String, bool)> = state
                    .videos
                    .iter()
                    .filter(|video| !video.is_image())
                    .map(|video| {
                        (
                            video.id,
                            video.title.clone(),
                            Some(video.id) == state.master_audio_index,
                        )
                    })
                    .collect();
                for (id, title, selected) in items {
                    if ui.selectable_label(selected, title).clicked() {
                        actions::set_master_audio(state, controller, id);
                    }
                }
            });
        }
    });
}
