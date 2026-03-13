#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum PlayerEvent {
    Loaded,
    PlaybackEnded,
    Error(String),
}
