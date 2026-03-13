use eframe::egui::{pos2, vec2, Rect};

use crate::domain::playback::MultiLayout;

pub fn compute_player_rects(area: Rect, count: usize, layout: MultiLayout) -> Vec<Rect> {
    if count == 0 {
        return Vec::new();
    }

    match layout {
        MultiLayout::Horizontal => horizontal(area, count),
        MultiLayout::Grid2 => grid(area, count, 2),
        MultiLayout::Grid4 => grid(area, count, 2),
    }
}

fn horizontal(area: Rect, count: usize) -> Vec<Rect> {
    let width = area.width() / count as f32;
    (0..count)
        .map(|index| {
            let min = pos2(area.left() + width * index as f32, area.top());
            Rect::from_min_size(min, vec2(width, area.height()))
        })
        .collect()
}

fn grid(area: Rect, count: usize, cols: usize) -> Vec<Rect> {
    let cols = cols.max(1);
    let rows = count.div_ceil(cols);
    let width = area.width() / cols as f32;
    let height = area.height() / rows as f32;

    (0..count)
        .map(|index| {
            let row = index / cols;
            let col = index % cols;
            let min = pos2(
                area.left() + width * col as f32,
                area.top() + height * row as f32,
            );
            Rect::from_min_size(min, vec2(width, height))
        })
        .collect()
}
