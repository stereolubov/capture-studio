//! Фрагментированный MP4 для записи (§10.1): безопасен при сбое и сразу открывается редакторами.
//!
//! В начале — `ftyp` + `moov` с `mvex` (описания дорожек известны с первого ключевого кадра), дальше
//! фрагменты `moof` + `mdat`. Новый фрагмент начинается на ключевом кадре видео, если с начала
//! текущего прошла секунда, — при сбое теряется не больше последнего фрагмента. Длительность
//! кадра известна с приходом следующего, поэтому по последнему кадру каждой дорожки держится
//! «хвост». Время: видео — по декодированию от первого кадра, сдвиги показа (B-кадры) — в `trun`,
//! показ первого кадра — в `elst`; звук — от первого показанного кадра видео (`tfdt`).

use crate::mkv::{AudioTrack, VideoTrack};
use crate::mp4::{MOVIE_TIMESCALE, VIDEO_TIMESCALE, avc1_entry, bx, full, matrix, mp4a_entry, stsd_with, u16b, u32b};
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

/// Наименьшая длительность фрагмента (по видео), 1 с.
const FRAGMENT_100NS: i64 = 10_000_000;
/// Длительность кадра AAC.
const AAC_FRAME: u32 = 1024;
/// Флаги кадров `trun`: независимый (ключевой) и зависимый.
const SYNC: u32 = 0x0200_0000;
const NON_SYNC: u32 = 0x0101_0000;

struct Sample {
    data: Vec<u8>,
    /// Время декодирования в единицах дорожки от начала её шкалы.
    decode: u64,
    duration: u32,
    composition: i32,
    sync: bool,
}

struct TrackState {
    timescale: u32,
    samples: Vec<Sample>,
    /// Последний кадр — длительность станет известна со следующим.
    tail: Option<Sample>,
}

impl TrackState {
    fn push(&mut self, s: Sample, fallback: u32) {
        if let Some(mut t) = self.tail.take() {
            t.duration = s.decode.saturating_sub(t.decode).max(1) as u32;
            self.samples.push(t);
        }
        let _ = fallback;
        self.tail = Some(s);
    }

    /// Закрывает «хвост» с длительностью предыдущего кадра (конец записи).
    fn close_tail(&mut self, fallback: u32) {
        if let Some(mut t) = self.tail.take() {
            t.duration = self.samples.last().map_or(fallback, |s| s.duration);
            self.samples.push(t);
        }
    }
}

pub struct Fmp4Writer {
    out: BufWriter<File>,
    tracks: Vec<TrackState>,
    sequence: u32,
    /// Время показа первого кадра видео (100 нс): ноль фильма.
    t0: i64,
    /// Время декодирования первого кадра видео (100 нс): ноль шкалы видео.
    dts0: i64,
    /// Время декодирования начала текущего фрагмента (100 нс).
    fragment_start: i64,
    frame_100ns: i64,
}

fn to_scale(v_100ns: i64, timescale: u32) -> i64 {
    (i128::from(v_100ns) * i128::from(timescale) / 10_000_000) as i64
}

impl Fmp4Writer {
    /// Создаёт файл и пишет `ftyp` + `moov`. `pts`/`dts` — первого (ключевого) кадра видео, 100 нс;
    /// `frame_100ns` — длительность кадра (для последнего кадра и на случай пустых дорожек).
    pub fn create(
        path: &Path,
        video: &VideoTrack,
        audio: &[AudioTrack],
        pts: i64,
        dts: i64,
        frame_100ns: i64,
    ) -> io::Result<Self> {
        let mut out = BufWriter::with_capacity(1 << 20, File::create(path)?);
        let ftyp = bx(b"ftyp", &[b"isom".as_slice(), &u32b(512), b"isom", b"iso6", b"avc1", b"mp41"].concat());
        out.write_all(&ftyp)?;
        out.write_all(&init_moov(video, audio, to_scale(pts - dts, VIDEO_TIMESCALE) as u32))?;
        out.flush()?;
        let mut tracks = vec![TrackState { timescale: VIDEO_TIMESCALE, samples: Vec::new(), tail: None }];
        for a in audio {
            tracks.push(TrackState { timescale: a.sample_rate.max(1), samples: Vec::new(), tail: None });
        }
        Ok(Self { out, tracks, sequence: 0, t0: pts, dts0: dts, fragment_start: dts, frame_100ns })
    }

