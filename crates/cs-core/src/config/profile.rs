//! Профиль `profiles/<имя>/profile.toml`: вывод, кодирование, трансляция, видео, аудио, горячие клавиши.

use super::Versioned;
use super::migrate::Migration;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncoderKind {
    #[default]
    Auto,
    Nvenc,
    Amf,
    Qsv,
    MediaFoundation,
    Software,
}

impl EncoderKind {
    /// Короткое имя для строки параметров под превью.
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Авто",
            Self::Nvenc => "NVENC H.264",
            Self::Amf => "AMF H.264",
            Self::Qsv => "QSV H.264",
            Self::MediaFoundation => "MF H.264",
            Self::Software => "OpenH264",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Codec {
    #[default]
    H264,
    Hevc,
    Av1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateControl {
    #[default]
    Cbr,
    Vbr,
    Cqp,
}

impl RateControl {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cbr => "CBR",
            Self::Vbr => "VBR",
            Self::Cqp => "CQP",
        }
    }
}

/// Аудиокодек записи (§9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioCodec {
    #[default]
    Aac,
    /// Только в MKV; в форматах с MP4 пишется AAC.
    Opus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordFormat {
    /// MKV с авто-ремуксом в MP4 после остановки (по умолчанию).
    #[default]
    MkvRemuxMp4,
    Mkv,
    Mp4,
    FragmentedMp4,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputSettings {
    pub encoder: EncoderKind,
    pub codec: Codec,
    pub rate_control: RateControl,
    pub bitrate_kbps: u32,
    /// Для CQP/CQ.
    pub cq_level: u32,
    pub keyframe_interval_s: u32,
    /// Пресет энкодера, `1..=7` (P1–P7 у NVENC).
    pub preset: u8,
    pub record_format: RecordFormat,
    pub h264_profile: String,
    pub b_frames: u8,
    pub lookahead: bool,
    /// Отдельный энкодер для записи (иначе общий со стримом).
    pub separate_record_encoder: bool,
    /// Битовая маска дорожек записи (1–6).
    pub record_tracks: u8,
    pub audio_bitrate_kbps: u32,
    pub audio_codec: AudioCodec,
    /// Удалять MKV после успешного ремукса в MP4 (§10.1, по умолчанию выкл.).
    pub delete_mkv_after_remux: bool,
}

impl Default for OutputSettings {
    fn default() -> Self {
        Self {
            encoder: EncoderKind::Auto,
            codec: Codec::H264,
            rate_control: RateControl::Cbr,
            bitrate_kbps: 6000,
            cq_level: 20,
            keyframe_interval_s: 2,
            preset: 5,
            record_format: RecordFormat::MkvRemuxMp4,
            h264_profile: "high".into(),
            b_frames: 2,
            lookahead: false,
            separate_record_encoder: false,
            record_tracks: 0b1,
            audio_bitrate_kbps: 160,
            audio_codec: AudioCodec::Aac,
            delete_mkv_after_remux: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StreamSettings {
    /// Ключ сервиса из `services.toml` (`twitch`, `youtube`, …) или `custom`.
    pub service: String,
    pub server: String,
    /// Ссылка на запись в Windows Credential Manager (сам ключ в TOML не хранится, §12.3).
    pub key_ref: Option<String>,
    pub reconnect: bool,
    pub dynamic_bitrate: bool,
}

impl Default for StreamSettings {
    fn default() -> Self {
        Self { service: "twitch".into(), server: "auto".into(), key_ref: None, reconnect: true, dynamic_bitrate: false }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScaleFilter {
    Bilinear,
    #[default]
    Bicubic,
    Lanczos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorSpace {
    #[default]
    Bt709,
    Bt601,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorRange {
    #[default]
    Limited,
    Full,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoSettings {
    /// `None` — по основному монитору.
    pub canvas: Option<(u32, u32)>,
    pub output: Option<(u32, u32)>,
    pub scale_filter: ScaleFilter,
    /// Частота кадров как дробь (числитель, знаменатель): 60/1, 60000/1001.
    pub fps: (u32, u32),
    pub color_space: ColorSpace,
    pub color_range: ColorRange,
    /// Яркость белого SDR для тонмаппинга HDR (нит).
    pub sdr_white_nits: u32,
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            canvas: None,
            output: None,
            scale_filter: ScaleFilter::Bicubic,
            fps: (60, 1),
            color_space: ColorSpace::Bt709,
            color_range: ColorRange::Limited,
            sdr_white_nits: 300,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    pub sample_rate: u32,
    /// `None` — устройство по умолчанию (со следованием за его сменой).
    pub desktop_device: Option<String>,
    pub mic_device: Option<String>,
    pub monitoring_device: Option<String>,
    /// Каналы микшера: «Рабочий стол», «Микрофон» (§8.1).
    pub channels: Vec<AudioChannelSettings>,
}

/// Источник звука канала.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioChannelKind {
    /// Loopback устройства вывода.
    Desktop,
    /// Устройство ввода.
    Mic,
    /// Звук приложения (process loopback, Windows 10 2004+); exe — в `app`.
    App,
}

/// Сохраняемые настройки канала микшера (§8.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioChannelSettings {
    pub name: String,
    pub kind: AudioChannelKind,
    pub fader_db: f32,
    #[serde(default)]
    pub muted: bool,
    /// Битовая маска дорожек записи 1–6.
    #[serde(default = "default_tracks")]
    pub tracks: u8,
    /// Смещение синхронизации, мс (−950…+2000).
    #[serde(default)]
    pub sync_offset_ms: i32,
    /// Цепочка фильтров (§8.3), по порядку применения.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<AudioFilter>,
    /// Имя exe для канала «Звук приложения».
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
}

/// Фильтр канала (§8.3). Цепочка применяется по порядку, до фейдера.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AudioFilter {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(flatten)]
    pub kind: AudioFilterKind,
}

fn default_enabled() -> bool {
    true
}

/// Тип фильтра с параметрами.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AudioFilterKind {
    /// Усиление, дБ (−30…+30).
    Gain { db: f32 },
    /// Шумоподавление RNNoise (без параметров; задержка 10 мс учитывается в синхронизации).
    NoiseSuppress,
    /// Шумовой гейт: пороги открытия/закрытия (дБ), атака, удержание, спад (мс).
    Gate { open_db: f32, close_db: f32, attack_ms: f32, hold_ms: f32, release_ms: f32 },
    /// Компрессор: порог (дБ), степень сжатия (n:1), атака, спад (мс), компенсация усиления (дБ).
    Compressor { threshold_db: f32, ratio: f32, attack_ms: f32, release_ms: f32, makeup_db: f32 },
    /// Лимитер: потолок (дБ), спад (мс).
    Limiter { ceiling_db: f32, release_ms: f32 },
}

impl AudioFilterKind {
    /// Все типы с параметрами по умолчанию — для меню «Добавить фильтр».
    pub const ALL: [AudioFilterKind; 5] = [
        AudioFilterKind::Gain { db: 0.0 },
        AudioFilterKind::NoiseSuppress,
        AudioFilterKind::Gate { open_db: -26.0, close_db: -32.0, attack_ms: 25.0, hold_ms: 200.0, release_ms: 150.0 },
        AudioFilterKind::Compressor {
            threshold_db: -18.0,
            ratio: 4.0,
            attack_ms: 6.0,
            release_ms: 60.0,
            makeup_db: 0.0,
        },
        AudioFilterKind::Limiter { ceiling_db: -1.0, release_ms: 60.0 },
    ];

    /// Ключ локализации названия.
    pub fn name_key(&self) -> &'static str {
        match self {
            AudioFilterKind::Gain { .. } => "filter-gain",
            AudioFilterKind::NoiseSuppress => "filter-noise",
            AudioFilterKind::Gate { .. } => "filter-gate",
            AudioFilterKind::Compressor { .. } => "filter-compressor",
            AudioFilterKind::Limiter { .. } => "filter-limiter",
        }
    }
}

fn default_tracks() -> u8 {
    0b1
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            desktop_device: None,
            mic_device: None,
            monitoring_device: None,
            channels: vec![
                AudioChannelSettings {
                    name: "Рабочий стол".into(),
                    kind: AudioChannelKind::Desktop,
                    fader_db: 0.0,
                    muted: false,
                    tracks: 0b1,
                    sync_offset_ms: 0,
                    filters: Vec::new(),
                    app: None,
                },
                AudioChannelSettings {
                    name: "Микрофон".into(),
                    kind: AudioChannelKind::Mic,
                    fader_db: 0.0,
                    muted: false,
                    tracks: 0b1,
                    sync_offset_ms: 0,
                    filters: Vec::new(),
                    app: None,
                },
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub output: OutputSettings,
    pub stream: StreamSettings,
    pub video: VideoSettings,
    pub audio: AudioSettings,
    /// Действие → сочетание (`scene.1` → `F1`).
    pub hotkeys: BTreeMap<String, String>,
}

impl Default for Profile {
    fn default() -> Self {
        let hotkeys = (1..=4).map(|i| (format!("scene.{i}"), format!("F{i}"))).collect();
        Self {
            output: OutputSettings::default(),
            stream: StreamSettings::default(),
            video: VideoSettings::default(),
            audio: AudioSettings::default(),
            hotkeys,
        }
    }
}

impl Versioned for Profile {
    const SCHEMA_VERSION: u32 = 1;
    fn migrations() -> &'static [Migration] {
        // v0 профилей не существовало: файл без версии читается как есть.
        &[|_| {}]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::from_toml_str;

    #[test]
    fn roundtrip() {
        let p = Profile::default();
        let mut t = toml::Table::try_from(&p).expect("ser");
        t.insert("schema_version".into(), toml::Value::Integer(1));
        let text = toml::to_string(&t).expect("text");
        let back: Profile = from_toml_str(&text).expect("de");
        assert_eq!(back, p);
    }

    #[test]
    fn filters_roundtrip() {
        let mut p = Profile::default();
        p.audio.channels[1].filters = vec![
            AudioFilter { enabled: true, kind: AudioFilterKind::Gain { db: 6.5 } },
            AudioFilter { enabled: false, kind: AudioFilterKind::Gain { db: -3.0 } },
            AudioFilter { enabled: true, kind: AudioFilterKind::NoiseSuppress },
            AudioFilter { enabled: true, kind: AudioFilterKind::ALL[2] },
            AudioFilter { enabled: true, kind: AudioFilterKind::ALL[3] },
            AudioFilter { enabled: true, kind: AudioFilterKind::ALL[4] },
        ];
        let mut t = toml::Table::try_from(&p).expect("ser");
        t.insert("schema_version".into(), toml::Value::Integer(1));
        let text = toml::to_string(&t).expect("text");
        assert!(text.contains("type = \"gain\""), "{text}");
        let back: Profile = from_toml_str(&text).expect("de");
        assert_eq!(back, p);
    }

    #[test]
    fn defaults_match_spec() {
        let p = Profile::default();
        assert_eq!(p.output.bitrate_kbps, 6000);
        assert_eq!(p.output.keyframe_interval_s, 2);
        assert_eq!(p.output.audio_bitrate_kbps, 160);
        assert_eq!(p.video.sdr_white_nits, 300);
        assert_eq!(p.hotkeys.get("scene.1").map(String::as_str), Some("F1"));
    }
}
