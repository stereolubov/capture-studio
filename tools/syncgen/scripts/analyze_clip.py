"""Замер A/V по вспышкам и щелчкам с исходными PTS, без журнала генератора."""
import sys

import numpy as np
from media_timing import decode

path = sys.argv[1]
frames, video_pts = decode(path, "video")
var = frames.std(axis=0)
mask = var >= np.percentile(var, 97)
lum = frames[:, mask].mean(axis=1)
thr = lum.min() + (lum.max() - lum.min()) * 0.5
v_on = [video_pts[i] for i in range(1, len(lum)) if lum[i] >= thr and lum[i - 1] < thr]
a_on = decode(path, "audio")

pairs = []
for tv in v_on:
    if len(a_on) == 0:
        break
    ta = min(a_on, key=lambda x: abs(x - tv))
    if abs(ta - tv) < 0.4:
        pairs.append((ta - tv) * 1000.0)

print("Время событий взято из PTS декодированных кадров и аудиоблоков")
print(f"вспышек={len(v_on)} щелчков={len(a_on)} пар={len(pairs)}")
if pairs:
    d = np.array(pairs)
    print(f"сдвиг аудио относительно видео, мс: среднее {d.mean():+.1f}, медиана {np.median(d):+.1f}, "
          f"мин {d.min():+.1f}, макс {d.max():+.1f}, разброс (σ) {d.std():.1f}")
    print("по событиям:", " ".join(f"{x:+.0f}" for x in d))
