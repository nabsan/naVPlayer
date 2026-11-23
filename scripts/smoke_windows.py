"""Windows process/window lifecycle smoke test against an isolated IPC endpoint.

Usage: python scripts/smoke_windows.py <path-to-navplayer.exe>
Only windows belonging to the processes started by this script are closed.
Run verify_codecs.py first to generate the media fixtures.
"""
import ctypes as C
from ctypes import wintypes as W
import json
import os
from pathlib import Path
import socket
import shutil
import subprocess
import sys
import time
import tempfile

ROOT = Path(__file__).resolve().parents[1]
MEDIA = ROOT / "target" / "validation"
EXE = Path(sys.argv[1]).resolve()
user32 = C.WinDLL("user32", use_last_error=True)
kernel32 = C.WinDLL("kernel32", use_last_error=True)
callback_type = C.WINFUNCTYPE(W.BOOL, W.HWND, W.LPARAM)
user32.EnumWindows.argtypes = [callback_type, W.LPARAM]
user32.GetWindowThreadProcessId.argtypes = [W.HWND, C.POINTER(W.DWORD)]
user32.GetWindowTextW.argtypes = [W.HWND, W.LPWSTR, C.c_int]
user32.PostMessageW.argtypes = [W.HWND, W.UINT, W.WPARAM, W.LPARAM]
user32.IsIconic.argtypes = [W.HWND]
kernel32.OpenProcess.argtypes = [W.DWORD, W.BOOL, W.DWORD]
kernel32.OpenProcess.restype = W.HANDLE
kernel32.CloseHandle.argtypes = [W.HANDLE]
kernel32.GetProcessTimes.argtypes = [W.HANDLE] + [C.POINTER(W.FILETIME)] * 4


def windows(pid, playback=False):
    result = []

    @callback_type
    def visit(hwnd, _):
        owner = W.DWORD()
        user32.GetWindowThreadProcessId(hwnd, C.byref(owner))
        if owner.value == pid:
            title = C.create_unicode_buffer(1024)
            user32.GetWindowTextW(hwnd, title, len(title))
            is_player = " | " in title.value
            if title.value.startswith("naVPlayer ver.") and is_player == playback:
                result.append(hwnd)
        return True

    user32.EnumWindows(visit, 0)
    return result


def wait_until(predicate, description, timeout=20):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(0.05)
    raise AssertionError(description)


def cpu_seconds(pid):
    handle = kernel32.OpenProcess(0x1000, False, pid)
    assert handle
    try:
        times = [W.FILETIME() for _ in range(4)]
        assert kernel32.GetProcessTimes(handle, *[C.byref(value) for value in times])
        return sum((value.dwHighDateTime << 32) | value.dwLowDateTime for value in times[2:]) / 10_000_000
    finally:
        kernel32.CloseHandle(handle)


def start(*files):
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    env = dict(os.environ, NAVPLAYER_IPC_ADDR=f"127.0.0.1:{port}")
    process = subprocess.Popen([str(EXE), *map(str, files)], cwd=EXE.parent, env=env)
    return process, env, port


def close(hwnd):
    assert user32.PostMessageW(hwnd, 0x0010, 0, 0)  # WM_CLOSE, identical to titlebar X


def cleanup(process):
    if process.poll() is None:
        for hwnd in windows(process.pid):
            close(hwnd)
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.terminate()
            process.wait(timeout=10)


