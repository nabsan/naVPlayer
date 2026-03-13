#[allow(dead_code)]
use crate::app::state::AppState;

#[allow(dead_code)]
pub fn sync_positions(state: &mut AppState) {
    if state.videos.is_empty() {
        state.shared_position_sec = 0.0;
        return;
    }

    if state.sync_enabled {
        let anchor = state
            .selected_index
            .and_then(|index| state.videos.get(index))
            .map(|video| video.position_sec)
            .unwrap_or(state.shared_position_sec);

        for video in &mut state.videos {
            if (video.position_sec - anchor).abs() > 0.1 {
                video.position_sec = anchor;
            }
        }
        state.shared_position_sec = anchor;
    } else {
        state.shared_position_sec = state
            .selected_index
            .and_then(|index| state.videos.get(index))
            .map(|video| video.position_sec)
            .unwrap_or(state.shared_position_sec);
    }
}
