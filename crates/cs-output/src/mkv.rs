//! Мультиплексор Matroska (§10.1): H.264 + AAC, кластеры по ключевым кадрам, Cues в конце.
//!
//! Устойчив к сбоям: заголовок пишется сразу, каждый кластер дописывается целиком и
//! сбрасывается на диск; при аварийном завершении теряется только незаконченный кластер,
//! файл остаётся читаемым (Segment неизвестного размера). При штатном завершении дописываются
//! Cues, длительность и SeekHead.

use std::fs::File;
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

const EBML: u32 = 0x1A45DFA3;
const SEGMENT: u32 = 0x18538067;
const SEEK_HEAD: u32 = 0x114D9B74;
const SEEK: u32 = 0x4DBB;
const SEEK_ID: u32 = 0x53AB;
const SEEK_POSITION: u32 = 0x53AC;
const INFO: u32 = 0x1549A966;
const TRACKS: u32 = 0x1654AE6B;
const CLUSTER: u32 = 0x1F43B675;
const CUES: u32 = 0x1C53BB6B;
const VOID: u32 = 0xEC;

/// Резерв под SeekHead в начале сегмента.
const SEEK_RESERVE: usize = 128;
/// Максимальная длина кластера, мс (относительный таймкод блока — int16).
const MAX_CLUSTER_MS: i64 = 5_000;

fn id_bytes(id: u32) -> Vec<u8> {
    let b = id.to_be_bytes();
    let skip = b.iter().position(|&x| x != 0).unwrap_or(3);
    b[skip..].to_vec()
}

/// Длина элемента как EBML vint (минимальная).
fn size_vint(n: u64) -> Vec<u8> {
    for len in 1..=8u32 {
        let max = (1u64 << (7 * len)) - 2;
        if n <= max {
            let mut v = n | (1u64 << (7 * len));
            let mut out = vec![0u8; len as usize];
            for i in (0..len as usize).rev() {
                out[i] = (v & 0xFF) as u8;
                v >>= 8;
            }
            return out;
        }
    }
    // Больше 2^56 — недостижимо для наших файлов.
    vec![0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
}

fn element(id: u32, payload: &[u8]) -> Vec<u8> {
    let mut v = id_bytes(id);
    v.extend(size_vint(payload.len() as u64));
    v.extend_from_slice(payload);
    v
}

fn uint(id: u32, n: u64) -> Vec<u8> {
    let b = n.to_be_bytes();
    let skip = b.iter().position(|&x| x != 0).unwrap_or(7);
    element(id, &b[skip..])
}

fn float(id: u32, f: f64) -> Vec<u8> {
    element(id, &f.to_be_bytes())
}

fn string(id: u32, s: &str) -> Vec<u8> {
    element(id, s.as_bytes())
}

/// Void заданного полного размера (≥ 2 байт).
fn void(total: usize) -> Vec<u8> {
    // 1 байт ID + 8 байт размера (фиксированная длина, чтобы точно попасть в total).
    if total >= 9 {
        let mut v = vec![VOID as u8];
        let n = (total - 9) as u64;
        v.extend_from_slice(&[0x01]);
        v.extend_from_slice(&n.to_be_bytes()[1..]);
        v.resize(total, 0);
        v
    } else {
        let mut v = vec![VOID as u8, 0x80 | (total as u8 - 2)];
        v.resize(total, 0);
        v
    }
}

/// Видеодорожка H.264.
#[derive(Debug, Clone)]
pub struct VideoTrack {
    pub width: u32,
    pub height: u32,
    /// `AVCDecoderConfigurationRecord`.
    pub codec_private: Vec<u8>,
}

/// Кодек аудиодорожки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioCodec {
    #[default]
    Aac,
    /// Opus (только MKV): задержка кодера, нс — `CodecDelay`.
    Opus { delay_ns: u64 },
}

/// Аудиодорожка.
#[derive(Debug, Clone)]
pub struct AudioTrack {
    pub sample_rate: u32,
    pub channels: u8,
    pub codec: AudioCodec,
    /// AAC — `AudioSpecificConfig`, Opus — `OpusHead`.
    pub codec_private: Vec<u8>,
    pub name: String,
}

pub struct MkvWriter {
    out: BufWriter<File>,
    segment_size_pos: u64,
    segment_data_start: u64,
    seek_reserve_pos: u64,
    duration_pos: u64,
    info_pos: u64,
    tracks_pos: u64,
    cluster: Vec<u8>,
    cluster_start_ms: Option<i64>,
    cues: Vec<(i64, u64)>,
    last_ms: i64,
}

