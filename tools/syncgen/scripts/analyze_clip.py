"""Замер рассинхрона A/V в записи: вспышки (рост яркости кадра) против щелчков (огибающая звука)."""
import json
import subprocess
import sys

import numpy as np

FF = r"C:\codecs\ffmpeg\bin"
path = sys.argv[1]

info = json.loads(subprocess.check_output([FF + r"\ffprobe.exe", "-v", "error", "-show_streams", "-of", "json", path]))
v = next(s for s in info["streams"] if s["codec_type"] == "video")
a = next(s for s in info["streams"] if s["codec_type"] == "audio")
num, den = map(int, v["avg_frame_rate"].split("/"))
fps = num / den
v_start = float(v.get("start_time", 0))
a_start = float(a.get("start_time", 0))

W, H = 64, 36
# fps=60 раскладывает кадры по меткам времени (пропуски дублируются): время = индекс / 60.
fps = 60.0
raw = subprocess.check_output([FF + r"\ffmpeg.exe", "-v", "error", "-i", path, "-an", "-vf", f"fps=60,scale={W}:{H},format=gray", "-f", "rawvideo", "-"])
frames = np.frombuffer(raw, np.uint8).reshape(-1, H, W).astype(np.float32)
# Пиксели, которые меняются сильнее всего, — окно с вспышками.
var = frames.std(axis=0)
mask = var >= np.percentile(var, 97)
lum = frames[:, mask].mean(axis=1)
thr = lum.min() + (lum.max() - lum.min()) * 0.5
v_on = [v_start + i / fps for i in range(1, len(lum)) if lum[i] >= thr and lum[i - 1] < thr]

pcm = subprocess.check_output([FF + r"\ffmpeg.exe", "-v", "error", "-i", path, "-vn", "-ac", "1", "-ar", "48000", "-f", "s16le", "-"])
s = np.abs(np.frombuffer(pcm, np.int16).astype(np.float32))
win = 48  # 1 мс
env = s[: len(s) // win * win].reshape(-1, win).max(axis=1)
athr = env.max() * 0.3
a_on = []
last = -1.0
for i in range(1, len(env)):
    t = a_start + i / 1000.0
    if env[i] >= athr and env[i - 1] < athr and t - last > 0.5:
        a_on.append(t)
        last = t

pairs = []
for tv in v_on:
    if not a_on:
        break
    ta = min(a_on, key=lambda x: abs(x - tv))
    if abs(ta - tv) < 0.4:
        pairs.append((ta - tv) * 1000.0)

print(f"fps={fps:.3f} v_start={v_start:.3f} a_start={a_start:.3f}")
print(f"вспышек={len(v_on)} щелчков={len(a_on)} пар={len(pairs)}")
if pairs:
    d = np.array(pairs)
    print(f"сдвиг аудио относительно видео, мс: среднее {d.mean():+.1f}, медиана {np.median(d):+.1f}, "
          f"мин {d.min():+.1f}, макс {d.max():+.1f}, разброс (σ) {d.std():.1f}")
    print("по событиям:", " ".join(f"{x:+.0f}" for x in d))
