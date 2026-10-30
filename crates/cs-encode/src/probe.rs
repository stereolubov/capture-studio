//! Пробное кодирование (§9.1): каждый энкодер кодирует 10 кадров, рабочим считается тот, что
//! выдал по пакету на кадр, первый — ключевой с SPS/PPS. Результат — список доступности с
//! причинами (для настроек) и выбор «Авто»: первый рабочий по цепочке NVENC → AMF → QSV → MF →
//! OpenH264.

use crate::{EncodeError, EncoderChoice, EncoderOptions, RateControl, VideoEncoder, VideoSettings, avc};
use std::time::Instant;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_SUBRESOURCE_DATA, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, ID3D11Device,
    ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_NV12, DXGI_SAMPLE_DESC};

/// Кадров в пробе.
pub const FRAMES: usize = 10;
/// Порядок автовыбора (§9.1).
pub const ORDER: [EncoderChoice; 5] = [
    EncoderChoice::Nvenc,
    EncoderChoice::Amf,
    EncoderChoice::Qsv,
    EncoderChoice::MediaFoundation,
    EncoderChoice::Software,
];

/// Итог пробы одного энкодера.
#[derive(Debug, Clone, PartialEq)]
pub struct Probe {
    pub choice: EncoderChoice,
    /// Имя энкодера или причина недоступности.
    pub result: Result<String, String>,
}

/// Пробует все энкодеры по порядку автовыбора.
pub fn probe_all(device: &ID3D11Device, opts: &EncoderOptions) -> Vec<Probe> {
    ORDER
        .iter()
        .map(|&choice| {
            let result = probe(device, choice, opts).map_err(|e| match e {
                EncodeError::Unavailable(why) => why,
                other => other.to_string(),
            });
            match &result {
                Ok(name) => tracing::info!("проба энкодера {choice:?}: {name}"),
                Err(why) => tracing::info!("проба энкодера {choice:?}: недоступен ({why})"),
            }
            Probe { choice, result }
        })
        .collect()
}

/// Первый рабочий энкодер по порядку автовыбора.
pub fn pick(probes: &[Probe]) -> Option<EncoderChoice> {
    probes.iter().find(|p| p.result.is_ok()).map(|p| p.choice)
}

/// 10 кадров 1280×720 через энкодер `choice` (без замены другим).
pub fn probe(device: &ID3D11Device, choice: EncoderChoice, opts: &EncoderOptions) -> Result<String, EncodeError> {
    let s = VideoSettings {
        width: 1280,
        height: 720,
        fps_num: 30,
        fps_den: 1,
        bitrate_kbps: 3000,
        keyframe_interval_s: 1,
        rate_control: RateControl::Cbr,
    };
    let frames = frames(device, s.width, s.height)?;
    let started = Instant::now();
    let mut enc = VideoEncoder::open_one(device, &s, choice, opts)?;
    let mut out = Vec::new();
    // Кадры подаются с темпом записи: асинхронные MFT принимают вход по событиям и при подаче
    // залпом отбрасывали бы лишнее (очередь на 4 кадра).
    let interval = std::time::Duration::from_nanos(s.frame_duration() as u64 * 100);
    for (i, tex) in frames.into_iter().enumerate() {
        let due = started + interval * i as u32;
        enc.push(tex, i as i64 * s.frame_duration());
        loop {
            enc.pump(&mut out)?;
            if Instant::now() >= due + interval {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    enc.drain(&mut out)?;
    if enc.dropped() > 0 {
        return Err(EncodeError::Failed(format!("пробное кодирование: отброшено {} кадров", enc.dropped())));
    }
    check(&out)?;
    Ok(enc.name().to_owned())
}

/// Пакеты пробы правильные: по одному на кадр, первый — ключевой с SPS/PPS.
fn check(out: &[crate::Packet]) -> Result<(), EncodeError> {
    let fail = |why: &str| Err(EncodeError::Failed(format!("пробное кодирование: {why}")));
    if out.len() != FRAMES {
        return fail(&format!("{} пакетов на {FRAMES} кадров", out.len()));
    }
    if !out[0].keyframe || avc::find_parameter_sets(&out[0].data).is_none() {
        return fail("первый пакет не ключевой или без SPS/PPS");
    }
    Ok(())
}

/// NV12-кадры с разной яркостью (энкодер не должен видеть одинаковые кадры).
fn frames(device: &ID3D11Device, w: u32, h: u32) -> Result<Vec<ID3D11Texture2D>, EncodeError> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: w,
        Height: h,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_NV12,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let (w, h) = (w as usize, h as usize);
    (0..FRAMES)
        .map(|i| {
            let mut px = vec![128u8; w * h * 3 / 2];
            for (n, y) in px[..w * h].iter_mut().enumerate() {
                *y = ((n % w + n / w + i * 16) % 220 + 16) as u8;
            }
            let init =
                D3D11_SUBRESOURCE_DATA { pSysMem: px.as_ptr().cast(), SysMemPitch: w as u32, SysMemSlicePitch: 0 };
            let mut tex = None;
            // SAFETY: начальные данные NV12 — плоскость Y и за ней UV с тем же шагом; буфер живёт
            // во время вызова.
            unsafe { device.CreateTexture2D(&desc, Some(&init), Some(&mut tex))? };
            tex.ok_or_else(|| EncodeError::Failed("текстура пробы".into()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(choice: EncoderChoice, ok: bool) -> Probe {
        Probe { choice, result: if ok { Ok("x".into()) } else { Err("нет".into()) } }
    }

    #[test]
    fn pick_takes_first_working_in_order() {
        let list = [
            probe(EncoderChoice::Nvenc, false),
            probe(EncoderChoice::Amf, false),
            probe(EncoderChoice::Qsv, true),
            probe(EncoderChoice::MediaFoundation, true),
        ];
        assert_eq!(pick(&list), Some(EncoderChoice::Qsv));
        assert_eq!(pick(&list[..2]), None);
    }

    #[test]
    fn check_requires_packet_per_frame_and_headers() {
        let key = crate::Packet {
            data: vec![0, 0, 0, 1, 0x67, 0x64, 0, 0x1f, 0, 0, 0, 1, 0x68, 0xee, 0, 0, 0, 1, 0x65, 0x88],
            pts: 0,
            dts: 0,
            keyframe: true,
        };
        let delta = crate::Packet { data: vec![0, 0, 0, 1, 0x41, 0x9a], pts: 1, dts: 1, keyframe: false };
        let mut out = vec![key.clone()];
        out.extend(std::iter::repeat_n(delta.clone(), FRAMES - 1));
        assert!(check(&out).is_ok());
        assert!(check(&out[..FRAMES - 1]).is_err(), "пакетов меньше кадров");
        out[0] = delta;
        assert!(check(&out).is_err(), "первый не ключевой");
    }
}
