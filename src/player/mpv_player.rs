use std::cmp::Ordering;
use std::collections::HashMap;
use std::ffi::CString;
use std::fs;
use std::os::raw::{c_char, c_double, c_int, c_void};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{anyhow, Result};
use libloading::Library;
use once_cell::sync::Lazy;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DefWindowProcW, GWLP_WNDPROC, GetWindowLongPtrW, SetWindowLongPtrW,
    WM_CLOSE,
};

use crate::domain::playback::PlaybackState;
use crate::domain::video::VideoItem;

const MPV_FORMAT_FLAG: c_int = 3;
const MPV_FORMAT_DOUBLE: c_int = 5;

type WindowProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

#[repr(C)]
struct mpv_handle {
    _private: [u8; 0],
}

type MpvCreate = unsafe extern "C" fn() -> *mut mpv_handle;
type MpvInitialize = unsafe extern "C" fn(*mut mpv_handle) -> c_int;
type MpvTerminateDestroy = unsafe extern "C" fn(*mut mpv_handle);
type MpvCommandString = unsafe extern "C" fn(*mut mpv_handle, *const c_char) -> c_int;
type MpvSetOptionString = unsafe extern "C" fn(*mut mpv_handle, *const c_char, *const c_char) -> c_int;
type MpvSetProperty = unsafe extern "C" fn(*mut mpv_handle, *const c_char, c_int, *mut c_void) -> c_int;
type MpvGetProperty = unsafe extern "C" fn(*mut mpv_handle, *const c_char, c_int, *mut c_void) -> c_int;
type MpvErrorString = unsafe extern "C" fn(c_int) -> *const c_char;

#[link(name = "user32")]
unsafe extern "system" {
    fn FindWindowW(class_name: *const u16, window_name: *const u16) -> *mut c_void;
    fn MoveWindow(hwnd: *mut c_void, x: c_int, y: c_int, width: c_int, height: c_int, repaint: i32) -> i32;
    fn ShowWindow(hwnd: *mut c_void, cmd_show: c_int) -> i32;
    fn GetSystemMetrics(index: c_int) -> c_int;
    fn SetForegroundWindow(hwnd: *mut c_void) -> i32;
    fn BringWindowToTop(hwnd: *mut c_void) -> i32;
    fn SetWindowPos(hwnd: *mut c_void, hwnd_insert_after: *mut c_void, x: c_int, y: c_int, cx: c_int, cy: c_int, u_flags: u32) -> i32;
    fn SetFocus(hwnd: *mut c_void) -> *mut c_void;
    fn SetActiveWindow(hwnd: *mut c_void) -> *mut c_void;
    fn GetForegroundWindow() -> *mut c_void;
    fn GetWindowThreadProcessId(hwnd: *mut c_void, process_id: *mut u32) -> u32;
    fn AttachThreadInput(id_attach: u32, id_attach_to: u32, attach: i32) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentThreadId() -> u32;
}

const SW_RESTORE: c_int = 9;
const SM_CXSCREEN: c_int = 0;
const SM_CYSCREEN: c_int = 1;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_SHOWWINDOW: u32 = 0x0040;
const HWND_TOPMOST: *mut c_void = -1isize as *mut c_void;
const HWND_NOTOPMOST: *mut c_void = -2isize as *mut c_void;

#[derive(Clone, Copy)]
struct WindowHook {
    player_id: usize,
    original_proc: isize,
}

static WINDOW_HOOKS: Lazy<Mutex<HashMap<isize, WindowHook>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static CLOSED_PLAYER_IDS: Lazy<Mutex<Vec<usize>>> = Lazy::new(|| Mutex::new(Vec::new()));

pub struct BackendTick {
    pub anchor_position: Option<f64>,
    pub playback_state: Option<PlaybackState>,
    pub selected_volume: Option<f64>,
    pub selected_fullscreen: Option<bool>,
    pub selected_render_status: Option<String>,
    pub closed_player_ids: Vec<usize>,
}

#[derive(Clone)]
pub struct EmbeddedPlayerHandle;

impl EmbeddedPlayerHandle {
    pub fn paint_callback(&self, rect: egui::Rect) -> egui::PaintCallback {
        egui::PaintCallback {
            rect,
            callback: std::sync::Arc::new(egui_glow::CallbackFn::new(|_, _| {})),
        }
    }
}

