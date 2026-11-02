"""Интеграционная проверка PTS; нужны FFmpeg и numpy, захвата экрана нет."""
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

import numpy as np
from media_timing import decode, match_events


class MediaTimingTest(unittest.TestCase):
    def test_first_event_does_not_anchor_whole_run(self):
        generated = np.cumsum([1 + (i % 4) * 0.05 for i in range(27)])
        generated[0] += 0.014
        recorded = generated + 100 + np.tile([-0.008, 0.008, 0], 9)
        self.assertEqual(match_events(recorded, generated), [(i, i) for i in range(27)])

    def test_fractional_pts_survive_segment_seek(self):
        with tempfile.TemporaryDirectory(prefix="cs-timing-") as tmp:
            path = str(Path(tmp) / "timing.mkv")
            subprocess.run([
                os.path.join(os.environ.get("FFMPEG_DIR", ""), "ffmpeg"), "-v", "error",
                "-f", "lavfi", "-i", "color=black:s=192x108:r=60:d=3",
                "-f", "lavfi", "-i",
                r"aevalsrc=if(between(mod(t\,1)\,0.5\,0.51)\,0.5*sin(2*PI*1000*t)\,0):s=48000:d=3",
                "-filter_complex",
                r"[0:v]drawbox=x=0:y=0:w=iw:h=ih:color=white:t=fill:enable=between(mod(t\,1)\,0.5\,0.549),settb=1/1000,setpts=PTS+7[v];[1:a]asetpts=PTS+0.003/TB[a]",
                "-map", "[v]", "-map", "[a]", "-c:v", "ffv1", "-enc_time_base:v", "1:1000",
                "-fps_mode", "passthrough", "-c:a", "pcm_s16le", path,
            ], check=True)
            for start, duration, expected in [(None, None, [0.507, 1.507, 2.507]), (1.2, 1.5, [1.507, 2.507])]:
                frames, pts = decode(path, "video", start, duration)
                bright = frames.mean(axis=(1, 2)) > 128
                flashes = pts[1:][bright[1:] & ~bright[:-1]]
                clicks = decode(path, "audio", start, duration)
                np.testing.assert_allclose(flashes, expected, atol=0.0001)
                np.testing.assert_allclose(clicks - flashes, -0.0039375, atol=0.0001)


if __name__ == "__main__":
    unittest.main()
