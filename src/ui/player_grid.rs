use eframe::egui::{Align2, Color32, FontId, Sense, Stroke, StrokeKind, Ui};

use crate::app::actions;
use crate::app::layout::compute_player_rects;
use crate::app::state::AppState;
use crate::player::controller::PlayerController;

pub fn show(ui: &mut Ui, state: &mut AppState, controller: &mut PlayerController) {
    let available = ui.available_size();
    let (outer_rect, _) = ui.allocate_exact_size(available, Sense::hover());
    let rects = compute_player_rects(outer_rect, state.visible_video_count(), state.layout);
    let painter = ui.painter_at(outer_rect);

    if rects.is_empty() {
        painter.text(
            outer_rect.center(),
            Align2::CENTER_CENTER,
            "Open 2 videos to compare movement",
            FontId::proportional(26.0),
            Color32::from_gray(180),
        );
        return;
    }

    for (index, rect) in rects.iter().enumerate() {
        let (title, position_sec, duration_sec, muted) = match state.videos.get(index) {
            Some(video) => (video.title.clone(), video.position_sec, video.duration_sec, video.muted),
            None => continue,
        };
        let id = ui.make_persistent_id(("video_tile", index));
        let response = ui.interact(*rect, id, Sense::click());
        let selected = state.selected_index == Some(index);
        let fill = if selected {
            Color32::from_rgb(28, 40, 54)
        } else {
            Color32::from_rgb(12, 14, 18)
        };
        painter.rect_filled(*rect, 12.0, fill);
        painter.rect_stroke(
            rect.shrink(2.0),
            12.0,
            Stroke::new(
                if selected { 2.0 } else { 1.0 },
                if selected {
                    Color32::from_rgb(104, 180, 255)
                } else {
                    Color32::from_gray(70)
                },
            ),
            StrokeKind::Inside,
        );
        let image_rect = rect.shrink(6.0);
        controller.paint_video(ui, index, image_rect);
        painter.text(
            rect.left_top() + eframe::egui::vec2(16.0, 16.0),
            Align2::LEFT_TOP,
            format!(
                "{}\n{:.1}s / {:.1}s\n{}",
                title,
                position_sec,
                duration_sec,
if muted { "Muted | external mpv window" } else { "Master audio | external mpv window" }
            ),
            FontId::proportional(18.0),
            Color32::WHITE,
        );
        if response.clicked() {
            actions::select_video(state, index);
        }
    }
}