    /// Кадр дорожки `track` (0 — видео, 1.. — аудио). Видео — в порядке декодирования,
    /// длины NAL уже 4-байтовые (`avc1`); времена — 100 нс от начала записи.
    pub fn write(&mut self, track: usize, data: &[u8], pts: i64, dts: i64, keyframe: bool) -> io::Result<()> {
        if track == 0 {
            if keyframe && dts - self.fragment_start >= FRAGMENT_100NS {
                // «Хвост» видео закрывается этим кадром, затем фрагмент — до него.
                let d = to_scale(dts - self.dts0, VIDEO_TIMESCALE).max(0) as u64;
                if let Some(mut t) = self.tracks[0].tail.take() {
                    t.duration = d.saturating_sub(t.decode).max(1) as u32;
                    self.tracks[0].samples.push(t);
                }
                self.flush_fragment()?;
                self.fragment_start = dts;
            }
            let sample = Sample {
                data: data.to_vec(),
                decode: to_scale(dts - self.dts0, VIDEO_TIMESCALE).max(0) as u64,
                duration: 0,
                composition: to_scale(pts - dts, VIDEO_TIMESCALE) as i32,
                sync: keyframe,
            };
            let fallback = to_scale(self.frame_100ns, VIDEO_TIMESCALE) as u32;
            self.tracks[0].push(sample, fallback);
        } else if let Some(t) = self.tracks.get_mut(track) {
            if pts < self.t0 {
                return Ok(());
            }
            let sample = Sample {
                data: data.to_vec(),
                decode: to_scale(pts - self.t0, t.timescale) as u64,
                duration: 0,
                composition: 0,
                sync: true,
            };
            t.push(sample, AAC_FRAME);
        }
        Ok(())
    }

    /// Дописывает последние кадры и закрывает файл.
    pub fn finish(mut self) -> io::Result<()> {
        let fallback = to_scale(self.frame_100ns, VIDEO_TIMESCALE) as u32;
        for (i, t) in self.tracks.iter_mut().enumerate() {
            t.close_tail(if i == 0 { fallback } else { AAC_FRAME });
        }
        self.flush_fragment()?;
        self.out.flush()
    }

    /// Пишет `moof` + `mdat` из готовых кадров всех дорожек.
    fn flush_fragment(&mut self) -> io::Result<()> {
        if self.tracks.iter().all(|t| t.samples.is_empty()) {
            return Ok(());
        }
        self.sequence += 1;
        // Сначала с нулевыми смещениями — чтобы узнать размер moof; затем с настоящими.
        let build = |offsets: &[u32]| -> Vec<u8> {
            let mut trafs = Vec::new();
            for (i, t) in self.tracks.iter().enumerate().filter(|(_, t)| !t.samples.is_empty()) {
                trafs.extend(traf(i as u32 + 1, t, offsets[i], i == 0));
            }
            bx(b"moof", &[full(b"mfhd", 0, 0, &u32b(self.sequence)), trafs].concat())
        };
        let zeros = vec![0u32; self.tracks.len()];
        let moof_len = build(&zeros).len() as u32;
        let mut offsets = vec![0u32; self.tracks.len()];
        let mut pos = moof_len + 8; // + заголовок mdat
        for (i, t) in self.tracks.iter().enumerate() {
            offsets[i] = pos;
            pos += t.samples.iter().map(|s| s.data.len() as u32).sum::<u32>();
        }
        let moof = build(&offsets);
        debug_assert_eq!(moof.len() as u32, moof_len);
        self.out.write_all(&moof)?;
        self.out.write_all(&u32b(pos - moof_len))?;
        self.out.write_all(b"mdat")?;
        for t in &mut self.tracks {
            for s in t.samples.drain(..) {
                self.out.write_all(&s.data)?;
            }
        }
        // Фрагмент целиком на диске: при сбое теряется только следующий.
        self.out.flush()
    }
}

