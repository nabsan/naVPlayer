use std::path::PathBuf;

pub fn app_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("naVPlayer").join("config.toml"))
}
