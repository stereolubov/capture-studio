//! Чтение Matroska-файлов, записанных [`crate::mkv::MkvWriter`] (для ремукса в MP4).
//!
//! Поддерживается подмножество формата, которое пишет сам Capture Studio: дорожки с
//! CodecPrivate, кластеры с SimpleBlock без лейсинга. Файл, оборванный сбоем (Segment
//! неизвестного размера, недописанный кластер), читается до последнего целого блока.

use std::io::{self, Read, Seek, SeekFrom};

/// Дорожка.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackInfo {
    pub number: u64,
    /// 1 — видео, 2 — аудио.
    pub kind: u64,
    pub codec_id: String,
    pub codec_private: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub sample_rate: f64,
    pub channels: u32,
}

/// Кадр: дорожка, время (мс), признак ключевого, положение данных в файле.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub track: u64,
    pub ms: i64,
    pub keyframe: bool,
    pub offset: u64,
    pub size: u32,
}

fn read_vint<R: Read>(r: &mut R, keep_marker: bool) -> io::Result<(u64, usize, bool)> {
    let mut first = [0u8; 1];
    r.read_exact(&mut first)?;
    let len = first[0].leading_zeros() as usize + 1;
    if len > 8 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "неверный vint"));
    }
    // Маска значащих битов первого байта (для длины 8 — ноль; сдвиг в u32, чтобы не переполнить u8).
    let mask = (0xFFu32 >> len) as u8;
    let mut v = if keep_marker { u64::from(first[0]) } else { u64::from(first[0] & mask) };
    let mut all_ones = (first[0] & mask) == mask;
    for _ in 1..len {
        let mut b = [0u8; 1];
        r.read_exact(&mut b)?;
        all_ones &= b[0] == 0xFF;
        v = (v << 8) | u64::from(b[0]);
    }
    Ok((v, len, all_ones))
}

fn read_uint(data: &[u8]) -> u64 {
    data.iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b))
}

fn read_float(data: &[u8]) -> f64 {
    match data.len() {
        4 => f64::from(f32::from_be_bytes([data[0], data[1], data[2], data[3]])),
        8 => f64::from_be_bytes([data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7]]),
        _ => 0.0,
    }
}

/// Элементы внутри буфера (для небольших мастер-элементов: Tracks, TrackEntry).
fn children(mut data: &[u8]) -> Vec<(u32, &[u8])> {
    let mut out = Vec::new();
    while !data.is_empty() {
        let mut cur = io::Cursor::new(data);
        let Ok((id, _, _)) = read_vint(&mut cur, true) else { break };
        let Ok((size, _, _)) = read_vint(&mut cur, false) else { break };
        let start = cur.position() as usize;
        let end = (start + size as usize).min(data.len());
        out.push((id as u32, &data[start..end]));
        data = &data[end..];
    }
    out
}