pub trait PlayerBackend {
    fn replace_playlist(&mut self, videos: &mut [VideoItem], master_audio_index: Option<usize>, speed: f32) -> Result<()>;
    fn play(&mut self, speed: f32) -> Result<()>;
    fn pause(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn seek(&mut self, position_sec: f64) -> Result<()>;
    fn set_speed(&mut self, speed: f32) -> Result<()>;
    fn set_master_audio(&mut self, index: Option<usize>) -> Result<()>;
    fn capture_frame(&mut self, index: usize, output_path: &Path) -> Result<()>;
    fn close_player(&mut self, index: usize) -> Result<()>;
    fn remove_players(&mut self, ids: &[usize]);
    fn tick(&mut self, videos: &mut [VideoItem], playback_state: PlaybackState, selected_index: Option<usize>, sync_enabled: bool, loop_playback: bool) -> BackendTick;
    fn render_handle(&self, index: usize) -> Option<EmbeddedPlayerHandle>;
    fn backend_name(&self) -> &'static str;
}

#[derive(Default)]
pub struct LibMpvBackend {
    players: Vec<WindowMpv>,
}

impl PlayerBackend for LibMpvBackend {
    fn replace_playlist(&mut self, videos: &mut [VideoItem], master_audio_index: Option<usize>, speed: f32) -> Result<()> {
        self.players.clear();
        for video in videos.iter_mut() {
            let title = format!("naVPlayer-mpv-{}", video.id);
            let player = WindowMpv::new(title, video.id)?;
            player.load_video(&video.path)?;
            player.set_property_f64("speed", f64::from(speed))?;
            player.set_property_bool("pause", true)?;
            player.set_property_bool("mute", Some(video.id) != master_audio_index)?;
            if let Ok(duration) = player.get_property_f64("duration") {
                if duration.is_finite() && duration > 0.0 {
                    video.duration_sec = duration;
                }
            }
            video.position_sec = 0.0;
            video.muted = Some(video.id) != master_audio_index;
            self.players.push(player);
        }
        self.arrange_windows();
        Ok(())
    }

    fn play(&mut self, speed: f32) -> Result<()> {
        self.set_speed(speed)?;
        for player in &self.players {
            player.set_property_bool("pause", false)?;
        }
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        for player in &self.players {
            player.set_property_bool("pause", true)?;
        }
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        for player in &self.players {
            player.set_property_bool("pause", true)?;
            player.set_property_f64("time-pos", 0.0)?;
        }
        Ok(())
    }

    fn seek(&mut self, position_sec: f64) -> Result<()> {
        for player in &self.players {
            player.set_property_f64("time-pos", position_sec)?;
        }
        Ok(())
    }

    fn set_speed(&mut self, speed: f32) -> Result<()> {
        for player in &self.players {
            player.set_property_f64("speed", f64::from(speed))?;
        }
        Ok(())
    }

    fn set_master_audio(&mut self, index: Option<usize>) -> Result<()> {
        for player in &self.players {
            player.set_property_bool("mute", Some(player.id) != index)?;
        }
        Ok(())
    }

    fn capture_frame(&mut self, index: usize, output_path: &Path) -> Result<()> {
        if let Some(player) = self.players.iter().find(|player| player.id == index) {
            return player.capture_frame(output_path);
        }
        Err(anyhow!("player {index} not found"))
    }

    fn close_player(&mut self, index: usize) -> Result<()> {
        if let Some(player) = self.players.iter().find(|player| player.id == index) {
            let _ = player.command_string("quit 0");
        }
        Ok(())
    }

    fn remove_players(&mut self, ids: &[usize]) {
        if ids.is_empty() {
            return;
        }
        self.players.retain(|player| !ids.contains(&player.id));
        for (index, player) in self.players.iter_mut().enumerate() {
            player.id = index;
            player.refresh_window_hook();
        }
        self.arrange_windows();
    }