impl MkvWriter {
    pub fn create(path: &Path, video: &VideoTrack, audio: &[AudioTrack]) -> io::Result<Self> {
        let mut out = BufWriter::new(File::create(path)?);
        let header = element(
            EBML,
            &[
                uint(0x4286, 1),
                uint(0x42F7, 1),
                uint(0x42F2, 4),
                uint(0x42F3, 8),
                string(0x4282, "matroska"),
                uint(0x4287, 4),
                uint(0x4285, 2),
            ]
            .concat(),
        );
        out.write_all(&header)?;
        // Segment неизвестного размера (исправляется при финализации).
        out.write_all(&id_bytes(SEGMENT))?;
        let segment_size_pos = out.stream_position()?;
        out.write_all(&[0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF])?;
        let segment_data_start = out.stream_position()?;
        let seek_reserve_pos = segment_data_start;
        out.write_all(&void(SEEK_RESERVE))?;

        // Info: масштаб таймкодов 1 мс, длительность — заглушка, исправляется в конце.
        let info_pos = out.stream_position()? - segment_data_start;
        let app = concat!("Capture Studio ", env!("CARGO_PKG_VERSION"));
        let before_duration = [uint(0x2AD7B1, 1_000_000), string(0x4D80, app), string(0x5741, app)].concat();
        let duration = float(0x4489, 0.0);
        let info_payload = [before_duration.clone(), duration.clone()].concat();
        let mut info = id_bytes(INFO);
        info.extend(size_vint(info_payload.len() as u64));
        let info_header_len = info.len();
        info.extend(info_payload);
        let info_abs = out.stream_position()?;
        out.write_all(&info)?;
        // Позиция значения Duration: после ID (2) и размера (1) элемента Duration.
        let duration_pos = info_abs + info_header_len as u64 + before_duration.len() as u64 + 3;

        let tracks_pos = out.stream_position()? - segment_data_start;
        let mut entries = Vec::new();
        let v = [
            uint(0xD7, 1),
            uint(0x73C5, 1),
            uint(0x83, 1),
            uint(0x9C, 0),
            string(0x86, "V_MPEG4/ISO/AVC"),
            element(0x63A2, &video.codec_private),
            element(0xE0, &[uint(0xB0, u64::from(video.width)), uint(0xBA, u64::from(video.height))].concat()),
        ]
        .concat();
        entries.extend(element(0xAE, &v));
        for (i, a) in audio.iter().enumerate() {
            let n = (i + 2) as u64;
            let codec = match a.codec {
                AudioCodec::Aac => string(0x86, "A_AAC"),
                // Opus: задержка кодера и предкрутка 80 мс перед точкой перемотки (RFC 7845 / Matroska).
                AudioCodec::Opus { delay_ns } => {
                    [string(0x86, "A_OPUS"), uint(0x56AA, delay_ns), uint(0x56BB, 80_000_000)].concat()
                }
            };
            let e = [
                uint(0xD7, n),
                uint(0x73C5, n),
                uint(0x83, 2),
                uint(0x9C, 0),
                string(0x536E, &a.name),
                codec,
                element(0x63A2, &a.codec_private),
                element(0xE1, &[float(0xB5, f64::from(a.sample_rate)), uint(0x9F, u64::from(a.channels))].concat()),
            ]
            .concat();
            entries.extend(element(0xAE, &e));
        }
        out.write_all(&element(TRACKS, &entries))?;
        out.flush()?;
        Ok(Self {
            out,
            segment_size_pos,
            segment_data_start,
            seek_reserve_pos,
            duration_pos,
            info_pos,
            tracks_pos,
            cluster: Vec::new(),
            cluster_start_ms: None,
            cues: Vec::new(),
            last_ms: 0,
        })
    }

    fn flush_cluster(&mut self) -> io::Result<()> {
        let Some(start) = self.cluster_start_ms.take() else { return Ok(()) };
        let payload = [uint(0xE7, start.max(0) as u64), std::mem::take(&mut self.cluster)].concat();
        let pos = self.out.stream_position()? - self.segment_data_start;
        self.out.write_all(&element(CLUSTER, &payload))?;
        // Кластер дописан целиком — сбрасываем на диск (устойчивость к сбоям).
        self.out.flush()?;
        if self.cues.last().is_none_or(|(t, _)| *t != start) {
            self.cues.push((start, pos));
        }
        Ok(())
    }

    /// Пишет кадр дорожки `track` (1 — видео, 2.. — аудио); `ms` — время от начала записи.
    pub fn write(&mut self, track: u64, data: &[u8], ms: i64, keyframe: bool) -> io::Result<()> {
        let ms = ms.max(0);
        let new_cluster = match self.cluster_start_ms {
            None => true,
            Some(start) => (track == 1 && keyframe) || ms - start > MAX_CLUSTER_MS || ms < start,
        };
        if new_cluster {
            self.flush_cluster()?;
            self.cluster_start_ms = Some(ms);
        }
        let rel = (ms - self.cluster_start_ms.unwrap_or(ms)).clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16;
        // Номер дорожки в SimpleBlock — vint (для 1..126 — один байт 0x80|n).
        let mut block = vec![0x80 | track as u8];
        block.extend_from_slice(&rel.to_be_bytes());
        block.push(if keyframe { 0x80 } else { 0x00 });
        block.extend_from_slice(data);
        self.cluster.extend(element(0xA3, &block));
        self.last_ms = self.last_ms.max(ms);
        Ok(())
    }

