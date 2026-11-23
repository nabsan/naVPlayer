"""Generate tiny fixtures and decode them with the actual bundled libmpv (no UI).

Usage: python scripts/verify_codecs.py
Requires ffmpeg on PATH. Generated files stay under ignored target/validation.
"""
import ctypes as C
import json
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "target" / "validation"
OUT.mkdir(parents=True, exist_ok=True)
mpv = C.CDLL(str(ROOT / "libmpv-2.dll"))


class Event(C.Structure):
    _fields_ = [("id", C.c_int), ("error", C.c_int), ("userdata", C.c_uint64), ("data", C.c_void_p)]


class EndFile(C.Structure):
    _fields_ = [("reason", C.c_int), ("error", C.c_int)]


mpv.mpv_create.restype = C.c_void_p
mpv.mpv_initialize.argtypes = [C.c_void_p]
mpv.mpv_set_option_string.argtypes = [C.c_void_p, C.c_char_p, C.c_char_p]
mpv.mpv_command.argtypes = [C.c_void_p, C.POINTER(C.c_char_p)]
mpv.mpv_wait_event.argtypes = [C.c_void_p, C.c_double]
mpv.mpv_wait_event.restype = C.POINTER(Event)
mpv.mpv_get_property_string.argtypes = [C.c_void_p, C.c_char_p]
mpv.mpv_get_property_string.restype = C.c_void_p
mpv.mpv_free.argtypes = [C.c_void_p]
mpv.mpv_terminate_destroy.argtypes = [C.c_void_p]


def prop(handle, name):
    ptr = mpv.mpv_get_property_string(handle, name.encode())
    if not ptr:
        return None
    try:
        return C.string_at(ptr).decode("utf-8", "replace")
    finally:
        mpv.mpv_free(ptr)


CASES = [
    ("h264-aac.mp4", "libx264", "aac", ["-preset", "ultrafast"]),
    ("hevc-aac.mkv", "libx265", "aac", ["-preset", "ultrafast", "-x265-params", "pools=1:log-level=error"]),
    ("vp9-opus.webm", "libvpx-vp9", "libopus", ["-deadline", "realtime", "-cpu-used", "8"]),
    ("av1-opus.mkv", "libaom-av1", "libopus", ["-cpu-used", "8"]),
    ("mpeg4-mp3.avi", "mpeg4", "libmp3lame", []),
    ("mpeg2-mp2.mpg", "mpeg2video", "mp2", []),
    ("wmv2-wma.wmv", "wmv2", "wmav2", []),
    ("prores-pcm.mov", "prores_ks", "pcm_s16le", ["-pix_fmt", "yuv422p10le", "-profile:v", "0"]),
]


def main():
    subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi",
                    "-i", "color=size=64x48:rate=2", "-t", "40", "-c:v", "libx264",
                    "-preset", "ultrafast", "-g", "2", "-threads", "1", str(OUT / "seek-video.mp4")],
                   check=True, capture_output=True)
    for extension in ["jpg", "jpeg", "png", "bmp", "webp", "tif", "tiff"]:
        subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y",
                        "-f", "lavfi", "-i", "testsrc2=size=160x120",
                        "-frames:v", "1", "-threads", "1", str(OUT / f"still.{extension}")],
                       check=True, capture_output=True)
    results = []
    for filename, video, audio, extra in CASES:
        path = OUT / filename
        command = ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y",
                   "-f", "lavfi", "-i", "testsrc2=size=128x96:rate=25",
                   "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000",
                   "-t", "0.6", "-c:v", video, "-threads", "2", "-c:a", audio,
                   *extra, str(path)]
        subprocess.run(command, check=True, capture_output=True)
        handle = mpv.mpv_create()
        assert handle
        try:
            for name, value in {"config": "no", "vo": "null", "ao": "null",
                                "hwdec": "no", "idle": "yes", "keep-open": "no",
                                "terminal": "no", "untimed": "yes"}.items():
                assert mpv.mpv_set_option_string(handle, name.encode(), value.encode()) >= 0, name
            assert mpv.mpv_initialize(handle) >= 0
            version = prop(handle, "mpv-version")
            args = (C.c_char_p * 3)(b"loadfile", str(path).encode(), None)
            assert mpv.mpv_command(handle, args) >= 0
            loaded = False
            eof = False
            codecs = {}
            deadline = time.monotonic() + 15
            while time.monotonic() < deadline:
                event = mpv.mpv_wait_event(handle, 0.1).contents
                if event.id == 8:
                    loaded = True
                    codecs = {"video": prop(handle, "video-codec"), "audio": prop(handle, "audio-codec")}
                if event.id == 7:
                    end = C.cast(event.data, C.POINTER(EndFile)).contents
                    assert end.reason == 0 and end.error >= 0, (filename, end.reason, end.error)
                    eof = True
                    break
            assert loaded and eof and codecs.get("video"), (filename, loaded, eof, codecs)
            result = {"file": filename, "status": "decoded-to-EOF", **codecs, "mpv": version}
            results.append(result)
            print(json.dumps(result), flush=True)
        finally:
            mpv.mpv_terminate_destroy(handle)
    (OUT / "codec-results.json").write_text(json.dumps(results, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()