    fn tick(&mut self, videos: &mut [VideoItem], playback_state: PlaybackState, selected_index: Option<usize>, sync_enabled: bool, _loop_playback: bool) -> BackendTick {
        let anchor_player_id = selected_index
            .filter(|index| self.players.iter().any(|player| player.id == *index))
            .or_else(|| self.players.first().map(|player| player.id));
        let mut selected_volume = None;
        let mut external_state = playback_state;
        let mut anchor = None;
        let mut closed_player_ids = drain_closed_player_ids();
        let mut needs_initial_arrange = false;

        for player in &mut self.players {
            let was_seen = player.seen_window;
            if player.detect_closed() {
                if !closed_player_ids.contains(&player.id) {
                    closed_player_ids.push(player.id);
                }
                continue;
            }
            if player.seen_window && !was_seen {
                needs_initial_arrange = true;
            }
            if let Some(video) = videos.get_mut(player.id) {
                if let Ok(duration) = player.get_property_f64("duration") {
                    if duration.is_finite() && duration > 0.0 {
                        video.duration_sec = duration;
                    }
                }
                if let Ok(position) = player.get_property_f64("time-pos") {
                    if position.is_finite() && position >= 0.0 {
                        video.position_sec = position;
                    }
                }
                if anchor_player_id == Some(player.id) {
                    anchor = Some(video.position_sec);
                    selected_volume = player.get_property_f64("volume").ok();
                    if let Ok(paused) = player.get_property_bool("pause") {
                        external_state = if paused {
                            PlaybackState::Paused
                        } else {
                            PlaybackState::Playing
                        };
                    }
                }
            }
        }

        if needs_initial_arrange {
            self.arrange_windows();
        }

        if external_state != playback_state {
            for player in &self.players {
                let _ = player.set_property_bool("pause", external_state != PlaybackState::Playing);
            }
        }

        if sync_enabled {
            if let Some(anchor_pos) = anchor {
                for player in &self.players {
                    if anchor_player_id == Some(player.id) {
                        continue;
                    }
                    if let Some(video) = videos.get_mut(player.id) {
                        if (video.position_sec - anchor_pos).abs() > 0.10 {
                            let _ = player.set_property_f64("time-pos", anchor_pos);
                            video.position_sec = anchor_pos;
                        }
                    }
                }
            }
        }

        BackendTick {
            anchor_position: anchor,
            playback_state: Some(external_state),
            selected_volume,
            selected_fullscreen: Some(false),
            selected_render_status: Some("External mpv window mode".to_owned()),
            closed_player_ids,
        }
    }

    fn render_handle(&self, _index: usize) -> Option<EmbeddedPlayerHandle> {
        None
    }

    fn backend_name(&self) -> &'static str {
        "libmpv-window"
    }
}

impl LibMpvBackend {
    fn arrange_windows(&self) {
        let total = self.players.len().min(2);
        if total == 0 {
            return;
        }
        let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        if screen_w <= 0 || screen_h <= 0 {
            return;
        }

        match total {
            1 => {
                if let Some(player) = self.players.first() {
                    let width = ((screen_w as f32) * 0.72) as i32;
                    let height = ((screen_h as f32) * 0.82) as i32;
                    let x = (screen_w - width) / 2;
                    let y = ((screen_h - height) / 2).max(0);
                    player.move_window(x, y, width, height);
                }
            }
            _ => {
                let width = screen_w / 2;
                let height = ((screen_h as f32) * 0.82) as i32;
                let y = ((screen_h - height) / 2).max(0);
                for (index, player) in self.players.iter().take(2).enumerate() {
                    let x = (index as i32) * width;
                    player.move_window(x, y, width, height);
                }
            }
        }
    }
}

struct WindowMpv {
    lib: Library,
    handle: *mut mpv_handle,
    id: usize,
    title: String,
    input_conf_path: PathBuf,
    script_path: PathBuf,
    seen_window: bool,
    hooked_hwnd: Option<isize>,
}

unsafe impl Send for WindowMpv {}
unsafe impl Sync for WindowMpv {}

impl WindowMpv {
    fn new(title: String, id: usize) -> Result<Self> {
        let lib = load_mpv_library()?;
        let handle = unsafe {
            let create: libloading::Symbol<'_, MpvCreate> = lib.get(b"mpv_create\0")?;
            create()
        };
        if handle.is_null() {
            return Err(anyhow!("mpv_create returned null"));
        }

        let input_conf_path = write_input_conf(&title)?;
        let script_path = write_seek_guard_script(&title)?;
        let player = Self {
            lib,
            handle,
            id,
            title,
            input_conf_path,
            script_path,
            seen_window: false,
            hooked_hwnd: None,
        };
        player.set_option_string("title", &player.title)?;
        player.set_option_string("force-window", "yes")?;
        player.set_option_string("keep-open", "no")?;
        player.set_option_string("idle", "no")?;
        player.set_option_string("input-default-bindings", "no")?;
        player.set_option_string("native-keyrepeat", "yes")?;
        player.set_option_string("input-ar-delay", "80")?;
        player.set_option_string("input-ar-rate", "120")?;
        player.set_option_string("input-vo-keyboard", "yes")?;
        player.set_option_string("input-conf", &player.input_conf_path.to_string_lossy())?;
        player.set_option_string("osc", "yes")?;
        player.set_option_string("hwdec", "auto-safe")?;
        player.set_option_string("player-operation-mode", "cplayer")?;
        player.initialize()?;
        player.load_support_script()?;
        Ok(player)
    }