def main():
    results = []
    process, _, _ = start()
    try:
        parent = wait_until(lambda: windows(process.pid), "idle parent missing")[0]
        time.sleep(1)
        before = cpu_seconds(process.pid)
        start_time = time.monotonic()
        time.sleep(3)
        cpu = (cpu_seconds(process.pid) - before) / (time.monotonic() - start_time) * 100
        assert process.poll() is None
        close(parent)
        assert process.wait(timeout=10) == 0
        results.append({"case": "idle-parent-close", "status": "passed", "cpu_percent_one_core": round(cpu, 3)})
    finally:
        cleanup(process)

    process, _, _ = start(MEDIA / "h264-aac.mp4", MEDIA / "hevc-aac.mkv")
    try:
        children = wait_until(lambda: (items if len(items := windows(process.pid, True)) == 2 else None), "two playback windows missing")
        close(children[0])
        wait_until(lambda: len(windows(process.pid, True)) == 1, "first window did not close")
        time.sleep(0.3)
        assert process.poll() is None, "parent exited while another video was open"
        close(windows(process.pid, True)[0])
        assert process.wait(timeout=10) == 0
        results.append({"case": "two-windows-last-close-exits", "status": "passed"})
    finally:
        cleanup(process)

    process, env, port = start()
    try:
        wait_until(lambda: windows(process.pid), "IPC parent missing")
        # Launching a WebM file reuses and wakes the otherwise idle original instance.
        forwarded = subprocess.run([str(EXE), str(MEDIA / "vp9-opus.webm")], env=env, timeout=10)
        assert forwarded.returncode == 0
        child = wait_until(lambda: windows(process.pid, True), "IPC did not wake idle UI")[0]
        close(child)
        assert process.wait(timeout=10) == 0
        results.append({"case": "ipc-webm-reuse-and-single-close", "status": "passed"})
    finally:
        cleanup(process)

    broken = MEDIA / "broken.mkv"
    broken.write_text("not a video", encoding="ascii")
    process, _, _ = start(broken)
    try:
        parent = wait_until(lambda: windows(process.pid), "failure parent missing")[0]
        wait_until(lambda: not user32.IsIconic(parent) and not windows(process.pid, True), "failure did not restore parent")
        time.sleep(0.5)
        assert process.poll() is None
        close(parent)
        assert process.wait(timeout=10) == 0
        results.append({"case": "broken-file-restores-parent", "status": "passed"})
    finally:
        cleanup(process)
    # Keep images open beyond mpv's default five-second image timeout.
    process, _, _ = start(MEDIA / "still.jpg", MEDIA / "still.png")
    try:
        children = wait_until(lambda: (items if len(items := windows(process.pid, True)) == 2 else None), "image windows missing")
        time.sleep(5.5)
        assert process.poll() is None and len(windows(process.pid, True)) == 2
        close(children[0])
        wait_until(lambda: len(windows(process.pid, True)) == 1, "first image did not close")
        assert process.poll() is None
        close(windows(process.pid, True)[0])
        assert process.wait(timeout=10) == 0
        results.append({"case": "jpg-png-persist-and-last-close-exits", "status": "passed"})
    finally:
        cleanup(process)

    process, _, _ = start(MEDIA / "still.png", MEDIA / "h264-aac.mp4")
    try:
        children = wait_until(lambda: (items if len(items := windows(process.pid, True)) == 2 else None), "mixed media windows missing")
        close(children[0])
        wait_until(lambda: len(windows(process.pid, True)) == 1, "mixed first close failed")
        assert process.poll() is None
        close(windows(process.pid, True)[0])
        assert process.wait(timeout=10) == 0
        results.append({"case": "mixed-video-image-close", "status": "passed"})
    finally:
        cleanup(process)
    # Exercise the native Windows Delete/Enter/Y path on an expendable image.
    delete_folder = MEDIA / f"delete-smoke-{time.time_ns()}"
    delete_folder.mkdir()
    delete_image = delete_folder / "delete-me.jpg"
    shutil.copy2(MEDIA / "still.jpg", delete_image)
    process, _, _ = start(delete_image)
    try:
        child = wait_until(lambda: windows(process.pid, True), "delete image window missing")[0]
        time.sleep(0.5)
        def special_key(key):
            # The dedicated Delete key is extended; without this bit mpv sees KP_DEL.
            scan = user32.MapVirtualKeyW(key, 0)
            flags = 1 | (scan << 16) | (0x01000000 if key == 0x2E else 0)
            assert user32.PostMessageW(child, 0x0100, key, flags)
            assert user32.PostMessageW(child, 0x0101, key, flags | 0xC0000000)
        special_key(0x2E)  # Delete opens the confirmation, without changing the file.
        time.sleep(0.4)
        assert delete_image.is_file()
        special_key(0x0D)  # Enter chooses the default N.
        time.sleep(0.4)
        assert delete_image.is_file() and process.poll() is None
        special_key(0x2E)
        time.sleep(0.4)
        assert user32.PostMessageW(child, 0x0102, ord("y"), 1)  # WM_CHAR
        wait_until(lambda: not delete_image.exists(), "confirmed native delete did not recycle image")
        assert process.wait(timeout=10) == 0
        results.append({"case": "native-delete-default-no-confirm-y-and-exit", "status": "passed"})
    finally:
        cleanup(process)
    # Prove native arrow navigation by recycling the selected synthetic image.
    # This observes the actual displayed file, rather than just a queued key event.
    arrow_folder = MEDIA / f"arrow-smoke-{time.time_ns()}"
    arrow_folder.mkdir()
    for filename in ["photo1.jpg", "photo2.jpg", "photo10.jpg"]:
        shutil.copy2(MEDIA / "still.jpg", arrow_folder / filename)
    shutil.copy2(MEDIA / "h264-aac.mp4", arrow_folder / "movie.mp4")
    process, _, _ = start(arrow_folder / "photo1.jpg")
    try:
        child = wait_until(lambda: windows(process.pid, True), "arrow image window missing")[0]
        def native_key(key):
            scan = user32.MapVirtualKeyW(key, 0)
            flags = 1 | (scan << 16) | (0x01000000 if 0x21 <= key <= 0x2E else 0)
            assert user32.PostMessageW(child, 0x0100, key, flags)
            assert user32.PostMessageW(child, 0x0101, key, flags | 0xC0000000)
        time.sleep(0.6)
        native_key(0x27)  # Right selects photo2, skipping movie.mp4.
        time.sleep(0.6)
        native_key(0x2E)
        time.sleep(0.4)
        assert user32.PostMessageW(child, 0x0102, ord("y"), 1)
        wait_until(lambda: not (arrow_folder / "photo2.jpg").exists(), "Right did not select photo2.jpg")
        assert (arrow_folder / "photo1.jpg").is_file()
        assert (arrow_folder / "photo10.jpg").is_file()
        time.sleep(0.6)  # Automatic post-delete advance selects photo10.
        native_key(0x25)  # Left returns to photo1.
        time.sleep(0.6)
        native_key(0x2E)
        time.sleep(0.4)
        assert user32.PostMessageW(child, 0x0102, ord("y"), 1)
        wait_until(lambda: not (arrow_folder / "photo1.jpg").exists(), "Left did not select photo1.jpg")
        assert (arrow_folder / "photo10.jpg").is_file()
        assert (arrow_folder / "movie.mp4").is_file()
        close(windows(process.pid, True)[0])
        assert process.wait(timeout=10) == 0
        results.append({"case": "native-left-right-select-correct-still-and-skip-video", "status": "passed"})
    finally:
        cleanup(process)
    (MEDIA / "windows-smoke-results.json").write_text(json.dumps(results, indent=2), encoding="utf-8")
    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    # A private runtime directory also isolates recent_files.toml from real history.
    with tempfile.TemporaryDirectory(prefix="smoke-", dir=MEDIA) as temporary:
        runtime = Path(temporary).resolve()
        assert runtime.is_relative_to(MEDIA.resolve())
        shutil.copy2(EXE, runtime / "navplayer.exe")
        shutil.copy2(EXE.parent / "libmpv-2.dll", runtime / "libmpv-2.dll")
        EXE = runtime / "navplayer.exe"
        main()