/// `traf`: базовое время декодирования и `trun` с размерами, длительностями, флагами и (для видео)
/// сдвигами показа. Смещение данных — от начала `moof` (`default-base-is-moof`).
fn traf(id: u32, t: &TrackState, data_offset: u32, video: bool) -> Vec<u8> {
    let tfhd = full(b"tfhd", 0, 0x02_0000, &u32b(id));
    let base = t.samples.first().map_or(0, |s| s.decode);
    let tfdt = full(b"tfdt", 1, 0, &base.to_be_bytes());
    // data-offset | sample-duration | sample-size | sample-flags | composition-offset (видео).
    let flags = 0x001 | 0x100 | 0x200 | 0x400 | if video { 0x800 } else { 0 };
    let mut p = u32b(t.samples.len() as u32).to_vec();
    p.extend_from_slice(&u32b(data_offset));
    for s in &t.samples {
        p.extend_from_slice(&u32b(s.duration));
        p.extend_from_slice(&u32b(s.data.len() as u32));
        p.extend_from_slice(&u32b(if s.sync { SYNC } else { NON_SYNC }));
        if video {
            p.extend_from_slice(&s.composition.to_be_bytes());
        }
    }
    // Версия 1: знаковые сдвиги показа.
    let trun = full(b"trun", 1, flags, &p);
    bx(b"traf", &[tfhd, tfdt, trun].concat())
}

/// `moov` для фрагментированного файла: дорожки без кадров, `mvex` с `trex`.
fn init_moov(video: &VideoTrack, audio: &[AudioTrack], first_composition: u32) -> Vec<u8> {
    let n = 1 + audio.len() as u32;
    let mut mvhd = vec![0u8; 8];
    mvhd.extend_from_slice(&u32b(MOVIE_TIMESCALE));
    mvhd.extend_from_slice(&u32b(0));
    mvhd.extend_from_slice(&u32b(0x0001_0000));
    mvhd.extend_from_slice(&u16b(0x0100));
    mvhd.extend_from_slice(&[0u8; 10]);
    mvhd.extend(matrix());
    mvhd.extend_from_slice(&[0u8; 24]);
    mvhd.extend_from_slice(&u32b(n + 1));
    let mut moov = full(b"mvhd", 0, 0, &mvhd);
    moov.extend(init_trak(
        1,
        VIDEO_TIMESCALE,
        Some((video.width, video.height)),
        avc1_entry(video.width, video.height, &video.codec_private),
        first_composition,
    ));
    for (i, a) in audio.iter().enumerate() {
        let entry = mp4a_entry(a.sample_rate, u16::from(a.channels), &a.codec_private);
        moov.extend(init_trak(i as u32 + 2, a.sample_rate.max(1), None, entry, 0));
    }
    let mut mvex = Vec::new();
    for id in 1..=n {
        let mut trex = u32b(id).to_vec();
        trex.extend_from_slice(&u32b(1));
        trex.extend_from_slice(&[0u8; 12]);
        mvex.extend(full(b"trex", 0, 0, &trex));
    }
    moov.extend(bx(b"mvex", &mvex));
    bx(b"moov", &moov)
}

