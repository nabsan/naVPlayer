use eframe::egui::{self, Button, RichText, Ui};

use crate::app::actions;
use crate::app::APP_VERSION;
use crate::app::state::AppState;
use crate::domain::playback::{MultiLayout, ViewMode};
use crate::infra::file_dialog;
use crate::player::controller::PlayerController;

pub fn show(ui: &mut Ui, state: &mut AppState, controller: &mut PlayerController) {
    ui.horizontal_wrapped(|ui| {
        if ui.add(Button::new("Open File(s)")).clicked() {
            let files = file_dialog::pick_video_files();
            if !files.is_empty() {
                actions::open_files(state, controller, files);
            }
        }

        if !state.recent_sessions.is_empty() {
            recent_selector(ui, state, controller);
            ui.separator();
        }

        ui.separator();
        mode_selector(ui, state);
        ui.separator();
        layout_selector(ui, state);
        ui.separator();

        let sync_label = if state.sync_enabled { "Sync ON" } else { "Sync OFF" };
        if ui.button(sync_label).clicked() {
            actions::toggle_sync(state);
        }

        ui.separator();
        speed_selector(ui, state, controller);

        if state.view_mode == ViewMode::Single {
            ui.separator();
            let mut enabled = state.auto_thumbnail_enabled;
            if ui.checkbox(&mut enabled, "Get Thumbnail").changed() {
                actions::toggle_auto_thumbnail(state);
            }
            if let Some(dir) = state.current_thumbnail_dir() {
                ui.label(RichText::new(format!("Save to: {dir}")).small());
                if ui.button("Open Thumbnails Folder").clicked() {
                    actions::open_thumbnail_folder(state);
                }
            }
        }

        ui.separator();
        ui.label(RichText::new(APP_VERSION).small().monospace());
        ui.separator();
        ui.label(RichText::new(&state.status_message).small().italics());
    });
}

fn mode_selector(ui: &mut Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.label("Mode");
        for mode in ViewMode::ALL {
            let selected = state.view_mode == mode;
            if ui.selectable_label(selected, mode.label()).clicked() {
                actions::set_view_mode(state, mode);
            }
        }
    });
}

fn layout_selector(ui: &mut Ui, state: &mut AppState) {
    egui::ComboBox::from_label("Layout")
        .selected_text(state.layout.label())
        .show_ui(ui, |ui| {
            for layout in MultiLayout::ALL {
                let selected = state.layout == layout;
                if ui.selectable_label(selected, layout.label()).clicked() {
                    actions::set_layout(state, layout);
                }
            }
        });
}

fn speed_selector(ui: &mut Ui, state: &mut AppState, controller: &mut PlayerController) {
    const SPEEDS: [f32; 5] = [0.5, 0.75, 1.0, 1.25, 1.5];
    egui::ComboBox::from_label("Speed")
        .selected_text(format!("{:.2}x", state.speed))
        .show_ui(ui, |ui| {
            for speed in SPEEDS {
                if ui
                    .selectable_label((state.speed - speed).abs() < f32::EPSILON, format!("{speed:.2}x"))
                    .clicked()
                {
                    actions::set_speed(state, controller, speed);
                }
            }
        });
}

fn recent_selector(ui: &mut Ui, state: &mut AppState, controller: &mut PlayerController) {
    egui::ComboBox::from_label("Recent")
        .selected_text("Recent Sessions")
        .show_ui(ui, |ui| {
            let items: Vec<String> = state.recent_sessions.iter().map(|session| session.summary()).collect();
            for (index, label) in items.into_iter().enumerate() {
                if ui.selectable_label(false, label).clicked() {
                    actions::open_recent(state, controller, index);
                    ui.close();
                }
            }
        });
}