    fn initialize(&self) -> Result<()> {
        let status = unsafe {
            let init: libloading::Symbol<'_, MpvInitialize> = self.lib.get(b"mpv_initialize\0")?;
            init(self.handle)
        };
        self.check_status(status, "mpv_initialize")
    }

    fn load_support_script(&self) -> Result<()> {
        let raw_path = self.script_path.to_string_lossy();
        let quoted = quote_for_mpv_command(&raw_path);
        let command = format!("load-script {quoted}");
        self.command_string(&command)
            .map_err(|err| anyhow!("failed to load support script {}: {err:#}", self.script_path.display()))
    }

    fn load_video(&self, path: &Path) -> Result<()> {
        let playlist = sibling_video_files(path);
        let current_index = playlist
            .iter()
            .position(|candidate| candidate == path)
            .unwrap_or(0);

        for (index, entry) in playlist.iter().enumerate() {
            let raw_path = entry.to_string_lossy();
            let quoted = quote_for_mpv_command(&raw_path);
            let command = if index == 0 {
                format!("loadfile {quoted} replace")
            } else {
                format!("loadfile {quoted} append")
            };
            self.command_string(&command)
                .map_err(|err| anyhow!("failed to queue {}: {err:#}", entry.display()))?;
        }

        if current_index > 0 {
            self.command_string(&format!("playlist-play-index {current_index}"))
                .map_err(|err| anyhow!("failed to select playlist index {current_index}: {err:#}"))?;
        }

        Ok(())
    }

    fn detect_closed(&mut self) -> bool {
        let exists = self.find_window().is_some();
        if exists {
            self.seen_window = true;
            self.refresh_window_hook();
            return false;
        }
        self.remove_window_hook();
        self.seen_window
    }

    fn move_window(&self, x: i32, y: i32, width: i32, height: i32) {
        if let Some(hwnd) = self.find_window() {
            unsafe {
                ShowWindow(hwnd, SW_RESTORE);
                MoveWindow(hwnd, x, y, width, height, 1);
                SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
                SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
                focus_window(hwnd);
            }
        }
    }

    fn find_window(&self) -> Option<*mut c_void> {
        let wide: Vec<u16> = self.title.encode_utf16().chain(std::iter::once(0)).collect();
        let hwnd = unsafe { FindWindowW(std::ptr::null(), wide.as_ptr()) };
        if hwnd.is_null() { None } else { Some(hwnd) }
    }

    fn refresh_window_hook(&mut self) {
        let Some(hwnd_ptr) = self.find_window() else {
            return;
        };
        let hwnd = hwnd_ptr as isize;
        let mut hooks = WINDOW_HOOKS.lock().expect("window hooks mutex poisoned");

        if self.hooked_hwnd != Some(hwnd) {
            drop(hooks);
            self.remove_window_hook();
            hooks = WINDOW_HOOKS.lock().expect("window hooks mutex poisoned");
        }

        if let Some(existing) = hooks.get_mut(&hwnd) {
            existing.player_id = self.id;
            self.hooked_hwnd = Some(hwnd);
            return;
        }

        let original_proc = unsafe { GetWindowLongPtrW(hwnd_ptr as HWND, GWLP_WNDPROC) };
        if original_proc == 0 {
            return;
        }
        unsafe {
            SetWindowLongPtrW(hwnd_ptr as HWND, GWLP_WNDPROC, hooked_window_proc as *const () as usize as isize);
        }
        hooks.insert(
            hwnd,
            WindowHook {
                player_id: self.id,
                original_proc,
            },
        );
        self.hooked_hwnd = Some(hwnd);
    }

