//! MP4 (ISO BMFF) с `moov` перед `mdat` (faststart) и ремукс MKV → MP4 без перекодирования (§10.1).
//!
//! Данные кадров копируются как есть: в MKV видео уже хранится с 4-байтовыми длинами NAL
//! (формат `avc1`), аудио — сырые кадры AAC. Таблицы: `stts` (длительности), `stss` (ключевые),
//! `stsz`, `stsc` (1 кадр на чанк), `co64`. Смещение начала аудио относительно видео
//! передаётся списком правок `elst`.

use crate::mkv_read::{self, Frame, TrackInfo};
use std::fs::File;
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

const VIDEO_TIMESCALE: u32 = 90_000;
const MOVIE_TIMESCALE: u32 = 1_000;
const AAC_FRAME: u32 = 1024;

fn bx(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(payload.len() + 8);
    v.extend_from_slice(&((payload.len() + 8) as u32).to_be_bytes());
    v.extend_from_slice(kind);
    v.extend_from_slice(payload);
    v
}

fn full(kind: &[u8; 4], version: u8, flags: u32, payload: &[u8]) -> Vec<u8> {
    let mut p = vec![version, (flags >> 16) as u8, (flags >> 8) as u8, flags as u8];
    p.extend_from_slice(payload);
    bx(kind, &p)
}

fn u16b(v: u16) -> [u8; 2] {
    v.to_be_bytes()
}
fn u32b(v: u32) -> [u8; 4] {
    v.to_be_bytes()
}

const MATRIX: [u32; 9] = [0x0001_0000, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000];

fn matrix() -> Vec<u8> {
    MATRIX.iter().flat_map(|m| m.to_be_bytes()).collect()
}

/// Дорожка для записи в MP4.
struct Track<'a> {
    id: u32,
    info: &'a TrackInfo,
    frames: Vec<Frame>,
    timescale: u32,
    /// Длительности кадров в единицах `timescale`.
    durations: Vec<u32>,
    /// Начало дорожки относительно начала фильма, мс.
    start_ms: i64,
}

impl Track<'_> {
    fn duration(&self) -> u64 {
        self.durations.iter().map(|d| u64::from(*d)).sum()
    }

    fn is_video(&self) -> bool {
        self.info.kind == 1
    }
}

fn stts(durations: &[u32]) -> Vec<u8> {
    let mut runs: Vec<(u32, u32)> = Vec::new();
    for d in durations {
        match runs.last_mut() {
            Some((n, v)) if v == d => *n += 1,
            _ => runs.push((1, *d)),
        }
    }
    let mut p = u32b(runs.len() as u32).to_vec();
    for (n, d) in runs {
        p.extend_from_slice(&u32b(n));
        p.extend_from_slice(&u32b(d));
    }
    full(b"stts", 0, 0, &p)
}

fn esds(asc: &[u8], bitrate: u32) -> Vec<u8> {
    let desc = |tag: u8, body: &[u8]| -> Vec<u8> {
        let mut v = vec![tag, body.len() as u8];
        v.extend_from_slice(body);
        v
    };
    let dsi = desc(5, asc);
    let mut dcd = vec![0x40, 0x15, 0, 0, 0];
    dcd.extend_from_slice(&u32b(bitrate));
    dcd.extend_from_slice(&u32b(bitrate));
    dcd.extend(dsi);
    let mut es = vec![0, 1, 0];
    es.extend(desc(4, &dcd));
    es.extend(desc(6, &[2]));
    full(b"esds", 0, 0, &desc(3, &es))
}

fn stsd(t: &Track) -> Vec<u8> {
    let entry = if t.is_video() {
        let mut p = vec![0u8; 6];
        p.extend_from_slice(&u16b(1)); // data_reference_index
        p.extend_from_slice(&[0u8; 16]);
        p.extend_from_slice(&u16b(t.info.width as u16));
        p.extend_from_slice(&u16b(t.info.height as u16));
        p.extend_from_slice(&u32b(0x0048_0000));
        p.extend_from_slice(&u32b(0x0048_0000));
        p.extend_from_slice(&u32b(0));
        p.extend_from_slice(&u16b(1));
        p.extend_from_slice(&[0u8; 32]);
        p.extend_from_slice(&u16b(0x0018));
        p.extend_from_slice(&0xFFFFu16.to_be_bytes());
        p.extend(bx(b"avcC", &t.info.codec_private));
        bx(b"avc1", &p)
    } else {
        let mut p = vec![0u8; 6];
        p.extend_from_slice(&u16b(1));
        p.extend_from_slice(&[0u8; 8]);
        p.extend_from_slice(&u16b(t.info.channels.max(1) as u16));
        p.extend_from_slice(&u16b(16));
        p.extend_from_slice(&[0u8; 4]);
        p.extend_from_slice(&u32b((t.info.sample_rate as u32) << 16));
        p.extend(esds(&t.info.codec_private, 160_000));
        bx(b"mp4a", &p)
    };
    let mut p = u32b(1).to_vec();
    p.extend(entry);
    full(b"stsd", 0, 0, &p)
}

