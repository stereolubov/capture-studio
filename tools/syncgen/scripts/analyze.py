"""Ошибка A/V-синхронизации записи относительно журнала эталонного генератора syncgen."""
import json
import sys

import numpy as np
from media_timing import decode, match_events

# Необязательно: отрезок записи [начало, длительность] в секундах — для длинных файлов.
path, events_path = sys.argv[1], sys.argv[2]
seg_start = float(sys.argv[3]) if len(sys.argv) > 3 else None
duration = float(sys.argv[4]) if seg_start is not None else None
events = [e for e in json.load(open(events_path, encoding="utf-8")) if e["video_actual"] is not None]

frames, video_pts = decode(path, "video", seg_start, duration)
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
v_on = np.array([video_pts[i] for i in range(1, len(lum)) if lum[i] >= thr and lum[i - 1] < thr])

a_on = decode(path, "audio", seg_start, duration)

# Сопоставление вспышек записи с событиями генератора: лучший постоянный сдвиг шкал.
g = np.array([e["video_actual"] for e in events])
if len(g) == 0 or len(v_on) == 0 or len(a_on) == 0:
    raise SystemExit("Недостаточно вспышек/щелчков или событий генератора для замера")

errors, measured, truth = [], [], []
for i, j in match_events(v_on, g):
    v = v_on[i]
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
