"""Проверка результатов `cargo run -p cs-encode --example aac_timing -- <папка>`."""
import csv
import os
from pathlib import Path
import subprocess
import sys

import numpy as np

folder = Path(sys.argv[1])
pcm = subprocess.check_output([
    os.path.join(os.environ.get("FFMPEG_DIR", ""), "ffmpeg"), "-v", "error",
    "-i", str(folder / "encoded.aac"), "-f", "f32le", "-",
])
source = np.fromfile(folder / "input.f32", np.float32)[::2]
decoded = np.frombuffer(pcm, np.float32)[::2]
packets = list(csv.DictReader((folder / "packets.csv").open(encoding="utf-8")))
origin = int(packets[0]["pts_100ns"]) / 10_000_000
delays = []
for frame in [24_000, 72_000, 120_000]:
    reference = source[frame:frame + 480]
    segment = decoded[frame - 100:frame + 3000]
    delay = int(np.argmax(np.correlate(segment, reference, mode="valid"))) - 100
    delays.append(delay + origin * 48_000)
print(f"AAC timing: delay samples={delays}, milliseconds={np.array(delays) / 48}")
if max(abs(v) for v in delays) > 2:
    raise SystemExit("AAC output differs from input timeline by more than two samples")