    fn remove_window_hook(&mut self) {
        let Some(hwnd) = self.hooked_hwnd.take() else {
            return;
        };
        let mut hooks = WINDOW_HOOKS.lock().expect("window hooks mutex poisoned");
        if let Some(hook) = hooks.remove(&hwnd) {
            unsafe {
                SetWindowLongPtrW(hwnd as HWND, GWLP_WNDPROC, hook.original_proc);
            }
        }
    }

    fn capture_frame(&self, output_path: &Path) -> Result<()> {
        let raw_path = output_path.to_string_lossy();
        let quoted = quote_for_mpv_command(&raw_path);
        let command = format!("screenshot-to-file {quoted} video");
        self.command_string(&command)
    }

    fn command_string(&self, command: &str) -> Result<()> {
        let command = CString::new(command)?;
        let status = unsafe {
            let func: libloading::Symbol<'_, MpvCommandString> = self.lib.get(b"mpv_command_string\0")?;
            func(self.handle, command.as_ptr())
        };
        self.check_status(status, "mpv_command_string")
    }

    fn set_option_string(&self, name: &str, value: &str) -> Result<()> {
        let option_name = name.to_owned();
        let name = CString::new(name)?;
        let value = CString::new(value)?;
        let status = unsafe {
            let func: libloading::Symbol<'_, MpvSetOptionString> = self.lib.get(b"mpv_set_option_string\0")?;
            func(self.handle, name.as_ptr(), value.as_ptr())
        };
        self.check_status(status, &format!("mpv_set_option_string({option_name})"))
    }

    fn set_property_bool(&self, name: &str, value: bool) -> Result<()> {
        let name = CString::new(name)?;
        let mut raw = if value { 1 } else { 0 };
        let status = unsafe {
            let func: libloading::Symbol<'_, MpvSetProperty> = self.lib.get(b"mpv_set_property\0")?;
            func(self.handle, name.as_ptr(), MPV_FORMAT_FLAG, (&mut raw as *mut c_int).cast::<c_void>())
        };
        self.check_status(status, "mpv_set_property(flag)")
    }

    fn set_property_f64(&self, name: &str, value: f64) -> Result<()> {
        let name = CString::new(name)?;
        let mut raw = value as c_double;
        let status = unsafe {
            let func: libloading::Symbol<'_, MpvSetProperty> = self.lib.get(b"mpv_set_property\0")?;
            func(self.handle, name.as_ptr(), MPV_FORMAT_DOUBLE, (&mut raw as *mut c_double).cast::<c_void>())
        };
        self.check_status(status, "mpv_set_property(double)")
    }

    fn get_property_bool(&self, name: &str) -> Result<bool> {
        let name = CString::new(name)?;
        let mut raw: c_int = 0;
        let status = unsafe {
            let func: libloading::Symbol<'_, MpvGetProperty> = self.lib.get(b"mpv_get_property\0")?;
            func(self.handle, name.as_ptr(), MPV_FORMAT_FLAG, (&mut raw as *mut c_int).cast::<c_void>())
        };
        self.check_status(status, "mpv_get_property(flag)")?;
        Ok(raw != 0)
    }

    fn get_property_f64(&self, name: &str) -> Result<f64> {
        let name = CString::new(name)?;
        let mut raw: c_double = 0.0;
        let status = unsafe {
            let func: libloading::Symbol<'_, MpvGetProperty> = self.lib.get(b"mpv_get_property\0")?;
            func(self.handle, name.as_ptr(), MPV_FORMAT_DOUBLE, (&mut raw as *mut c_double).cast::<c_void>())
        };
        self.check_status(status, "mpv_get_property(double)")?;
        Ok(raw)
    }

    fn check_status(&self, status: c_int, context: &str) -> Result<()> {
        if status >= 0 {
            return Ok(());
        }
        Err(anyhow!("{context}: {}", self.error_text(status)))
    }

    fn error_text(&self, status: c_int) -> String {
        unsafe {
            let func: libloading::Symbol<'_, MpvErrorString> = match self.lib.get(b"mpv_error_string\0") {
                Ok(func) => func,
                Err(_) => return format!("mpv error {status}"),
            };
            let ptr = func(status);
            if ptr.is_null() {
                format!("mpv error {status}")
            } else {
                std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        }
    }
}