    /// Финализация: последний кластер, Cues, длительность, SeekHead, размер сегмента.
    pub fn finish(mut self, duration_ms: i64) -> io::Result<()> {
        self.flush_cluster()?;
        let cues_pos = self.out.stream_position()? - self.segment_data_start;
        let mut points = Vec::new();
        for (t, pos) in &self.cues {
            let track_pos = element(0xB7, &[uint(0xF7, 1), uint(0xF1, *pos)].concat());
            points.extend(element(0xBB, &[uint(0xB3, *t as u64), track_pos].concat()));
        }
        self.out.write_all(&element(CUES, &points))?;
        let end = self.out.stream_position()?;

        let seek =
            |id: u32, pos: u64| element(SEEK, &[element(SEEK_ID, &id_bytes(id)), uint(SEEK_POSITION, pos)].concat());
        let head = element(
            SEEK_HEAD,
            &[seek(INFO, self.info_pos), seek(TRACKS, self.tracks_pos), seek(CUES, cues_pos)].concat(),
        );
        if head.len() + 2 <= SEEK_RESERVE {
            self.out.seek(SeekFrom::Start(self.seek_reserve_pos))?;
            self.out.write_all(&head)?;
            self.out.write_all(&void(SEEK_RESERVE - head.len()))?;
        }
        self.out.seek(SeekFrom::Start(self.duration_pos))?;
        self.out.write_all(&(duration_ms.max(self.last_ms) as f64).to_be_bytes())?;
        self.out.seek(SeekFrom::Start(self.segment_size_pos))?;
        let size = end - self.segment_data_start;
        let mut v = [0u8; 8];
        v[0] = 0x01;
        v[1..].copy_from_slice(&size.to_be_bytes()[1..]);
        self.out.write_all(&v)?;
        self.out.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vint_sizes() {
        assert_eq!(size_vint(0), vec![0x80]);
        assert_eq!(size_vint(126), vec![0xFE]);
        assert_eq!(size_vint(127), vec![0x40, 0x7F]);
        assert_eq!(size_vint(16382), vec![0x7F, 0xFE]);
        assert_eq!(id_bytes(SEGMENT), vec![0x18, 0x53, 0x80, 0x67]);
        assert_eq!(id_bytes(0xA3), vec![0xA3]);
        assert_eq!(void(SEEK_RESERVE).len(), SEEK_RESERVE);
        assert_eq!(void(5).len(), 5);
    }

    #[test]
    fn writes_parsable_file() {
        let dir = std::env::temp_dir().join(format!("cs-mkv-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("t.mkv");
        let v = VideoTrack {
            width: 1280,
            height: 720,
            codec_private: vec![1, 0x64, 0, 0x28, 0xFF, 0xE1, 0, 1, 0x67, 1, 0, 1, 0x68],
        };
        let a = AudioTrack {
            sample_rate: 48000,
            channels: 2,
            codec: AudioCodec::Aac,
            codec_private: vec![0x11, 0x90],
            name: "Дорожка 1".into(),
        };
        let mut w = MkvWriter::create(&path, &v, &[a]).expect("create");
        for i in 0..300i64 {
            w.write(1, &[0, 0, 0, 2, 0x65, 0x88], i * 16, i % 60 == 0).expect("video");
            w.write(2, &[0x21, 0x10], i * 16 + 3, true).expect("audio");
        }
        w.finish(4800).expect("finish");
        let data = std::fs::read(&path).expect("read");
        assert_eq!(&data[..4], &[0x1A, 0x45, 0xDF, 0xA3]);
        // Сегмент с известным размером = до конца файла.
        let seg = data.windows(4).position(|w| w == [0x18, 0x53, 0x80, 0x67]).expect("segment");
        let size_bytes = &data[seg + 4..seg + 12];
        assert_eq!(size_bytes[0], 0x01);
        let size = u64::from_be_bytes([
            0,
            size_bytes[1],
            size_bytes[2],
            size_bytes[3],
            size_bytes[4],
            size_bytes[5],
            size_bytes[6],
            size_bytes[7],
        ]);
        assert_eq!(seg as u64 + 12 + size, data.len() as u64);
        // Кластеров по числу ключевых кадров (каждые 60 кадров): 5; Cues присутствуют.
        let clusters = data.windows(4).filter(|w| *w == [0x1F, 0x43, 0xB6, 0x75]).count();
        assert_eq!(clusters, 5);
        assert!(data.windows(4).any(|w| w == [0x1C, 0x53, 0xBB, 0x6B]));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
