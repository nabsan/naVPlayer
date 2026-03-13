use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::domain::playback::ViewMode;

const MAX_RECENT_SESSIONS: usize = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentSession {
    pub view_mode: ViewMode,
    pub paths: Vec<PathBuf>,
}

impl RecentSession {
    pub fn summary(&self) -> String {
        let head = self
            .paths
            .first()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("(missing)");
        let extra = self.paths.len().saturating_sub(1);
        if extra == 0 {
            format!("{} | {}", self.view_mode.label(), head)
        } else {
            format!("{} | {} (+{})", self.view_mode.label(), head, extra)
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct RecentStore {
    sessions: Vec<RecentSession>,
}

pub fn recent_store_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join("recent_files.toml")))
        .or_else(|| std::env::current_dir().ok().map(|dir| dir.join("recent_files.toml")))
        .unwrap_or_else(|| PathBuf::from("recent_files.toml"))
}

pub fn load_recent_sessions() -> Vec<RecentSession> {
    let path = recent_store_path();
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(store) = toml::from_str::<RecentStore>(&raw) else {
        return Vec::new();
    };
    store
        .sessions
        .into_iter()
        .filter(|session| !session.paths.is_empty())
        .collect()
}

pub fn record_recent_session(view_mode: ViewMode, paths: &[PathBuf]) -> Result<Vec<RecentSession>> {
    let normalized: Vec<PathBuf> = paths
        .iter()
        .filter(|path| path.exists())
        .cloned()
        .collect();
    if normalized.is_empty() {
        return Ok(load_recent_sessions());
    }

    let mut sessions = load_recent_sessions();
    sessions.retain(|session| !(session.view_mode == view_mode && session.paths == normalized));
    sessions.insert(
        0,
        RecentSession {
            view_mode,
            paths: normalized,
        },
    );
    sessions.truncate(MAX_RECENT_SESSIONS);

    let path = recent_store_path();
    let store = RecentStore {
        sessions: sessions.clone(),
    };
    let raw = toml::to_string_pretty(&store)?;
    fs::write(path, raw)?;
    Ok(sessions)
}