impl Drop for WindowMpv {
    fn drop(&mut self) {
        self.remove_window_hook();
        unsafe {
            if !self.handle.is_null() {
                if let Ok(func) = self.lib.get::<MpvTerminateDestroy>(b"mpv_terminate_destroy\0") {
                    func(self.handle);
                }
            }
        }
        let _ = fs::remove_file(&self.input_conf_path);
        let _ = fs::remove_file(&self.script_path);
    }
}

fn load_mpv_library() -> Result<Library> {
    let mut candidates = Vec::new();
    if let Ok(path) = std::env::var("MPV_DLL_PATH") {
        candidates.push(path);
    }
    candidates.push("mpv-2.dll".to_owned());
    candidates.push("libmpv-2.dll".to_owned());
    candidates.push("mpv-1.dll".to_owned());
    candidates.push("mpv.dll".to_owned());

    let mut errors = Vec::new();
    for candidate in candidates {
        match unsafe { Library::new(&candidate) } {
            Ok(lib) => return Ok(lib),
            Err(err) => errors.push(format!("{candidate}: {err}")),
        }
    }

    Err(anyhow!(
        "unable to load libmpv. Put mpv-2.dll next to the executable or set MPV_DLL_PATH. Tried: {}",
        errors.join(" | ")
    ))
}

fn write_input_conf(title: &str) -> Result<PathBuf> {
    let mut path = std::env::temp_dir();
    path.push(format!("{title}.input.conf"));
    let content = [
        "UP add volume 5",
        "DOWN add volume -5",
        "RIGHT script-message navplayer-safe-seek 10 3",
        "RIGHT repeatable script-message navplayer-safe-seek 10 3",
        "LEFT script-message navplayer-safe-seek -10 3",
        "LEFT repeatable script-message navplayer-safe-seek -10 3",
        "n script-message navplayer-playlist-next",
        "p script-message navplayer-playlist-prev",
        "SPACE cycle pause",
        "MBTN_LEFT cycle pause",
        "f cycle fullscreen",
        "Shift+f set fullscreen no",
        "c script-message navplayer-capture-thumbnail",
        "q quit 0",
    ]
    .join("\n");
    fs::write(&path, content)?;
    Ok(path)
}

fn write_seek_guard_script(title: &str) -> Result<PathBuf> {
    let mut path = std::env::temp_dir();
    path.push(format!("{title}.seek_guard.lua"));
    let content = r#"
local mp = require 'mp'
local utils = require 'mp.utils'

local function sanitize_filename(name)
    return (name:gsub('[<>:"/\\|%?%*]', '_'))
end

local function split_dir_and_stem(path)
    local normalized = path:gsub('/', '\\')
    local dir = normalized:match('^(.*)\\[^\\]+$') or '.'
    local file = normalized:match('([^\\]+)$') or 'thumbnail'
    local stem = file:gsub('%.%w+$', '')
    return dir, stem
end

local function format_timestamp(pos)
    local millis = math.floor((math.max(pos, 0) * 1000) + 0.5)
    local minutes = math.floor(millis / 60000)
    local seconds = math.floor((millis % 60000) / 1000)
    local ms = millis % 1000
    return string.format('%02dm%02ds%03dms', minutes, seconds, ms)
end

mp.register_script_message('navplayer-safe-seek', function(step_text, guard_text)
    local step = tonumber(step_text) or 10
    local guard = tonumber(guard_text) or 3
    local pos = mp.get_property_number('time-pos', 0)
    local duration = mp.get_property_number('duration', 0)
    local epsilon = 0.2

    if step > 0 then
        if duration <= 0 then
            mp.commandv('seek', tostring(step), 'relative', 'exact')
            return
        end

        local max_target = math.max(0, duration - guard - epsilon)
        if pos >= max_target then
            return
        end

        local target = math.min(pos + step, max_target)
        if target <= pos + 0.01 then
            return
        end

        mp.commandv('seek', tostring(target), 'absolute', 'exact')
        mp.osd_message('>>', 0.35)
        return
    end

    local min_target = guard + epsilon
    if pos <= min_target then
        return
    end

    local target = math.max(pos + step, min_target)
    if target >= pos - 0.01 then
        return
    end

    mp.commandv('seek', tostring(target), 'absolute', 'exact')
    mp.osd_message('<<', 0.35)
end)

mp.register_script_message('navplayer-capture-thumbnail', function()
    local video_path = mp.get_property('path')
    if not video_path or video_path == '' then
        return
    end

    local pos = mp.get_property_number('time-pos', 0)
    local dir, stem = split_dir_and_stem(video_path)
    local output_dir = utils.join_path(dir, 'thumbnails')
    utils.subprocess({ args = { 'cmd', '/c', 'mkdir', output_dir }, cancellable = false })

    local filename = string.format('%s_%s.jpg', sanitize_filename(stem), format_timestamp(pos))
    local output_path = utils.join_path(output_dir, filename)
    mp.commandv('screenshot-to-file', output_path, 'video')
    mp.osd_message('Saved thumbnail: ' .. output_path, 2.0)
end)

local function show_current_file()
    local title = mp.get_property('media-title')
    if title and title ~= '' then
        mp.osd_message('Now playing: ' .. title, 1.5)
    end
end

mp.register_script_message('navplayer-playlist-next', function()
    mp.commandv('playlist-next', 'force')
    mp.add_timeout(0.05, show_current_file)
end)

mp.register_script_message('navplayer-playlist-prev', function()
    mp.commandv('playlist-prev', 'force')
    mp.add_timeout(0.05, show_current_file)
end)
"#;
    fs::write(&path, content.trim_start())?;
    Ok(path)
}