fn init_trak(id: u32, timescale: u32, size: Option<(u32, u32)>, entry: Vec<u8>, first_composition: u32) -> Vec<u8> {
    let mut tkhd = vec![0u8; 8];
    tkhd.extend_from_slice(&u32b(id));
    tkhd.extend_from_slice(&[0u8; 4]);
    tkhd.extend_from_slice(&u32b(0)); // длительность неизвестна
    tkhd.extend_from_slice(&[0u8; 8]);
    tkhd.extend_from_slice(&u16b(0));
    tkhd.extend_from_slice(&u16b(0));
    tkhd.extend_from_slice(&u16b(if size.is_none() { 0x0100 } else { 0 }));
    tkhd.extend_from_slice(&[0u8; 2]);
    tkhd.extend(matrix());
    let (w, h) = size.unwrap_or((0, 0));
    tkhd.extend_from_slice(&u32b(w << 16));
    tkhd.extend_from_slice(&u32b(h << 16));
    let mut mdhd = vec![0u8; 8];
    mdhd.extend_from_slice(&u32b(timescale));
    mdhd.extend_from_slice(&u32b(0));
    mdhd.extend_from_slice(&u16b(0x55C4)); // «und»
    mdhd.extend_from_slice(&u16b(0));
    let (handler, name) = if size.is_some() { (b"vide", "Video") } else { (b"soun", "Audio") };
    let mut hdlr = vec![0u8; 4];
    hdlr.extend_from_slice(handler);
    hdlr.extend_from_slice(&[0u8; 12]);
    hdlr.extend_from_slice(name.as_bytes());
    hdlr.push(0);
    let media_header = if size.is_some() { full(b"vmhd", 0, 1, &[0u8; 8]) } else { full(b"smhd", 0, 0, &[0u8; 4]) };
    let dref = full(b"dref", 0, 0, &[u32b(1).to_vec(), full(b"url ", 0, 1, &[])].concat());
    let empty = |kind: &[u8; 4]| full(kind, 0, 0, &u32b(0));
    let mut stsz = u32b(0).to_vec();
    stsz.extend_from_slice(&u32b(0));
    let stbl = [stsd_with(entry), empty(b"stts"), empty(b"stsc"), full(b"stsz", 0, 0, &stsz), empty(b"stco")].concat();
    let minf = bx(b"minf", &[media_header, bx(b"dinf", &dref), bx(b"stbl", &stbl)].concat());
    let mdia = bx(b"mdia", &[full(b"mdhd", 0, 0, &mdhd), full(b"hdlr", 0, 0, &hdlr), minf].concat());
    let mut parts = vec![full(b"tkhd", 0, 3, &tkhd)];
    // B-кадры: показ начинается со сдвига первого кадра на шкале декодирования.
    if first_composition > 0 {
        let mut elst = u32b(1).to_vec();
        elst.extend_from_slice(&u32b(0)); // на всю длительность
        elst.extend_from_slice(&u32b(first_composition));
        elst.extend_from_slice(&u32b(0x0001_0000));
        parts.push(bx(b"edts", &full(b"elst", 0, 0, &elst)));
    }
    parts.push(mdia);
    bx(b"trak", &parts.concat())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(data: &[u8], kind: &[u8; 4]) -> usize {
        data.windows(4).filter(|w| *w == kind).count()
    }

    /// 3 с видео 60 к/с с B-кадрами (I P B B …) и AAC со сдвигом 20 мс: структура фрагментов;
    /// при наличии `FFMPEG_DIR` — проверка ffprobe (число кадров, порядок времени показа).
    #[test]
    fn writes_fragments_with_b_frames() {
        let dir = std::env::temp_dir().join(format!("cs-fmp4-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("a.mp4");
        let avcc = vec![1, 0x64, 0, 0x28, 0xFF, 0xE1, 0, 4, 0x67, 0x64, 0, 0x28, 1, 0, 4, 0x68, 0xEE, 0x3C, 0x80];
        let video = VideoTrack { width: 320, height: 240, codec_private: avcc };
        let audio =
            [AudioTrack { sample_rate: 48_000, channels: 2, codec_private: vec![0x11, 0x90], name: "1".into() }];
        let fd = 166_667i64;
        // Порядок показа 0,1,2,… → порядок декодирования 0,3,1,2,6,4,5,…; DTS — сдвиг на кадр.
        let mut order = vec![0i64];
        for g in 0..59 {
            order.extend([3 * g + 3, 3 * g + 1, 3 * g + 2]);
        }
        let mut w = Fmp4Writer::create(&path, &video, &audio, 0, -fd, fd).expect("create");
        let mut a = 0i64;
        for (i, &k) in order.iter().enumerate() {
            let dts = (i as i64 - 1) * fd;
            let key = k % 60 == 0;
            w.write(0, &[0, 0, 0, 2, if key { 0x65 } else { 0x41 }, k as u8], k * fd, dts, key).expect("v");
            while 200_000 + a * 1024 * 10_000_000 / 48_000 <= dts {
                let t = 200_000 + a * 1024 * 10_000_000 / 48_000;
                w.write(1, &[0x21, 0x10, 0x04], t, t, true).expect("a");
                a += 1;
            }
        }
        w.finish().expect("finish");
        let data = std::fs::read(&path).expect("read");
        assert_eq!(&data[4..8], b"ftyp");
        assert_eq!(count(&data, b"mvex"), 1);
        assert!(count(&data, b"moof") >= 2, "фрагменты по ключевым кадрам: {}", count(&data, b"moof"));
        assert_eq!(count(&data, b"moof"), count(&data, b"mdat"));
        assert_eq!(count(&data, b"elst"), 1, "сдвиг показа видео");
        if let Some(ff) = std::env::var_os("FFMPEG_DIR") {
            let probe = std::path::Path::new(&ff).join("ffprobe");
            let out = std::process::Command::new(&probe)
                .args(["-v", "error", "-select_streams", "v:0", "-show_entries", "packet=pts", "-of", "csv=p=0"])
                .arg(&path)
                .output()
                .expect("ffprobe");
            let mut pts: Vec<i64> =
                String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| l.trim().parse().ok()).collect();
            assert_eq!(pts.len(), order.len(), "все кадры видео");
            pts.sort_unstable();
            assert_eq!(pts[0], 0, "показ начинается с нуля");
            assert!(pts.windows(2).all(|w| w[1] - w[0] == 1500), "ровно через кадр");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
