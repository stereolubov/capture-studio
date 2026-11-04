"""Local RTMP interoperability probe; only synthetic video/audio leaves the process.

Run from the workspace root after `cargo build -p cs-output --example publish_flv`.
Requires ffmpeg/ffprobe in PATH. Artifacts stay under target/m3-probe.
"""
import json
import os
from pathlib import Path
import subprocess
import time


def run(*args):
    return subprocess.run(args, check=True, capture_output=True, text=True)


root = Path("target/m3-probe")
root.mkdir(parents=True, exist_ok=True)
source = root / "source.flv"
received = root / "received.mkv"
run("ffmpeg", "-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i",
    "testsrc2=size=640x360:rate=30", "-f", "lavfi", "-i",
    "sine=frequency=1000:sample_rate=48000", "-t", "3", "-c:v", "libx264",
    "-preset", "ultrafast", "-g", "30", "-bf", "2", "-pix_fmt", "yuv420p",
    "-c:a", "aac", "-b:a", "160k", "-f", "flv", "-y", str(source))
with (root / "receiver.log").open("w") as log:
    server = subprocess.Popen([
        "ffmpeg", "-hide_banner", "-loglevel", "info", "-listen", "1", "-timeout", "10",
        "-i", "rtmp://127.0.0.1:19450/live/test", "-c", "copy", "-y", str(received),
    ], stdout=log, stderr=log, creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
    try:
        time.sleep(0.4)
        probe = subprocess.run([
            "target/debug/examples/publish_flv.exe", str(source), "rtmp://127.0.0.1:19450/live",
        ], env={**os.environ, "CS_STREAM_KEY": "test"}, capture_output=True, text=True, timeout=20)
        if probe.returncode:
            raise RuntimeError(probe.stderr)
        server.wait(timeout=10)
    finally:
        if server.poll() is None:
            server.terminate()
            server.wait()


def counts(path):
    result = run("ffprobe", "-v", "error", "-count_packets", "-show_entries",
                 "stream=codec_name,nb_read_packets,width,height,sample_rate", "-of", "json", str(path))
    return json.loads(result.stdout)["streams"]


expected, actual = counts(source), counts(received)
assert expected == actual, (expected, actual)
run("ffmpeg", "-hide_banner", "-v", "error", "-xerror", "-i", str(received), "-f", "null", "-")
print(json.dumps({"result": "PASS", "streams": actual}, indent=2))