fn sibling_video_files(path: &Path) -> Vec<PathBuf> {
    let Some(parent) = path.parent() else {
        return vec![path.to_path_buf()];
    };

    let Ok(entries) = fs::read_dir(parent) else {
        return vec![path.to_path_buf()];
    };

    let mut files: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|entry| entry.is_file())
        .filter(|entry| is_supported_video_file(entry))
        .collect();

    files.sort_by(|left, right| compare_file_names(left, right));

    if files.iter().any(|entry| entry == path) {
        files
    } else {
        vec![path.to_path_buf()]
    }
}

fn is_supported_video_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "mp4" | "mov"))
        .unwrap_or(false)
}

fn compare_file_names(left: &Path, right: &Path) -> Ordering {
    let left_name = left
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let right_name = right
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    left_name.cmp(&right_name)
}
fn quote_for_mpv_command(input: &str) -> String {
    let escaped = input.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn drain_closed_player_ids() -> Vec<usize> {
    let mut ids = CLOSED_PLAYER_IDS
        .lock()
        .expect("closed player ids mutex poisoned");
    ids.sort_unstable();
    ids.dedup();
    std::mem::take(&mut *ids)
}

fn focus_window(hwnd: *mut c_void) {
    unsafe {
        let foreground = GetForegroundWindow();
        let current_thread = GetCurrentThreadId();
        let target_thread = GetWindowThreadProcessId(hwnd, std::ptr::null_mut());
        let foreground_thread = if foreground.is_null() {
            0
        } else {
            GetWindowThreadProcessId(foreground, std::ptr::null_mut())
        };

        if foreground_thread != 0 && foreground_thread != current_thread {
            AttachThreadInput(current_thread, foreground_thread, 1);
        }
        if target_thread != 0 && target_thread != current_thread {
            AttachThreadInput(current_thread, target_thread, 1);
        }

        BringWindowToTop(hwnd);
        SetForegroundWindow(hwnd);
        SetActiveWindow(hwnd);
        SetFocus(hwnd);

        if target_thread != 0 && target_thread != current_thread {
            AttachThreadInput(current_thread, target_thread, 0);
        }
        if foreground_thread != 0 && foreground_thread != current_thread {
            AttachThreadInput(current_thread, foreground_thread, 0);
        }
    }
}

unsafe extern "system" fn hooked_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let key = hwnd as isize;
    let hook = {
        let hooks = WINDOW_HOOKS.lock().expect("window hooks mutex poisoned");
        hooks.get(&key).copied()
    };

    if msg == WM_CLOSE {
        if let Some(hook) = hook {
            let mut closed = CLOSED_PLAYER_IDS
                .lock()
                .expect("closed player ids mutex poisoned");
            if !closed.contains(&hook.player_id) {
                closed.push(hook.player_id);
            }
        }
        return 0;
    }

    if let Some(hook) = hook {
        let original_proc: WindowProc = unsafe { std::mem::transmute(hook.original_proc) };
        unsafe { CallWindowProcW(Some(original_proc), hwnd, msg, wparam, lparam) }
    } else {
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }
}


