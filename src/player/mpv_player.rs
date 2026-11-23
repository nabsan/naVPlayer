use std::cell::Cell;
use std::collections::HashSet;
use std::ffi::CString;
use std::fs;
use std::os::raw::{c_char, c_double, c_int, c_void};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use libloading::Library;

use crate::domain::playback::PlaybackState;
use crate::domain::video::VideoItem;

const MPV_FORMAT_FLAG: c_int = 3;
const MPV_FORMAT_DOUBLE: c_int = 5;

// Native identity stays private even when two windows display the same file.
static CLAIMED_WINDOWS: OnceLock<Mutex<HashSet<isize>>> = OnceLock::new();

#[repr(C)]
struct mpv_handle {
    _private: [u8; 0],
}

#[repr(C)]
struct MpvEvent {
    event_id: c_int,
    error: c_int,
    reply_userdata: u64,
    data: *mut c_void,
}
#[repr(C)]
struct MpvEndFile {
    reason: c_int,
    error: c_int,
}
#[repr(C)]
struct MpvClientMessage {
    num_args: c_int,
    args: *const *const c_char,
}

#[derive(Clone)]
struct ImageAction {
    command: String,
    path: PathBuf,
}

struct DeleteTicket {
    path: PathBuf,
    len: u64,
    modified: std::time::SystemTime,
    created: std::time::SystemTime,
}
impl DeleteTicket {
    fn capture(path: &Path) -> Result<Self> {
        let path = std::path::absolute(path)?;
        let metadata = fs::symlink_metadata(&path)?;
        anyhow::ensure!(
            metadata.file_type().is_file() && crate::domain::media::is_image_path(&path),
            "Only regular still images can be deleted"
        );
        Ok(Self {
            path,
            len: metadata.len(),
            modified: metadata.modified()?,
            created: metadata.created()?,
        })
    }
    fn unchanged(&self) -> Result<bool> {
        let now = Self::capture(&self.path)?;
        Ok(self.len == now.len && self.modified == now.modified && self.created == now.created)
    }
}
type MpvWaitEvent = unsafe extern "C" fn(*mut mpv_handle, c_double) -> *const MpvEvent;

#[derive(Default)]
struct PlayerEvents {
    closed: bool,
    file_loaded: bool,
    error: Option<String>,
    image_actions: Vec<ImageAction>,
}

type MpvCreate = unsafe extern "C" fn() -> *mut mpv_handle;
type MpvInitialize = unsafe extern "C" fn(*mut mpv_handle) -> c_int;
type MpvTerminateDestroy = unsafe extern "C" fn(*mut mpv_handle);
type MpvCommandString = unsafe extern "C" fn(*mut mpv_handle, *const c_char) -> c_int;
type MpvSetOptionString =
    unsafe extern "C" fn(*mut mpv_handle, *const c_char, *const c_char) -> c_int;
type MpvSetProperty =
    unsafe extern "C" fn(*mut mpv_handle, *const c_char, c_int, *mut c_void) -> c_int;
type MpvGetProperty =
    unsafe extern "C" fn(*mut mpv_handle, *const c_char, c_int, *mut c_void) -> c_int;
type MpvErrorString = unsafe extern "C" fn(c_int) -> *const c_char;

