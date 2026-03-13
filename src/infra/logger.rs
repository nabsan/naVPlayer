use tracing::error;

pub fn log_error(message: &str) {
    error!("{message}");
}