fn parse_track(entry: &[u8]) -> TrackInfo {
    let mut t = TrackInfo {
        number: 0,
        kind: 0,
        codec_id: String::new(),
        codec_private: vec![],
        width: 0,
        height: 0,
        sample_rate: 0.0,
        channels: 0,
    };
    for (id, d) in children(entry) {
        match id {
            0xD7 => t.number = read_uint(d),
            0x83 => t.kind = read_uint(d),
            0x86 => t.codec_id = String::from_utf8_lossy(d).into_owned(),
            0x63A2 => t.codec_private = d.to_vec(),
            0xE0 => {
                for (vid, vd) in children(d) {
                    match vid {
                        0xB0 => t.width = read_uint(vd) as u32,
                        0xBA => t.height = read_uint(vd) as u32,
                        _ => {}
                    }
                }
            }
            0xE1 => {
                for (aid, ad) in children(d) {
                    match aid {
                        0xB5 => t.sample_rate = read_float(ad),
                        0x9F => t.channels = read_uint(ad) as u32,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    t
}

/// Результат разбора: дорожки и кадры в порядке файла.
pub struct MkvIndex {
    pub tracks: Vec<TrackInfo>,
    pub frames: Vec<Frame>,
}

/// Индексирует файл (данные кадров не читаются в память).
pub fn index<R: Read + Seek>(r: &mut R) -> io::Result<MkvIndex> {
    let len = r.seek(SeekFrom::End(0))?;
    r.seek(SeekFrom::Start(0))?;
    let mut tracks = Vec::new();
    let mut frames = Vec::new();
    // Обход: EBML header пропускается, Segment и Cluster — вход внутрь, остальное — по размеру.
    loop {
        let pos = r.stream_position()?;
        if pos >= len {
            break;
        }
        let Ok((id, _, _)) = read_vint(r, true) else { break };
        let Ok((size, _, unknown)) = read_vint(r, false) else { break };
        let data_start = r.stream_position()?;
        let end = if unknown { len } else { (data_start + size).min(len) };
        match id as u32 {
            0x18538067 | 0x1F43B675 => {
                // Segment / Cluster: читаем содержимое на следующих итерациях.
                if id as u32 == 0x1F43B675 {
                    let cluster_end = end;
                    let mut cluster_time = 0i64;
                    while r.stream_position()? < cluster_end {
                        let Ok((cid, _, _)) = read_vint(r, true) else { return Ok(MkvIndex { tracks, frames }) };
                        let Ok((csize, _, _)) = read_vint(r, false) else { return Ok(MkvIndex { tracks, frames }) };
                        let cstart = r.stream_position()?;
                        if cstart + csize > len {
                            return Ok(MkvIndex { tracks, frames }); // оборванный блок
                        }
                        match cid as u32 {
                            0xE7 => {
                                let mut b = vec![0u8; csize as usize];
                                r.read_exact(&mut b)?;
                                cluster_time = read_uint(&b) as i64;
                            }
                            0xA3 => {
                                let (track, tlen, _) = read_vint(r, false)?;
                                let mut hdr = [0u8; 3];
                                r.read_exact(&mut hdr)?;
                                let rel = i16::from_be_bytes([hdr[0], hdr[1]]);
                                let header = tlen as u64 + 3;
                                frames.push(Frame {
                                    track,
                                    ms: cluster_time + i64::from(rel),
                                    keyframe: hdr[2] & 0x80 != 0,
                                    offset: cstart + header,
                                    size: (csize - header) as u32,
                                });
                                r.seek(SeekFrom::Start(cstart + csize))?;
                            }
                            _ => {
                                r.seek(SeekFrom::Start(cstart + csize))?;
                            }
                        }
                    }
                }
            }
            0x1654AE6B => {
                let mut b = vec![0u8; (end - data_start) as usize];
                r.read_exact(&mut b)?;
                tracks = children(&b).into_iter().filter(|(i, _)| *i == 0xAE).map(|(_, d)| parse_track(d)).collect();
            }
            _ => {
                r.seek(SeekFrom::Start(end))?;
            }
        }
    }
    Ok(MkvIndex { tracks, frames })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mkv::{AudioCodec, AudioTrack, MkvWriter, VideoTrack};

    #[test]
    fn roundtrip_with_writer_including_truncated_file() {
        let dir = std::env::temp_dir().join(format!("cs-mkvr-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("t.mkv");
        let v = VideoTrack { width: 640, height: 360, codec_private: vec![1, 2, 3] };
        let a = AudioTrack {
            sample_rate: 48000,
            channels: 2,
            codec: AudioCodec::Aac,
            codec_private: vec![0x11, 0x90],
            name: "1".into(),
        };
        let mut w = MkvWriter::create(&path, &v, &[a]).expect("create");
        for i in 0..120i64 {
            w.write(1, &[i as u8; 10], i * 33, i % 30 == 0).expect("v");
            w.write(2, &[0xAA; 4], i * 33 + 5, true).expect("a");
        }
        w.finish(4000).expect("finish");

        let mut f = std::fs::File::open(&path).expect("open");
        let idx = index(&mut f).expect("index");
        assert_eq!(idx.tracks.len(), 2);
        assert_eq!(idx.tracks[0].codec_id, "V_MPEG4/ISO/AVC");
        assert_eq!((idx.tracks[0].width, idx.tracks[0].height), (640, 360));
        assert_eq!(idx.tracks[1].sample_rate, 48000.0);
        assert_eq!(idx.frames.len(), 240);
        let v: Vec<_> = idx.frames.iter().filter(|f| f.track == 1).collect();
        assert_eq!(v[31].ms, 31 * 33);
        assert!(v[30].keyframe && !v[31].keyframe);
        // Данные кадра по смещению.
        let mut buf = vec![0u8; v[7].size as usize];
        f.seek(SeekFrom::Start(v[7].offset)).expect("seek");
        f.read_exact(&mut buf).expect("read");
        assert_eq!(buf, vec![7u8; 10]);

        // Обрезанный «после сбоя» файл читается до последнего целого блока.
        let data = std::fs::read(&path).expect("read");
        let cut = &data[..data.len() * 2 / 3];
        let idx = index(&mut io::Cursor::new(cut)).expect("index cut");
        assert!(idx.frames.len() > 100 && idx.frames.len() < 240);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
