"""Декодирование с исходными PTS: без округления кадров и догадок о начале отрезка."""
import os
import re
import subprocess

import numpy as np


def decode(path, kind, start=None, duration=None, width=96, height=54):
    seek = [] if start is None else ["-ss", str(start), "-t", str(duration)]
    cmd = [os.path.join(os.environ.get("FFMPEG_DIR", ""), "ffmpeg"),
           "-hide_banner", "-nostats", "-v", "info", "-copyts", *seek, "-i", path]
    if kind == "video":
        cmd += ["-an", "-vf", f"scale={width}:{height},format=gray,showinfo",
                "-fps_mode", "passthrough", "-f", "rawvideo", "-"]
    else:
        cmd += ["-vn", "-af", "aresample=48000,aformat=sample_fmts=s16:channel_layouts=mono,ashowinfo",
                "-f", "s16le", "-"]
    result = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True)
    log = result.stderr.decode("utf-8", errors="replace")
    if kind == "video":
        base = re.search(r"config in time_base:\s*(\d+)/(\d+)", log)
        if base is None:
            raise ValueError("Нет временной базы декодированного видео")
        tb = int(base[1]) / int(base[2])
        pts = np.array([int(x) * tb for x in re.findall(r"\bn:\s*\d+\s+pts:\s*(-?\d+)", log)])
        frames = np.frombuffer(result.stdout, np.uint8).reshape(-1, height, width).astype(np.float32)
        if len(pts) != len(frames):
            raise ValueError("Число PTS не совпало с числом видеокадров")
        return frames, pts
    blocks = re.findall(r"\bn:\d+\s+pts:(-?\d+).*?rate:(\d+)\s+nb_samples:(\d+)", log)
    samples = np.frombuffer(result.stdout, np.int16).astype(np.float32)
    sizes = np.array([int(b[2]) for b in blocks])
    if not len(sizes) or sizes.sum() != len(samples):
        raise ValueError("Число отсчётов не совпало с журналом аудиоблоков")
    offsets = np.concatenate(([0], np.cumsum(sizes)))
    # Позиции пересечения порога переводятся через PTS соответствующего аудиоблока.
    levels = np.abs(samples)
    threshold = levels.max() * 0.3
    edges = np.flatnonzero((levels[1:] >= threshold) & (levels[:-1] < threshold)) + 1
    onset, last = [], -float("inf")
    for index in edges:
        block = int(np.searchsorted(offsets, index, side="right") - 1)
        pts, rate, _ = map(int, blocks[block])
        time = (pts + index - offsets[block]) / rate
        if time - last > 0.5:
            onset.append(time)
            last = time
    return np.array(onset)


def match_events(recorded, generated):
    """Сдвиг шкал по всем событиям, а не по одному (первое бывает запоздавшим)."""
    best, best_score = [], (0, -float("inf"))
    for gv in generated:
        shift = gv - recorded[0]
        for _ in range(3):
            nearest = np.abs((recorded + shift)[:, None] - generated).argmin(axis=1)
            residual = generated[nearest] - (recorded + shift)
            good = np.abs(residual) < 0.04
            if good.any():
                shift += np.median(residual[good])
        distance = np.abs(recorded + shift - generated[nearest])
        matches, used = [], set()
        for i in np.argsort(distance):
            j = int(nearest[i])
            if distance[i] <= 0.02 and j not in used:
                matches.append((int(i), j))
                used.add(j)
        score = (len(matches), -sum(distance[i] for i, _ in matches))
        if score > best_score:
            best, best_score = sorted(matches), score
    return best
