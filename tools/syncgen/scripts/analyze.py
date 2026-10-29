"""Ошибка A/V-синхронизации записи относительно журнала эталонного генератора syncgen."""
import json
import os
import subprocess
import sys

import numpy as np

# Папка с ffmpeg/ffprobe: переменная FFMPEG_DIR, иначе — из PATH.
FF = os.environ.get("FFMPEG_DIR", "")
# Необязательно: отрезок записи [начало, длительность] в секундах — для длинных файлов.
path, events_path = sys.argv[1], sys.argv[2]
seg_start = float(sys.argv[3]) if len(sys.argv) > 3 else None
seek = ["-ss", sys.argv[3], "-t", sys.argv[4]] if seg_start is not None else []
events = [e for e in json.load(open(events_path, encoding="utf-8")) if e["video_actual"] is not None]

info = json.loads(subprocess.check_output([os.path.join(FF, "ffprobe"), "-v", "error", "-show_streams", "-of", "json", path]))
a = next(s for s in info["streams"] if s["codec_type"] == "audio")
a_start = float(a.get("start_time", 0))
# При разборе отрезка оба потока декодируются с одной точки: начало отсчёта у них общее.
t0 = seg_start if seg_start is not None else 0.0
if seg_start is not None:
    a_start = seg_start

W, H, FPS = 96, 54, 60.0
raw = subprocess.check_output([os.path.join(FF, "ffmpeg"), "-v", "error", *seek, "-i", path, "-an", "-vf", f"fps=60,scale={W}:{H},format=gray", "-f", "rawvideo", "-"])
frames = np.frombuffer(raw, np.uint8).reshape(-1, H, W).astype(np.float32)
# Окно генератора почти всё время чёрное и изредка белое: такие пиксели не путаются с другим
# движением на экране (видео, смена картинок). Если окна не нашлось — самые изменчивые пиксели.
mask = (np.median(frames, axis=0) < 40) & (frames.max(axis=0) > 200)


def most_flashing(frames, cand):
    """Окно генератора вспыхивает каждую секунду, а тёмные окна пользователя (терминал,
    мессенджер) белеют лишь изредка — при прокрутке или переключении. Оставляем кандидатов,
    у которых вспышек не меньше половины от самого частого."""
    if cand.sum() == 0:
        return cand
    series = frames[:, cand] > 128
    edges = (series[1:] & ~series[:-1]).sum(axis=0)
    out = np.zeros_like(cand)
    out[cand] = edges >= edges.max() / 2
    return out


mask = most_flashing(frames, mask)
if mask.sum() < 4:
    var = frames.std(axis=0)
    mask = var >= np.percentile(var, 98)
lum = frames[:, mask].mean(axis=1)
thr = lum.min() + (lum.max() - lum.min()) * 0.5
v_on = np.array([t0 + i / FPS for i in range(1, len(lum)) if lum[i] >= thr and lum[i - 1] < thr])

pcm = subprocess.check_output([os.path.join(FF, "ffmpeg"), "-v", "error", *seek, "-i", path, "-vn", "-ac", "1", "-ar", "48000", "-f", "s16le", "-"])
s = np.abs(np.frombuffer(pcm, np.int16).astype(np.float32))
env = s[: len(s) // 48 * 48].reshape(-1, 48).max(axis=1)
athr = env.max() * 0.3
a_on, last = [], -1.0
for i in range(1, len(env)):
    t = a_start + i / 1000.0
    if env[i] >= athr and env[i - 1] < athr and t - last > 0.5:
        a_on.append(t)
        last = t
a_on = np.array(a_on)

# Сопоставление вспышек записи с событиями генератора: лучший постоянный сдвиг шкал.
g = np.array([e["video_actual"] for e in events])
best = (0, 0.0)
for gv in g:
    shift = gv - v_on[0]
    n = sum(np.min(np.abs(g - (v + shift))) < 0.02 for v in v_on)
    if n > best[0]:
        best = (n, shift)
shift = best[1]

errors, measured, truth = [], [], []
for v in v_on:
    j = int(np.argmin(np.abs(g - (v + shift))))
    if abs(g[j] - (v + shift)) > 0.02 or len(a_on) == 0:
        continue
    ta = a_on[int(np.argmin(np.abs(a_on - v)))]
    if abs(ta - v) > 0.3:
        continue
    m = (ta - v) * 1000.0
    t = (events[j]["audio"] - events[j]["video_actual"]) * 1000.0
    measured.append(m)
    truth.append(t)
    errors.append(m - t)

print(f"вспышек в записи={len(v_on)}, щелчков={len(a_on)}, сопоставлено с генератором={len(errors)}")
if errors:
    e = np.array(errors)
    print(f"эталонный сдвиг (генератор): среднее {np.mean(truth):+.1f} мс")
    print(f"ошибка записи (звук − видео): среднее {e.mean():+.1f} мс, медиана {np.median(e):+.1f}, "
          f"мин {e.min():+.1f}, макс {e.max():+.1f}, σ {e.std():.1f}")
    print("по событиям:", " ".join(f"{x:+.0f}" for x in e))