fn trak(t: &Track, offsets: &[u64], movie_duration_ms: u64) -> Vec<u8> {
    let media_dur = t.duration();
    let track_dur_ms = media_dur * u64::from(MOVIE_TIMESCALE) / u64::from(t.timescale);
    let mut tkhd = Vec::new();
    tkhd.extend_from_slice(&[0u8; 8]);
    tkhd.extend_from_slice(&u32b(t.id));
    tkhd.extend_from_slice(&[0u8; 4]);
    tkhd.extend_from_slice(&u32b((track_dur_ms + t.start_ms.max(0) as u64) as u32));
    tkhd.extend_from_slice(&[0u8; 8]);
    tkhd.extend_from_slice(&u16b(0));
    tkhd.extend_from_slice(&u16b(if t.is_video() { 0 } else { 1 }));
    tkhd.extend_from_slice(&u16b(if t.is_video() { 0 } else { 0x0100 }));
    tkhd.extend_from_slice(&u16b(0));
    tkhd.extend(matrix());
    let (w, h) = if t.is_video() { (t.info.width, t.info.height) } else { (0, 0) };
    tkhd.extend_from_slice(&u32b(w << 16));
    tkhd.extend_from_slice(&u32b(h << 16));

    let mut mdhd = vec![0u8; 8];
    mdhd.extend_from_slice(&u32b(t.timescale));
    mdhd.extend_from_slice(&u32b(media_dur as u32));
    mdhd.extend_from_slice(&u16b(0x55C4)); // 'und'
    mdhd.extend_from_slice(&u16b(0));

    let mut hdlr = vec![0u8; 4];
    hdlr.extend_from_slice(if t.is_video() { b"vide" } else { b"soun" });
    hdlr.extend_from_slice(&[0u8; 12]);
    hdlr.extend_from_slice(if t.is_video() { b"VideoHandler\0" } else { b"SoundHandler\0" });

    let media_header = if t.is_video() { full(b"vmhd", 0, 1, &[0u8; 8]) } else { full(b"smhd", 0, 0, &[0u8; 4]) };
    let dref = full(b"dref", 0, 0, &[u32b(1).to_vec(), full(b"url ", 0, 1, &[])].concat());
    let dinf = bx(b"dinf", &dref);

    let mut stsz = u32b(0).to_vec();
    stsz.extend_from_slice(&u32b(t.frames.len() as u32));
    for f in &t.frames {
        stsz.extend_from_slice(&u32b(f.size));
    }
    let stsc = full(b"stsc", 0, 0, &[u32b(1), u32b(1), u32b(1), u32b(1)].concat());
    let mut co64 = u32b(offsets.len() as u32).to_vec();
    for o in offsets {
        co64.extend_from_slice(&o.to_be_bytes());
    }
    let mut stbl = [stsd(t), stts(&t.durations)].concat();
    if t.is_video() {
        let keys: Vec<u32> =
            t.frames.iter().enumerate().filter(|(_, f)| f.keyframe).map(|(i, _)| i as u32 + 1).collect();
        let mut p = u32b(keys.len() as u32).to_vec();
        for k in keys {
            p.extend_from_slice(&u32b(k));
        }
        stbl.extend(full(b"stss", 0, 0, &p));
    }
    stbl.extend(full(b"stsz", 0, 0, &stsz));
    stbl.extend(stsc);
    stbl.extend(full(b"co64", 0, 0, &co64));

    let minf = bx(b"minf", &[media_header, dinf, bx(b"stbl", &stbl)].concat());
    let mdia = bx(b"mdia", &[full(b"mdhd", 0, 0, &mdhd), full(b"hdlr", 0, 0, &hdlr), minf].concat());
    let mut parts = vec![full(b"tkhd", 0, 3, &tkhd)];
    // Аудио началось позже видео: пустая правка на время задержки (синхронизация, §8.1).
    if t.start_ms > 0 {
        let mut elst = u32b(2).to_vec();
        elst.extend_from_slice(&u32b(t.start_ms as u32));
        elst.extend_from_slice(&(-1i32).to_be_bytes());
        elst.extend_from_slice(&u32b(0x0001_0000));
        elst.extend_from_slice(&u32b(track_dur_ms.min(movie_duration_ms) as u32));
        elst.extend_from_slice(&0i32.to_be_bytes());
        elst.extend_from_slice(&u32b(0x0001_0000));
        parts.push(bx(b"edts", &full(b"elst", 0, 0, &elst)));
    }
    parts.push(mdia);
    bx(b"trak", &parts.concat())
}

