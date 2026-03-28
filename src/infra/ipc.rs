use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use serde::{Deserialize, Serialize};

const IPC_ADDR: &str = "127.0.0.1:43871";

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
    let Ok(addr) = IPC_ADDR.parse() else {
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

pub fn start_server() -> Receiver<IpcMessage> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let Ok(listener) = TcpListener::bind(IPC_ADDR) else {
            return;
        };
        for incoming in listener.incoming() {
            let Ok(mut stream) = incoming else {
                continue;
            };
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
        }
    });
    rx
}
