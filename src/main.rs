#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod domain;
mod infra;
mod player;
mod ui;
mod util;

use std::path::PathBuf;

use app::{APP_VERSION, NaVPlayerApp};
use eframe::{egui, Renderer};
use infra::ipc;
use infra::logger;
use tracing_subscriber::EnvFilter;

fn main() {
    init_logging();
    let launch_paths = collect_launch_paths();
    let ipc_message = if launch_paths.is_empty() {
        ipc::IpcMessage::show()
    } else {
        ipc::IpcMessage::open(launch_paths.clone())
    };
    if ipc::send_to_existing_instance(&ipc_message) {
        return;
    }
    let ipc_rx = ipc::start_server();

    let options = eframe::NativeOptions {
        renderer: Renderer::Glow,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([960.0, 640.0])
            .with_title(format!("naVPlayer {}", APP_VERSION)),
        centered: true,
        ..Default::default()
    };

    if let Err(error) = eframe::run_native(
        &format!("naVPlayer {}", APP_VERSION),
        options,
        Box::new(move |cc| Ok(Box::new(NaVPlayerApp::new(cc, launch_paths.clone(), ipc_rx)))),
    ) {
        logger::log_error(&format!("failed to start naVPlayer: {error}"));
        eprintln!("failed to start naVPlayer: {error}");
    }
}

fn collect_launch_paths() -> Vec<PathBuf> {
    std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "mp4" | "mov"))
                .unwrap_or(false)
        })
        .collect()
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}
