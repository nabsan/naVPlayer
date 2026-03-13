use eframe::egui::{Align2, Color32, FontId, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::app::state::AppState;
use crate::player::controller::PlayerController;

pub fn show(ui: &mut Ui, state: &mut AppState, controller: &mut PlayerController) {
    let available = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(available, Sense::click());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 16.0, Color32::from_rgb(12, 14, 18));
    painter.rect_stroke(
        rect.shrink(2.0),
        16.0,
        Stroke::new(1.0, Color32::from_gray(70)),
        StrokeKind::Inside,
    );

    if let Some(index) = state.selected_index {
        let image_rect = rect.shrink(6.0);
        controller.paint_video(ui, index, image_rect);
        if let Some(video) = state.videos.get(index) {
            painter.text(
                rect.left_top() + eframe::egui::vec2(18.0, 18.0),
                Align2::LEFT_TOP,
                format!("{}\n{:.1}s / {:.1}s\nExternal mpv window", video.title, video.position_sec, video.duration_sec),
                FontId::proportional(18.0),
                Color32::WHITE,
            );
        }
    } else {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "Drop or open a .mp4 / .mov file",
            FontId::proportional(28.0),
            Color32::from_gray(180),
        );
    }

    if response.double_clicked() {
        ui.scroll_to_rect(rect, Some(eframe::egui::Align::Center));
    }
    ui.allocate_space(Vec2::ZERO);
}


