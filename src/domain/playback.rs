#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ViewMode {
    Single,
    Multi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiLayout {
    Horizontal,
    Grid2,
    Grid4,
}

impl MultiLayout {
    pub const ALL: [Self; 3] = [Self::Horizontal, Self::Grid2, Self::Grid4];

    pub fn label(self) -> &'static str {
        match self {
            Self::Horizontal => "Horizontal",
            Self::Grid2 => "1x2",
            Self::Grid4 => "2x2",
        }
    }
}

impl ViewMode {
    pub const ALL: [Self; 2] = [Self::Single, Self::Multi];

    pub fn label(self) -> &'static str {
        match self {
            Self::Single => "Single",
            Self::Multi => "Multi",
        }
    }
}