fn build_moov(tracks: &[Track], offsets: &[Vec<u64>]) -> Vec<u8> {
    let movie_ms = tracks
        .iter()
        .map(|t| t.duration() * u64::from(MOVIE_TIMESCALE) / u64::from(t.timescale) + t.start_ms.max(0) as u64)
        .max()
        .unwrap_or(0);
    let mut mvhd = vec![0u8; 8];
    mvhd.extend_from_slice(&u32b(MOVIE_TIMESCALE));
    mvhd.extend_from_slice(&u32b(movie_ms as u32));
    mvhd.extend_from_slice(&u32b(0x0001_0000));
    mvhd.extend_from_slice(&u16b(0x0100));
    mvhd.extend_from_slice(&[0u8; 10]);
    mvhd.extend(matrix());
    mvhd.extend_from_slice(&[0u8; 24]);
    mvhd.extend_from_slice(&u32b(tracks.len() as u32 + 1));
    let mut moov = full(b"mvhd", 0, 0, &mvhd);
    for (t, o) in tracks.iter().zip(offsets) {
        moov.extend(trak(t, o, movie_ms));
    }
    bx(b"moov", &moov)
}

/// Длительности видеокадров в 1/90000 с. Метки MKV округлены до миллисекунд (при 60 fps кадры
/// идут через 16/17 мс), поэтому при ровном темпе берётся точная длительность кадра по частоте,
/// иначе (пропуски, переменная частота) — разница соседних меток.
fn video_durations(frames: &[Frame]) -> Vec<u32> {
    let deltas: Vec<i64> = frames.windows(2).map(|w| (w[1].ms - w[0].ms).max(1)).collect();
    if let (Some(first), Some(last)) = (frames.first(), frames.last())
        && frames.len() > 2
    {
        let fps = (frames.len() - 1) as f64 * 1000.0 / (last.ms - first.ms).max(1) as f64;
        const STANDARD: [f64; 9] = [24.0, 25.0, 30.0, 48.0, 50.0, 60.0, 120.0, 30000.0 / 1001.0, 60000.0 / 1001.0];
        if let Some(nominal) = STANDARD.into_iter().find(|f| (f - fps).abs() / f < 0.01) {
            let frame_ms = 1000.0 / nominal;
            if deltas.iter().all(|d| (*d as f64 - frame_ms).abs() <= 1.5) {
                let dur = (f64::from(VIDEO_TIMESCALE) / nominal).round() as u32;
                return vec![dur; frames.len()];
            }
        }
    }
    let mut d: Vec<u32> = deltas.iter().map(|d| (*d * 90) as u32).collect();
    d.push(d.last().copied().unwrap_or(1500));
    d
}