#[link(name = "user32")]
unsafe extern "system" {
    fn IsWindow(hwnd: *mut c_void) -> i32;
    fn FindWindowExW(
        parent: *mut c_void,
        after: *mut c_void,
        class_name: *const u16,
        window_name: *const u16,
    ) -> *mut c_void;
    fn MoveWindow(
        hwnd: *mut c_void,
        x: c_int,
        y: c_int,
        width: c_int,
        height: c_int,
        repaint: i32,
    ) -> i32;
    fn ShowWindow(hwnd: *mut c_void, cmd_show: c_int) -> i32;
    fn GetSystemMetrics(index: c_int) -> c_int;
    fn SetForegroundWindow(hwnd: *mut c_void) -> i32;
    fn BringWindowToTop(hwnd: *mut c_void) -> i32;
    fn SetWindowPos(
        hwnd: *mut c_void,
        hwnd_insert_after: *mut c_void,
        x: c_int,
        y: c_int,
        cx: c_int,
        cy: c_int,
        u_flags: u32,
    ) -> i32;
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

pub struct BackendTick {
    pub anchor_position: Option<f64>,
    pub playback_state: Option<PlaybackState>,
    pub selected_volume: Option<f64>,
    pub selected_fullscreen: Option<bool>,
    pub selected_render_status: Option<String>,
    pub closed_player_ids: Vec<usize>,
    pub failed: bool,
}

pub trait PlayerBackend {
    fn replace_playlist(
        &mut self,
        videos: &mut [VideoItem],
        master_audio_index: Option<usize>,
        speed: f32,
    ) -> Result<()>;
    fn play(&mut self, speed: f32) -> Result<()>;
    fn pause(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn seek(&mut self, position_sec: f64) -> Result<()>;
    fn set_speed(&mut self, speed: f32) -> Result<()>;
    fn set_master_audio(&mut self, index: Option<usize>) -> Result<()>;
    fn capture_frame(&mut self, index: usize, output_path: &Path) -> Result<()>;
    fn close_player(&mut self, index: usize) -> Result<()>;
    fn remove_players(&mut self, ids: &[usize]);
    fn tick(
        &mut self,
        videos: &mut [VideoItem],
        playback_state: PlaybackState,
        selected_index: Option<usize>,
        sync_enabled: bool,
        loop_playback: bool,
    ) -> BackendTick;
    fn backend_name(&self) -> &'static str;
}

#[derive(Default)]
pub struct LibMpvBackend {
    players: Vec<WindowMpv>,
    last_sync: Option<Instant>,
    loop_playback: Option<bool>,
}

impl PlayerBackend for LibMpvBackend {
    fn replace_playlist(
        &mut self,
        videos: &mut [VideoItem],
        master_audio_index: Option<usize>,
        speed: f32,
    ) -> Result<()> {
        self.players.clear();
        let mut players = Vec::new();
        self.last_sync = None;
        self.loop_playback = None;
        for video in videos.iter_mut() {
            let title = format!("naVPlayer-mpv-{}-{}", std::process::id(), video.id);
            let player = WindowMpv::new(title, video.id)?;
            player.load_video(&video.path)?;
            player.set_property_f64("speed", f64::from(speed))?;
            player.set_property_bool("pause", true)?;
            player.set_property_bool("mute", Some(video.id) != master_audio_index)?;
            video.position_sec = 0.0;
            video.muted = Some(video.id) != master_audio_index;
            players.push(player);
        }
        self.players = players;
        self.arrange_windows();
        Ok(())
    }

    fn play(&mut self, speed: f32) -> Result<()> {
        self.set_speed(speed)?;
        for player in self.players.iter().filter(|player| !player.is_image.get()) {
            player.set_property_bool("pause", false)?;
        }
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        for player in self.players.iter().filter(|player| !player.is_image.get()) {
            player.set_property_bool("pause", true)?;
        }
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        for player in self.players.iter().filter(|player| !player.is_image.get()) {
            player.set_property_bool("pause", true)?;
            player.set_property_f64("time-pos", 0.0)?;
        }
        Ok(())
    }

    fn seek(&mut self, position_sec: f64) -> Result<()> {
        for player in self.players.iter().filter(|player| !player.is_image.get()) {
            player.set_property_f64("time-pos", position_sec)?;
        }
        Ok(())
    }

    fn set_speed(&mut self, speed: f32) -> Result<()> {
        for player in self.players.iter().filter(|player| !player.is_image.get()) {
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
        }
        self.arrange_windows();
    }

    fn tick(
        &mut self,
        videos: &mut [VideoItem],
        playback_state: PlaybackState,
        selected_index: Option<usize>,
        sync_enabled: bool,
        loop_playback: bool,
    ) -> BackendTick {
        let anchor_player_id = selected_index
            .filter(|index| {
                self.players
                    .iter()
                    .any(|player| player.id == *index && !player.is_image.get())
            })
            .or_else(|| {
                self.players
                    .iter()
                    .find(|player| !player.is_image.get())
                    .map(|player| player.id)
            });
        let mut selected_volume = None;
        let mut external_state = if anchor_player_id.is_none() {
            PlaybackState::Paused
        } else {
            playback_state
        };
        let mut anchor = None;
        let mut closed_player_ids = Vec::new();
        let mut failed = false;
        let mut status = None;
        if self.loop_playback != Some(loop_playback) {
            for player in &self.players {
                let _ = player.command_string(if loop_playback {
                    "set loop-file inf"
                } else {
                    "set loop-file no"
                });
            }
            self.loop_playback = Some(loop_playback);
        }
        let mut needs_initial_arrange = false;

        for player in &mut self.players {
            let was_seen = player.seen_window;
            let events = player.drain_events();
            for action in events.image_actions {
                if let Err(error) = player.image_action(action) {
                    let message = format!("File operation failed: {error:#}");
                    player.image_result(false, &message);
                    status = Some(message);
                }
            }
            if let Some(error) = events.error {
                failed = true;
                status = Some(error);
            }
            if events.closed {
                if !closed_player_ids.contains(&player.id) {
                    closed_player_ids.push(player.id);
                }
                continue;
            }
            player.discover_window();
            if player.seen_window && !was_seen {
                needs_initial_arrange = true;
            }
            if let Some(video) = videos.get_mut(player.id) {
                if events.file_loaded
                    && let Some(path) = player.get_property_string("path")
                {
                    video.path = PathBuf::from(path);
                    video.title = video
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                    player
                        .is_image
                        .set(crate::domain::media::is_image_path(&video.path));
                    video.duration_sec = 0.0;
                    video.position_sec = 0.0;
                }
                if player.is_image.get() {
                    // A still image has no timeline and cannot be a sync anchor.
                    continue;
                }
                if (events.file_loaded || video.duration_sec <= 0.0)
                    && let Ok(duration) = player.get_property_f64("duration")
                    && duration.is_finite()
                    && duration > 0.0
                {
                    video.duration_sec = duration;
                }
                if let Ok(position) = player.get_property_f64("time-pos")
                    && position.is_finite()
                    && position >= 0.0
                {
                    video.position_sec = position;
                }
                if anchor_player_id == Some(player.id) {
                    anchor = Some(video.position_sec);
                    selected_volume = player.get_property_f64("volume").ok();
                    if let Ok(paused) = player.get_property_bool("pause") {
                        external_state = if paused {
                            if playback_state == PlaybackState::Stopped {
                                PlaybackState::Stopped
                            } else {
                                PlaybackState::Paused
                            }
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
            for player in self.players.iter().filter(|player| !player.is_image.get()) {
                let _ = player.set_property_bool("pause", external_state != PlaybackState::Playing);
            }
        }

        if sync_enabled
            && self.players.len() > 1
            && self
                .last_sync
                .is_none_or(|last| last.elapsed() >= Duration::from_millis(500))
        {
            self.last_sync = Some(Instant::now());
            if let Some(anchor_pos) = anchor {
                for player in &self.players {
                    if anchor_player_id == Some(player.id) {
                        continue;
                    }
                    if let Some(video) = videos.get_mut(player.id)
                        && player.ready
                        && !player.is_image.get()
                        && (video.position_sec - anchor_pos).abs() > 0.15
                        && !player.get_property_bool("seeking").unwrap_or(true)
                    {
                        let _ = player.set_property_f64("time-pos", anchor_pos);
                        video.position_sec = anchor_pos;
                    }
                }
            }
        }

        BackendTick {
            anchor_position: anchor,
            playback_state: Some(external_state),
            selected_volume,
            selected_fullscreen: selected_index
                .and_then(|id| self.players.iter().find(|player| player.id == id))
                .and_then(|player| player.get_property_bool("fullscreen").ok()),
            selected_render_status: status,
            closed_player_ids,
            failed,
        }
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
    hwnd: Cell<Option<isize>>,
    ready: bool,
    is_image: Cell<bool>,
    delete_ticket: Option<DeleteTicket>,
    shutdown_received: bool,
}

impl WindowMpv {
    fn new(title: String, id: usize) -> Result<Self> {
        Self::with_options(title, id, &[])
    }

    fn with_options(title: String, id: usize, overrides: &[(&str, &str)]) -> Result<Self> {
        let lib = load_mpv_library()?;
        let handle = unsafe {
            let create: libloading::Symbol<'_, MpvCreate> = lib.get(b"mpv_create\0")?;
            create()
        };
        if handle.is_null() {
            return Err(anyhow!("mpv_create returned null"));
        }

        // Own the handle immediately: even a temp-file or option error must destroy it.
        let player = Self {
            lib,
            handle,
            id,
            title,
            input_conf_path: PathBuf::new(),
            script_path: PathBuf::new(),
            seen_window: false,
            hwnd: Cell::new(None),
            ready: false,
            is_image: Cell::new(false),
            delete_ticket: None,
            shutdown_received: false,
        };
        let mut player = player;
        player.input_conf_path = write_input_conf(&player.title)?;
        player.script_path = write_seek_guard_script(&player.title)?;
        player.set_option_string("config", "no")?;
        player.set_option_string(
            "title",
            &format!(
                "naVPlayer {} | ${{path:Loading...}}",
                crate::app::APP_VERSION
            ),
        )?;
        player.set_option_string("force-window", "yes")?;
        player.set_option_string("keep-open", "yes")?;
        player.set_option_string("image-display-duration", "inf")?;
        player.set_option_string("idle", "yes")?;
        player.set_option_string("input-default-bindings", "no")?;
        player.set_option_string("native-keyrepeat", "yes")?;
        player.set_option_string("input-ar-delay", "80")?;
        player.set_option_string("input-ar-rate", "120")?;
        player.set_option_string("input-vo-keyboard", "yes")?;
        player.set_option_string("input-conf", &player.input_conf_path.to_string_lossy())?;
        player.set_option_string("osc", "yes")?;
        player.set_option_string("hwdec", "auto-safe")?;
        player.set_option_string("player-operation-mode", "cplayer")?;
        for (name, value) in overrides {
            player.set_option_string(name, value)?;
        }
        player.initialize()?;
        player.load_support_script()?;
        Ok(player)
    }

    fn drain_events(&mut self) -> PlayerEvents {
        let mut events = PlayerEvents::default();
        if self.shutdown_received {
            events.closed = true;
            return events;
        }
        // Event data is borrowed until the next wait_event call; copy immediately.
        unsafe {
            let Ok(wait) = self.lib.get::<MpvWaitEvent>(b"mpv_wait_event\0") else {
                return events;
            };
            for _ in 0..256 {
                let event = &*wait(self.handle, 0.0);
                match event.event_id {
                    0 => break,
                    1 => {
                        events.closed = true;
                        self.shutdown_received = true;
                        break;
                    } // MPV_EVENT_SHUTDOWN (X or q)
                    6 => {
                        self.ready = false;
                        self.delete_ticket = None;
                    } // START_FILE
                    8 => {
                        events.file_loaded = true;
                        self.ready = true;
                    }
                    7 if !event.data.is_null() => {
                        let end = &*event.data.cast::<MpvEndFile>();
                        if end.reason == 4 {
                            // MPV_END_FILE_REASON_ERROR
                            events.error =
                                Some(format!("Playback error: {}", self.error_text(end.error)));
                            events.closed = true;
                        }
                    }
                    16 if !event.data.is_null() => {
                        let message = &*event.data.cast::<MpvClientMessage>();
                        if message.num_args == 2 && !message.args.is_null() {
                            let args = std::slice::from_raw_parts(message.args, 2);
                            if args.iter().all(|arg| !arg.is_null()) {
                                let command = std::ffi::CStr::from_ptr(args[0]).to_string_lossy();
                                if matches!(
                                    command.as_ref(),
                                    "navplayer-image-next"
                                        | "navplayer-reveal-file"
                                        | "navplayer-image-prev"
                                        | "navplayer-delete-prepare"
                                        | "navplayer-delete-cancel"
                                        | "navplayer-delete-confirm"
                                ) {
                                    events.image_actions.push(ImageAction {
                                        command: command.into_owned(),
                                        path: PathBuf::from(
                                            std::ffi::CStr::from_ptr(args[1])
                                                .to_string_lossy()
                                                .into_owned(),
                                        ),
                                    });
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        events
    }

    fn image_result(&self, ok: bool, message: &str) {
        let _ = self.command_string(&format!(
            "script-message navplayer-image-result {} {}",
            if ok { "ok" } else { "error" },
            quote_for_mpv_command(message)
        ));
    }

    fn image_action(&mut self, action: ImageAction) -> Result<()> {
        if action.command == "navplayer-delete-cancel" {
            self.delete_ticket = None;
            return Ok(());
        }
        let current = self
            .get_property_string("path")
            .ok_or_else(|| anyhow!("No file is open"))?;
        let current = std::path::absolute(Path::new(&current))?;
        anyhow::ensure!(
            current == std::path::absolute(&action.path)?,
            "The displayed file changed; operation cancelled"
        );
        if action.command == "navplayer-reveal-file" {
            crate::infra::file_dialog::reveal_file(&current)?;
            self.image_result(true, "Opened file location in Explorer");
            return Ok(());
        }
        anyhow::ensure!(
            self.get_property_bool("current-tracks/video/image")
                .unwrap_or(false),
            "This command only applies to still images"
        );
        match action.command.as_str() {
            "navplayer-delete-prepare" => {
                self.delete_ticket = Some(DeleteTicket::capture(&current)?);
            }
            "navplayer-image-next" | "navplayer-image-prev" => {
                self.delete_ticket = None;
                if let Some(path) = crate::infra::image_actions::adjacent_image(
                    &current,
                    action.command == "navplayer-image-next",
                )? {
                    self.load_video(&path)?;
                } else {
                    self.image_result(true, "No more images in this direction");
                }
            }
            "navplayer-delete-confirm" => {
                let ticket = self
                    .delete_ticket
                    .take()
                    .ok_or_else(|| anyhow!("No deletion confirmation is pending"))?;
                anyhow::ensure!(
                    ticket.path == current && ticket.unchanged()?,
                    "The file changed after confirmation was requested; deletion cancelled"
                );
                let next = crate::infra::image_actions::image_after_delete(&current)?;
                self.command_string("stop")?;
                self.ready = false;
                // Release the file before asking Windows to recycle it.
                let deadline = Instant::now() + Duration::from_secs(3);
                while self
                    .get_property_string("path")
                    .is_some_and(|path| !path.is_empty())
                {
                    anyhow::ensure!(
                        !self.drain_events().closed,
                        "Window closed; deletion cancelled"
                    );
                    if Instant::now() >= deadline {
                        let _ = self.load_video(&current);
                        return Err(anyhow!("Could not release the image file"));
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                let result = (|| {
                    anyhow::ensure!(ticket.unchanged()?, "The file changed; deletion cancelled");
                    crate::infra::image_actions::recycle_image(&current)
                })();
                if let Err(error) = result {
                    let _ = self.load_video(&current);
                    return Err(error);
                }
                self.image_result(true, "Moved to Recycle Bin");
                if let Some(path) = next {
                    self.load_video(&path)?;
                } else {
                    self.command_string("quit 0")?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn get_property_string(&self, name: &str) -> Option<String> {
        type GetString = unsafe extern "C" fn(*mut mpv_handle, *const c_char) -> *mut c_char;
        type Free = unsafe extern "C" fn(*mut c_void);
        let name = CString::new(name).ok()?;
        unsafe {
            let get = self
                .lib
                .get::<GetString>(b"mpv_get_property_string\0")
                .ok()?;
            let free = self.lib.get::<Free>(b"mpv_free\0").ok()?;
            let ptr = get(self.handle, name.as_ptr());
            if ptr.is_null() {
                return None;
            }
            let value = std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned();
            free(ptr.cast());
            Some(value)
        }
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
        self.command_string(&command).map_err(|err| {
            anyhow!(
                "failed to load support script {}: {err:#}",
                self.script_path.display()
            )
        })
    }

    fn load_video(&self, path: &Path) -> Result<()> {
        let path = std::path::absolute(path)?;
        self.is_image
            .set(crate::domain::media::is_image_path(&path));
        let raw_path = path.to_string_lossy();
        let quoted = quote_for_mpv_command(&raw_path);
        let command = format!("loadfile {quoted}");
        self.command_string(&command)
            .map_err(|err| anyhow!("failed to load {}: {err:#}", path.display()))
    }

    fn discover_window(&mut self) {
        if self.find_window().is_some() {
            self.seen_window = true;
        }
    }

    fn move_window(&self, x: i32, y: i32, width: i32, height: i32) {
        if let Some(hwnd) = self.find_window() {
            unsafe {
                ShowWindow(hwnd, SW_RESTORE);
                MoveWindow(hwnd, x, y, width, height, 1);
                SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
                );
                SetWindowPos(
                    hwnd,
                    HWND_NOTOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
                );
                focus_window(hwnd);
            }
        }
    }

    fn find_window(&self) -> Option<*mut c_void> {
        if let Some(hwnd) = self.hwnd.get() {
            if unsafe { IsWindow(hwnd as *mut c_void) } != 0 {
                return Some(hwnd as *mut c_void);
            }
            self.hwnd.set(None);
            CLAIMED_WINDOWS
                .get_or_init(Default::default)
                .lock()
                .unwrap()
                .remove(&hwnd);
        }
        let path = self.get_property_string("path")?;
        let caption = format!("naVPlayer {} | {path}", crate::app::APP_VERSION);
        let wide: Vec<u16> = caption.encode_utf16().chain(Some(0)).collect();
        let mut after = std::ptr::null_mut();
        loop {
            let hwnd = unsafe {
                FindWindowExW(std::ptr::null_mut(), after, std::ptr::null(), wide.as_ptr())
            };
            if hwnd.is_null() {
                return None;
            }
            after = hwnd;
            let mut owner = 0;
            unsafe {
                GetWindowThreadProcessId(hwnd, &mut owner);
            }
            if owner != std::process::id() {
                continue;
            }
            if CLAIMED_WINDOWS
                .get_or_init(Default::default)
                .lock()
                .unwrap()
                .insert(hwnd as isize)
            {
                self.hwnd.set(Some(hwnd as isize));
                return Some(hwnd);
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
            let func: libloading::Symbol<'_, MpvCommandString> =
                self.lib.get(b"mpv_command_string\0")?;
            func(self.handle, command.as_ptr())
        };
        self.check_status(status, "mpv_command_string")
    }

    fn set_option_string(&self, name: &str, value: &str) -> Result<()> {
        let option_name = name.to_owned();
        let name = CString::new(name)?;
        let value = CString::new(value)?;
        let status = unsafe {
            let func: libloading::Symbol<'_, MpvSetOptionString> =
                self.lib.get(b"mpv_set_option_string\0")?;
            func(self.handle, name.as_ptr(), value.as_ptr())
        };
        self.check_status(status, &format!("mpv_set_option_string({option_name})"))
    }

    fn set_property_bool(&self, name: &str, value: bool) -> Result<()> {
        let name = CString::new(name)?;
        let mut raw = if value { 1 } else { 0 };
        let status = unsafe {
            let func: libloading::Symbol<'_, MpvSetProperty> =
                self.lib.get(b"mpv_set_property\0")?;
            func(
                self.handle,
                name.as_ptr(),
                MPV_FORMAT_FLAG,
                (&mut raw as *mut c_int).cast::<c_void>(),
            )
        };
        self.check_status(status, "mpv_set_property(flag)")
    }

    fn set_property_f64(&self, name: &str, value: f64) -> Result<()> {
        let name = CString::new(name)?;
        let mut raw = value as c_double;
        let status = unsafe {
            let func: libloading::Symbol<'_, MpvSetProperty> =
                self.lib.get(b"mpv_set_property\0")?;
            func(
                self.handle,
                name.as_ptr(),
                MPV_FORMAT_DOUBLE,
                (&mut raw as *mut c_double).cast::<c_void>(),
            )
        };
        self.check_status(status, "mpv_set_property(double)")
    }

    fn get_property_bool(&self, name: &str) -> Result<bool> {
        let name = CString::new(name)?;
        let mut raw: c_int = 0;
        let status = unsafe {
            let func: libloading::Symbol<'_, MpvGetProperty> =
                self.lib.get(b"mpv_get_property\0")?;
            func(
                self.handle,
                name.as_ptr(),
                MPV_FORMAT_FLAG,
                (&mut raw as *mut c_int).cast::<c_void>(),
            )
        };
        self.check_status(status, "mpv_get_property(flag)")?;
        Ok(raw != 0)
    }

    fn get_property_f64(&self, name: &str) -> Result<f64> {
        let name = CString::new(name)?;
        let mut raw: c_double = 0.0;
        let status = unsafe {
            let func: libloading::Symbol<'_, MpvGetProperty> =
                self.lib.get(b"mpv_get_property\0")?;
            func(
                self.handle,
                name.as_ptr(),
                MPV_FORMAT_DOUBLE,
                (&mut raw as *mut c_double).cast::<c_void>(),
            )
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
            let func: libloading::Symbol<'_, MpvErrorString> =
                match self.lib.get(b"mpv_error_string\0") {
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
        if let Some(hwnd) = self.hwnd.take() {
            CLAIMED_WINDOWS
                .get_or_init(Default::default)
                .lock()
                .unwrap()
                .remove(&hwnd);
        }
        unsafe {
            if !self.handle.is_null()
                && let Ok(func) = self
                    .lib
                    .get::<MpvTerminateDestroy>(b"mpv_terminate_destroy\0")
            {
                func(self.handle);
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
        "RIGHT repeatable script-message navplayer-horizontal right",
        "LEFT repeatable script-message navplayer-horizontal left",
        "DEL script-message navplayer-delete-image",
        "e script-message navplayer-open-folder",
        "E script-message navplayer-open-folder",
        "h script-message navplayer-help",
        "H script-message navplayer-help",
        "i script-message navplayer-help",
        "I script-message navplayer-help",
        "n script-message navplayer-open-adjacent next",
        "p script-message navplayer-open-adjacent prev",
        "SPACE cycle pause",
        "f cycle fullscreen",
        "Shift+f set fullscreen no",
        "c script-message navplayer-capture-thumbnail",
        "Ctrl+b no-osd set time-pos 0",
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

local pending_delete = nil
local delete_busy = false
local confirmation_timer = nil
local confirmation_keys = {}

local help_visible = false
local help_overlay = mp.create_osd_overlay('ass-events')
help_overlay.z = 1000

local function hide_help()
    help_visible = false
    help_overlay:remove()
    mp.remove_key_binding('navplayer-help-close')
end

local function render_help()
    if not help_visible then return end
    local image = mp.get_property_native('current-tracks/video/image', false)
    local lines = { image and 'ショートカット一覧  —  静止画' or 'ショートカット一覧  —  動画', '' }
    if image then
        table.insert(lines, '← / →     前 / 次の静止画（動画は飛ばす）')
        table.insert(lines, 'Delete       削除の確認 → Y: ごみ箱 / N: キャンセル')
    else
        table.insert(lines, 'Space        再生 / 一時停止')
        table.insert(lines, '← / →     10秒巻き戻し / 10秒早送り')
        table.insert(lines, '↑ / ↓      音量を上げる / 下げる')
        table.insert(lines, 'Ctrl+B        動画の先頭へ戻る')
    end
    table.insert(lines, 'E                Explorerでファイルの場所を開く')
    table.insert(lines, 'N / P          同じフォルダの次 / 前のファイル')
    table.insert(lines, 'F                フルスクリーン切り替え')
    table.insert(lines, 'Shift+F       通常ウィンドウへ戻る')
    table.insert(lines, 'C                現在フレームをサムネとして保存')
    table.insert(lines, 'Q / Alt+F4  この表示画面を閉じる')
    table.insert(lines, '')
    table.insert(lines, 'H / I / Esc   この一覧を閉じる')
    local w, h = mp.get_osd_size()
    if not w or not h or w <= 0 or h <= 0 then return end
    local size = math.max(14, math.floor(math.min(28, (h - 48) / (#lines * 1.1), (w - 48) / 25)))
    local height = math.min(h - 24, math.ceil(#lines * size * 1.4 + 28))
    help_overlay.res_x = w
    help_overlay.res_y = h
    local background = string.format('{\\an7\\pos(12,12)\\bord0\\shad0\\c&H101010&\\1a&H25&\\p1}m 0 0 l %d 0 %d %d 0 %d{\\p0}', w - 24, w - 24, height, height)
    local foreground = string.format('{\\an7\\pos(28,24)\\fnYu Gothic UI\\fs%d\\bord0\\shad0\\c&HFFFFFF&}', size)
    help_overlay.data = background .. '\n' .. foreground .. table.concat(lines, '\\N')
    help_overlay:update()
end

mp.observe_property('osd-dimensions', 'native', render_help)
mp.register_event('file-loaded', render_help)
mp.register_event('shutdown', hide_help)

local function clear_delete_prompt(notify)
    local path = pending_delete
    pending_delete = nil
    for _, name in ipairs(confirmation_keys) do mp.remove_key_binding(name) end
    confirmation_keys = {}
    if confirmation_timer then confirmation_timer:kill(); confirmation_timer = nil end
    if notify and path then mp.commandv('script-message', 'navplayer-delete-cancel', path) end
    mp.osd_message('', 0)
end

mp.register_script_message('navplayer-help', function()
    if help_visible then hide_help(); return end
    if delete_busy then return end
    clear_delete_prompt(true)
    help_visible = true
    mp.add_forced_key_binding('ESC', 'navplayer-help-close', hide_help)
    render_help()
end)

mp.register_script_message('navplayer-open-folder', function()
    if delete_busy then return end
    hide_help()
    clear_delete_prompt(true)
    local path = mp.get_property('path')
    if path and path ~= '' then
        mp.commandv('script-message', 'navplayer-reveal-file', path)
    end
end)

mp.register_script_message('navplayer-delete-image', function()
    if pending_delete or delete_busy or not mp.get_property_native('current-tracks/video/image', false) then return end
    hide_help()
    local path = mp.get_property('path')
    if not path or path == '' then return end
    pending_delete = path
    mp.commandv('script-message', 'navplayer-delete-prepare', path)
    local function cancel()
        clear_delete_prompt(true)
        mp.osd_message('キャンセルしました', 1)
    end
    local function confirm()
        local target = pending_delete
        if not target or target ~= mp.get_property('path') then cancel(); return end
        clear_delete_prompt(false)
        delete_busy = true
        mp.osd_message('ごみ箱へ移動中...', 10)
        mp.commandv('script-message', 'navplayer-delete-confirm', target)
    end
    local bindings = {
        {'y', confirm}, {'Y', confirm}, {'n', cancel}, {'N', cancel},
        {'ENTER', cancel}, {'ESC', cancel}, {'LEFT', cancel}, {'RIGHT', cancel},
        {'p', cancel}, {'P', cancel}, {'DEL', cancel}
    }
    for index, binding in ipairs(bindings) do
        local name = 'navplayer-delete-confirm-' .. index
        table.insert(confirmation_keys, name)
        mp.add_forced_key_binding(binding[1], name, binding[2])
    end
    confirmation_timer = mp.add_timeout(60, cancel)
    mp.osd_message('削除しますか？ y/N\nごみ箱へ移動: ' .. path, 60)
end)

mp.register_event('start-file', function() clear_delete_prompt(true) end)
mp.register_script_message('navplayer-image-result', function(status, message)
    delete_busy = false
    if status ~= 'ok' then clear_delete_prompt(true) end
    mp.osd_message(message or '', 3)
end)

local last_image_arrow = -1
mp.register_script_message('navplayer-horizontal', function(direction)
    if pending_delete or delete_busy then return end
    if direction ~= 'left' and direction ~= 'right' then return end
    local forward = direction == 'right'
    -- No track (e.g. while loading): do nothing instead of guessing its type.
    local is_image = mp.get_property_native('current-tracks/video/image')
    if is_image == true then
        local now = mp.get_time()
        if now - last_image_arrow < 0.18 then return end
        last_image_arrow = now
        local path = mp.get_property('path')
        if path then mp.commandv('script-message', forward and 'navplayer-image-next' or 'navplayer-image-prev', path) end
        return
    end
    -- Videos only seek within the current file. Never issue a loadfile here.
    if is_image == false then
        mp.commandv('script-message', 'navplayer-safe-seek', forward and '10' or '-10', '3')
    end
end)

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
    if mp.get_property_native('current-tracks/video/image', false) then return end
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

local video_extensions = { __VIDEO_EXTENSIONS__ }
local function supported_video(name)
    local extension = name:lower():match('%.([^%.]+)$')
    return extension and video_extensions[extension] == true
end

local function basename(path)
    local normalized = path:gsub('/', '\\')
    return normalized:match('([^\\]+)$') or normalized
end

local function list_siblings(video_path)
    local dir = split_dir_and_stem(video_path)
    local entries = utils.readdir(dir, 'files') or {}
    local files = {}

    for _, name in ipairs(entries) do
        if supported_video(name) then
            table.insert(files, name)
        end
    end

    table.sort(files, function(a, b)
        return a:lower() < b:lower()
    end)

    return dir, files
end

mp.register_script_message('navplayer-open-adjacent', function(direction)
    local current_path = mp.get_property('path')
    if not current_path or current_path == '' then
        return
    end

    local dir, files = list_siblings(current_path)
    local current_name = basename(current_path)
    local current_index = nil

    for index, name in ipairs(files) do
        if name == current_name then
            current_index = index
            break
        end
    end

    if not current_index then
        return
    end

    local target_index = nil
    if direction == 'next' then
        if current_index >= #files then
            return
        end
        target_index = current_index + 1
    else
        if current_index <= 1 then
            return
        end
        target_index = current_index - 1
    end

    local target_path = utils.join_path(dir, files[target_index])
    mp.commandv('loadfile', target_path, 'replace')
    mp.add_timeout(0.05, show_current_file)
end)
"#;
    let extensions = crate::domain::media::VIDEO_EXTENSIONS
        .iter()
        .chain(crate::domain::media::IMAGE_EXTENSIONS)
        .map(|extension| format!("['{extension}'] = true"))
        .collect::<Vec<_>>()
        .join(", ");
    let content = content.replace("__VIDEO_EXTENSIONS__", &extensions);
    fs::write(&path, content.trim_start())?;
    Ok(path)
}

fn quote_for_mpv_command(input: &str) -> String {
    let escaped = input.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
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

#[cfg(test)]
mod tests {
    use super::*;

    fn image_test_backend(name: &str) -> (LibMpvBackend, Vec<VideoItem>, PathBuf) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/validation");
        let folder = root.join(format!(
            "{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&folder).unwrap();
        for name in ["photo1.jpg", "photo2.jpg", "photo10.jpg"] {
            fs::copy(root.join("still.jpg"), folder.join(name)).unwrap();
        }
        fs::copy(root.join("h264-aac.mp4"), folder.join("movie.mp4")).unwrap();
        let path = folder.join("photo1.jpg");
        let player = headless(0);
        player.load_video(&path).unwrap();
        let mut backend = LibMpvBackend {
            players: vec![player],
            ..LibMpvBackend::default()
        };
        let mut items = vec![VideoItem::new(0, path)];
        pump_until(&mut backend, &mut items, |backend| backend.players[0].ready);
        std::thread::sleep(Duration::from_millis(100));
        (backend, items, folder)
    }

    fn pump_until(
        backend: &mut LibMpvBackend,
        items: &mut [VideoItem],
        done: impl Fn(&LibMpvBackend) -> bool,
    ) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let tick = backend.tick(items, PlaybackState::Paused, Some(0), false, false);
            assert!(!tick.failed, "{:?}", tick.selected_render_status);
            if done(backend) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "image action timeout; status={:?}",
                tick.selected_render_status
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn press(backend: &LibMpvBackend, key: &str) {
        backend.players[0]
            .command_string(&format!("keypress {key}"))
            .unwrap();
    }
    fn displayed(backend: &LibMpvBackend, path: &Path) -> bool {
        backend.players[0].ready
            && backend.players[0]
                .get_property_string("path")
                .is_some_and(|current| Path::new(&current) == path)
    }
    fn ask_delete(backend: &mut LibMpvBackend, items: &mut [VideoItem]) {
        press(backend, "DEL");
        pump_until(backend, items, |backend| {
            backend.players[0].delete_ticket.is_some()
        });
        std::thread::sleep(Duration::from_millis(40));
    }

    #[test]
    #[ignore = "requires bundled libmpv and synthetic fixtures"]
    fn real_mpv_image_arrows_and_delete_cancellation() {
        let (mut backend, mut items, folder) = image_test_backend("image-keys");
        press(&backend, "RIGHT");
        pump_until(&mut backend, &mut items, |backend| {
            displayed(backend, &folder.join("photo2.jpg"))
        });
        std::thread::sleep(Duration::from_millis(200));
        press(&backend, "RIGHT");
        pump_until(&mut backend, &mut items, |backend| {
            displayed(backend, &folder.join("photo10.jpg"))
        });
        std::thread::sleep(Duration::from_millis(200));
        press(&backend, "LEFT");
        pump_until(&mut backend, &mut items, |backend| {
            displayed(backend, &folder.join("photo2.jpg"))
        });
        for key in ["n", "N", "ENTER", "ESC", "LEFT", "RIGHT"] {
            ask_delete(&mut backend, &mut items);
            assert!(folder.join("photo2.jpg").is_file());
            press(&backend, key);
            pump_until(&mut backend, &mut items, |backend| {
                backend.players[0].delete_ticket.is_none()
            });
            assert!(displayed(&backend, &folder.join("photo2.jpg")));
            assert!(folder.join("photo2.jpg").is_file());
        }
        // Y without an active prompt never deletes a file.
        press(&backend, "y");
        std::thread::sleep(Duration::from_millis(50));
        backend.tick(&mut items, PlaybackState::Paused, Some(0), false, false);
        assert!(folder.join("photo2.jpg").is_file());
    }

    #[test]
    #[ignore = "requires bundled libmpv and synthetic seek-video.mp4 fixture"]
    fn real_mpv_video_arrows_seek_without_changing_files_even_at_end() {
        let (mut backend, mut items, folder) = image_test_backend("video-arrow-separation");
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/validation/seek-video.mp4");
        let video = folder.join("video1.mp4");
        fs::copy(&source, &video).unwrap();
        fs::copy(&source, folder.join("video2.mp4")).unwrap();
        backend.players[0].load_video(&video).unwrap();
        backend.players[0].set_property_bool("pause", true).unwrap();
        pump_until(&mut backend, &mut items, |backend| {
            displayed(backend, &video)
                && backend.players[0]
                    .get_property_f64("duration")
                    .is_ok_and(|duration| duration > 39.0)
        });
        backend.seek(15.0).unwrap();
        pump_until(&mut backend, &mut items, |backend| {
            backend.players[0]
                .get_property_f64("time-pos")
                .is_ok_and(|pos| (pos - 15.0).abs() < 0.2)
        });
        press(&backend, "RIGHT");
        pump_until(&mut backend, &mut items, |backend| {
            backend.players[0]
                .get_property_f64("time-pos")
                .is_ok_and(|pos| (pos - 25.0).abs() < 0.2)
        });
        assert!(displayed(&backend, &video));
        press(&backend, "LEFT");
        pump_until(&mut backend, &mut items, |backend| {
            backend.players[0]
                .get_property_f64("time-pos")
                .is_ok_and(|pos| (pos - 15.0).abs() < 0.2)
        });
        assert!(displayed(&backend, &video));
        backend.seek(36.8).unwrap();
        pump_until(&mut backend, &mut items, |backend| {
            backend.players[0]
                .get_property_f64("time-pos")
                .is_ok_and(|pos| pos > 36.5)
        });
        for _ in 0..3 {
            press(&backend, "RIGHT");
            std::thread::sleep(Duration::from_millis(40));
        }
        backend.tick(&mut items, PlaybackState::Paused, Some(0), false, false);
        assert!(
            displayed(&backend, &video),
            "Right at the end must not advance to another file"
        );
        assert!(backend.players[0].delete_ticket.is_none());
    }

    #[test]
    #[ignore = "recycles only generated test images; requires Windows Recycle Bin"]
    fn real_mpv_confirmed_delete_recycles_and_advances() {
        let (mut backend, mut items, folder) = image_test_backend("image-recycle");
        ask_delete(&mut backend, &mut items);
        press(&backend, "y");
        pump_until(&mut backend, &mut items, |backend| {
            displayed(backend, &folder.join("photo2.jpg")) && !folder.join("photo1.jpg").exists()
        });
        assert!(folder.join("photo2.jpg").is_file());
        assert!(folder.join("photo10.jpg").is_file());
        assert!(folder.join("movie.mp4").is_file());
        std::thread::sleep(Duration::from_millis(200));
        press(&backend, "RIGHT");
        pump_until(&mut backend, &mut items, |backend| {
            displayed(backend, &folder.join("photo10.jpg"))
        });
        ask_delete(&mut backend, &mut items);
        press(&backend, "Y");
        pump_until(&mut backend, &mut items, |backend| {
            displayed(backend, &folder.join("photo2.jpg")) && !folder.join("photo10.jpg").exists()
        });
        ask_delete(&mut backend, &mut items);
        press(&backend, "y");
        pump_until(&mut backend, &mut items, |backend| {
            backend.players[0].shutdown_received
        });
        assert!(!folder.join("photo2.jpg").exists());
        assert!(folder.join("movie.mp4").is_file());
    }

    #[test]
    #[ignore = "tests failed recycle of a locked generated image"]
    fn real_mpv_failed_delete_preserves_and_restores_the_image() {
        use std::os::windows::fs::OpenOptionsExt;
        let (mut backend, mut items, folder) = image_test_backend("image-locked");
        let path = folder.join("photo1.jpg");
        // Allow reads but deliberately deny FILE_SHARE_DELETE.
        let lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&path)
            .unwrap();
        ask_delete(&mut backend, &mut items);
        press(&backend, "y");
        pump_until(&mut backend, &mut items, |backend| {
            backend.players[0].delete_ticket.is_none() && displayed(backend, &path)
        });
        assert!(path.is_file());
        assert!(folder.join("photo2.jpg").is_file());
        drop(lock);
        std::thread::sleep(Duration::from_millis(100));
        ask_delete(&mut backend, &mut items);
        press(&backend, "n");
        pump_until(&mut backend, &mut items, |backend| {
            backend.players[0].delete_ticket.is_none()
        });
        assert!(path.is_file());
    }

    #[test]
    #[ignore = "requires bundled libmpv and synthetic fixtures"]
    fn real_mpv_navigation_invalidates_delete_confirmation() {
        let (mut backend, mut items, folder) = image_test_backend("image-stale");
        ask_delete(&mut backend, &mut items);
        backend.players[0]
            .load_video(&folder.join("photo10.jpg"))
            .unwrap();
        pump_until(&mut backend, &mut items, |backend| {
            displayed(backend, &folder.join("photo10.jpg"))
                && backend.players[0].delete_ticket.is_none()
        });
        std::thread::sleep(Duration::from_millis(50));
        press(&backend, "y");
        std::thread::sleep(Duration::from_millis(50));
        backend.tick(&mut items, PlaybackState::Paused, Some(0), false, false);
        assert!(folder.join("photo1.jpg").is_file());
        assert!(folder.join("photo10.jpg").is_file());
    }

    fn headless(id: usize) -> WindowMpv {
        WindowMpv::with_options(
            format!("naVPlayer-test-{}-{id}", std::process::id()),
            id,
            &[
                ("vo", "null"),
                ("ao", "null"),
                ("osc", "no"),
                ("hwdec", "no"),
            ],
        )
        .unwrap()
    }

    #[test]
    #[ignore = "requires bundled libmpv and scripts/verify_codecs.py fixtures"]
    fn real_mpv_still_images_do_not_participate_in_video_sync_or_seek() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/validation");
        for extension in crate::domain::media::IMAGE_EXTENSIONS {
            let image_path = root.join(format!("still.{extension}"));
            let video_path = root.join("h264-aac.mp4");
            let image = headless(0);
            let video = headless(1);
            image.load_video(&image_path).unwrap();
            image.set_property_bool("pause", true).unwrap();
            video.load_video(&video_path).unwrap();
            video.set_property_bool("pause", true).unwrap();
            let mut backend = LibMpvBackend {
                players: vec![image, video],
                ..LibMpvBackend::default()
            };
            let mut items = vec![VideoItem::new(0, image_path), VideoItem::new(1, video_path)];
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                let snapshot =
                    backend.tick(&mut items, PlaybackState::Paused, Some(0), true, false);
                assert!(
                    !snapshot.failed,
                    "{extension}: {:?}",
                    snapshot.selected_render_status
                );
                assert!(snapshot.closed_player_ids.is_empty());
                if backend.players.iter().all(|player| player.ready) {
                    break;
                }
                assert!(Instant::now() < deadline, "image load timeout: {extension}");
                std::thread::sleep(Duration::from_millis(10));
            }
            assert!(
                backend.players[0]
                    .get_property_bool("current-tracks/video/image")
                    .unwrap()
            );
            assert_eq!(
                backend.players[0]
                    .get_property_string("image-display-duration")
                    .as_deref(),
                Some("inf")
            );
            backend.play(1.0).unwrap();
            assert!(backend.players[0].get_property_bool("pause").unwrap());
            backend.seek(0.2).unwrap();
            let snapshot = backend.tick(&mut items, PlaybackState::Playing, Some(0), true, false);
            assert!(
                snapshot.anchor_position.is_some(),
                "video remains the anchor even when image is selected"
            );
            assert_eq!(items[0].duration_sec, 0.0);
            assert_eq!(items[0].position_sec, 0.0);
            backend.stop().unwrap();
        }
    }

    #[test]
    #[ignore = "requires bundled libmpv and scripts/verify_codecs.py fixtures"]
    fn real_mpv_quit_after_eof_reports_shutdown_and_second_player_survives() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/validation/h264-aac.mp4");
        assert!(path.is_file());
        let mut first = headless(0);
        let mut second = headless(1);
        first.load_video(&path).unwrap();
        second.load_video(&path).unwrap();
        first.set_property_bool("pause", false).unwrap();
        second.set_property_bool("pause", false).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !(first.ready && second.ready) {
            assert!(Instant::now() < deadline, "file-loaded timeout");
            assert!(!first.drain_events().closed);
            assert!(!second.drain_events().closed);
            std::thread::sleep(Duration::from_millis(10));
        }
        // Normal EOF should keep both windows/players open, not request app exit.
        std::thread::sleep(Duration::from_millis(800));
        assert!(!first.drain_events().closed);
        first.command_string("quit 0").unwrap();
        loop {
            if first.drain_events().closed {
                break;
            }
            assert!(Instant::now() < deadline, "shutdown timeout");
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(first);
        assert!(!second.drain_events().closed);
        assert!(second.get_property_f64("duration").unwrap() > 0.0);
        second.command_string("quit 0").unwrap();
        while !second.drain_events().closed {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    #[ignore = "requires bundled libmpv"]
    fn real_mpv_load_failure_is_reported_even_without_a_video_window() {
        let mut player = headless(2);
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/validation/missing-file.mkv");
        player.load_video(&path).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let event = player.drain_events();
            if event.closed {
                assert!(event.error.unwrap().starts_with("Playback error:"));
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    #[ignore = "requires bundled libmpv and scripts/verify_codecs.py fixtures"]
    fn real_mpv_next_previous_navigation_accepts_mixed_containers() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/validation");
        let folder = root.join("navigation");
        fs::create_dir_all(&folder).unwrap();
        let first = folder.join("a.mp4");
        let second = folder.join("b.WEBM");
        let third = folder.join("c.PNG");
        fs::copy(root.join("h264-aac.mp4"), &first).unwrap();
        fs::copy(root.join("vp9-opus.webm"), &second).unwrap();
        fs::copy(root.join("still.png"), &third).unwrap();
        let mut player = headless(3);
        player.load_video(&first).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !player.ready {
            assert!(!player.drain_events().closed);
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        // The Lua script is initialized on mpv's script thread.
        std::thread::sleep(Duration::from_millis(100));
        for (direction, target) in [
            ("next", &second),
            ("next", &third),
            ("prev", &second),
            ("prev", &first),
        ] {
            player
                .command_string(&format!(
                    "script-message navplayer-open-adjacent {direction}"
                ))
                .unwrap();
            loop {
                assert!(!player.drain_events().closed);
                if player
                    .get_property_string("path")
                    .is_some_and(|path| Path::new(&path) == target)
                {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "navigation to {} timed out",
                    target.display()
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}
