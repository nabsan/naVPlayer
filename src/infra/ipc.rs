use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};

// The legacy player uses 43871. Never forward new features to that old binary.
const IPC_ADDR: &str = "127.0.0.1:43873";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcMessage {
    pub command: String,
    pub paths: Vec<PathBuf>,
}

impl IpcMessage {
    pub fn open(paths: Vec<PathBuf>) -> Self {
        Self {
            command: "open".to_owned(),
            paths,
        }
    }

    pub fn show() -> Self {
        Self {
            command: "show".to_owned(),
            paths: Vec::new(),
        }
    }
}

pub fn send_to_existing_instance(message: &IpcMessage) -> bool {
    let timeout = Duration::from_millis(200);
    let Ok(addr) = ipc_addr().parse() else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, timeout) else {
        return false;
    };
    let Ok(payload) = toml::to_string(message) else {
        return false;
    };
    stream.write_all(payload.as_bytes()).is_ok()
}

pub struct IpcInbox {
    pub receiver: Receiver<IpcMessage>,
    wakeup: Arc<Mutex<Option<egui::Context>>>,
}

impl IpcInbox {
    pub fn attach(&self, ctx: egui::Context) {
        *self.wakeup.lock().expect("IPC wakeup") = Some(ctx);
    }
}

fn ipc_addr() -> String {
    // Separate loopback endpoint for smoke tests or a portable installation.
    std::env::var("NAVPLAYER_IPC_ADDR")
        .ok()
        .filter(|addr| {
            addr.parse::<std::net::SocketAddr>()
                .is_ok_and(|addr| addr.ip().is_loopback())
        })
        .unwrap_or_else(|| IPC_ADDR.into())
}

pub fn start_server() -> IpcInbox {
    let (tx, rx) = mpsc::channel();
    let wakeup = Arc::new(Mutex::new(None::<egui::Context>));
    let notify = wakeup.clone();
    std::thread::spawn(move || {
        let Ok(listener) = TcpListener::bind(ipc_addr()) else {
            return;
        };
        for incoming in listener.incoming() {
            let Ok(mut stream) = incoming else {
                continue;
            };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let mut raw = String::new();
            if stream.read_to_string(&mut raw).is_err() {
                continue;
            }
            let Ok(message) = toml::from_str::<IpcMessage>(&raw) else {
                continue;
            };
            if tx.send(message).is_err() {
                break;
            }
            if let Some(ctx) = notify.lock().expect("IPC wakeup").as_ref() {
                ctx.request_repaint();
            }
        }
    });
    IpcInbox {
        receiver: rx,
        wakeup,
    }
}