/// Ремукс MKV → MP4. `progress(0..1)` вызывается по ходу копирования данных.
pub fn remux(src: &Path, dst: &Path, mut progress: impl FnMut(f32)) -> io::Result<()> {
    let mut input = File::open(src)?;
    let idx = mkv_read::index(&mut input)?;
    let mut tracks: Vec<Track> = Vec::new();
    for info in idx.tracks.iter().filter(|t| t.codec_id == "V_MPEG4/ISO/AVC" || t.codec_id == "A_AAC") {
        let frames: Vec<Frame> = idx.frames.iter().filter(|f| f.track == info.number).copied().collect();
        if frames.is_empty() {
            continue;
        }
        let is_video = info.kind == 1;
        let timescale = if is_video { VIDEO_TIMESCALE } else { (info.sample_rate as u32).max(1) };
        let durations: Vec<u32> = if is_video { video_durations(&frames) } else { vec![AAC_FRAME; frames.len()] };
        tracks.push(Track { id: tracks.len() as u32 + 1, info, start_ms: frames[0].ms, frames, timescale, durations });
    }
    if !tracks.iter().any(|t| t.is_video()) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "в MKV нет видеодорожки H.264"));
    }
    // Начало фильма — первый кадр видео.
    let base = tracks.iter().find(|t| t.is_video()).map_or(0, |t| t.start_ms);
    for t in &mut tracks {
        t.start_ms -= base;
    }

    // Порядок данных в mdat — как в MKV (уже перемежён по времени).
    let order: Vec<(usize, usize)> = {
        let mut counters = vec![0usize; tracks.len()];
        let mut v = Vec::with_capacity(idx.frames.len());
        for f in &idx.frames {
            if let Some(ti) = tracks.iter().position(|t| t.info.number == f.track) {
                v.push((ti, counters[ti]));
                counters[ti] += 1;
            }
        }
        v
    };
    let ftyp = bx(b"ftyp", &[b"isom".as_slice(), &u32b(512), b"isom", b"iso2", b"avc1", b"mp41"].concat());
    let zero: Vec<Vec<u64>> = tracks.iter().map(|t| vec![0u64; t.frames.len()]).collect();
    let moov_len = build_moov(&tracks, &zero).len() as u64;
    let mdat_start = ftyp.len() as u64 + moov_len + 16; // mdat с 64-битным размером
    let mut offsets: Vec<Vec<u64>> = zero;
    let mut pos = mdat_start;
    for &(ti, fi) in &order {
        offsets[ti][fi] = pos;
        pos += u64::from(tracks[ti].frames[fi].size);
    }
    let moov = build_moov(&tracks, &offsets);
    debug_assert_eq!(moov.len() as u64, moov_len);

    let tmp = dst.with_extension("mp4.part");
    let mut out = BufWriter::with_capacity(1 << 20, File::create(&tmp)?);
    out.write_all(&ftyp)?;
    out.write_all(&moov)?;
    out.write_all(&u32b(1))?;
    out.write_all(b"mdat")?;
    out.write_all(&(pos - mdat_start + 16).to_be_bytes())?;
    let total = order.len().max(1);
    let mut buf = Vec::new();
    for (n, &(ti, fi)) in order.iter().enumerate() {
        let f = tracks[ti].frames[fi];
        buf.resize(f.size as usize, 0);
        input.seek(SeekFrom::Start(f.offset))?;
        input.read_exact(&mut buf)?;
        out.write_all(&buf)?;
        if n % 2000 == 0 {
            progress(n as f32 / total as f32);
        }
    }
    out.flush()?;
    drop(out);
    std::fs::rename(&tmp, dst)?;
    progress(1.0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mkv::{AudioTrack, MkvWriter, VideoTrack};

    #[test]
    fn steady_60fps_gets_exact_durations() {
        let frames: Vec<Frame> =
            (0..120).map(|i| Frame { track: 1, ms: i * 1000 / 60, keyframe: false, offset: 0, size: 1 }).collect();
        assert!(video_durations(&frames).iter().all(|d| *d == 1500));
        // С пропуском кадра — фактические разницы меток.
        let mut gap = frames.clone();
        gap.remove(50);
        let d = video_durations(&gap);
        assert!(d.iter().any(|x| *x > 2900));
    }

    #[test]
    fn stts_runs() {
        let b = stts(&[1500, 1500, 1500, 1501, 1500]);
        // 3 серии: (3×1500), (1×1501), (1×1500).
        assert_eq!(u32::from_be_bytes([b[12], b[13], b[14], b[15]]), 3);
    }

    /// Ремукс синтетического MKV; если доступен ffprobe — проверка структуры MP4.
    #[test]
    fn remux_structure() {
        let dir = std::env::temp_dir().join(format!("cs-mp4-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let mkv = dir.join("a.mkv");
        let mp4 = dir.join("a.mp4");
        let avcc = vec![1, 0x64, 0, 0x28, 0xFF, 0xE1, 0, 4, 0x67, 0x64, 0, 0x28, 1, 0, 4, 0x68, 0xEE, 0x3C, 0x80];
        let mut w = MkvWriter::create(
            &mkv,
            &VideoTrack { width: 320, height: 240, codec_private: avcc },
            &[AudioTrack { sample_rate: 48000, channels: 2, codec_private: vec![0x11, 0x90], name: "1".into() }],
        )
        .expect("mkv");
        for i in 0..90i64 {
            w.write(1, &[0, 0, 0, 2, 0x65, i as u8], i * 1000 / 30, i % 30 == 0).expect("v");
        }
        for i in 0..140i64 {
            w.write(2, &[0x21, 0x10, 0x04], 20 + i * 1024 * 1000 / 48000, true).expect("a");
        }
        w.finish(3000).expect("finish");
        remux(&mkv, &mp4, |_| {}).expect("remux");
        let data = std::fs::read(&mp4).expect("read");
        assert_eq!(&data[4..8], b"ftyp");
        let moov_at = data.windows(4).position(|w| w == b"moov").expect("moov");
        let mdat_at = data.windows(4).position(|w| w == b"mdat").expect("mdat");
        assert!(moov_at < mdat_at, "faststart: moov перед mdat");
        assert!(data.windows(4).any(|w| w == b"elst"), "аудио начинается на 20 мс позже видео");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
