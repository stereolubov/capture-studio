//! AMF и QSV на реальных GPU AMD / Intel: NV12-текстуры → пакеты H.264 с SPS/PPS.
//! Требуют соответствующего GPU, поэтому `#[ignore]` (§18.2); запуск: `cargo test -- --ignored`.
//! Без такого GPU тест пишет «пропущено» и проходит. Устройство создаётся на адаптере нужного
//! производителя (второй GPU тоже подходит, например встроенная графика Intel рядом с NVIDIA).
//! Поток пишется в `%TEMP%/cs-<энкодер>-test.h264` — проверка: `ffmpeg -i … -f null -`.

#![cfg(windows)]

use cs_encode::{EncodeError, EncoderChoice, EncoderOptions, Packet, RateControl, VideoEncoder, VideoSettings};
use cs_encode::{avc, pipeline_depth};
use cs_gpu::{ColorConverter, ColorParams, ConvertOptions, GpuDevice};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM;

/// Высота не кратна 16 — проверка выравнивания у QSV.
const W: u32 = 1280;
const H: u32 = 714;

fn run(gpu: &GpuDevice, choice: EncoderChoice, opts: EncoderOptions) -> Result<Vec<Packet>, EncodeError> {
    let s = VideoSettings {
        width: W,
        height: H,
        fps_num: 60,
        fps_den: 1,
        bitrate_kbps: 4000,
        keyframe_interval_s: 1,
        rate_control: RateControl::Cbr,
    };
    let mut enc = match choice {
        EncoderChoice::Amf => VideoEncoder::Amf(Box::new(cs_encode::amf::AmfEncoder::new(&gpu.device, &s, &opts)?)),
        _ => VideoEncoder::Qsv(Box::new(cs_encode::qsv::QsvEncoder::new(&gpu.device, &s, &opts)?)),
    };
    let mut conv = {
        let _g = gpu.lock();
        let co = ConvertOptions { pool: 8 + pipeline_depth(choice, &opts), ..Default::default() };
        ColorConverter::new(gpu, W, H, ColorParams { bt601: false, full_range: false }, co).expect("conv")
    };
    let mut out = Vec::new();
    for i in 0..120i64 {
        let shade = (i * 2) as u8;
        let px: Vec<u8> = (0..W * H).flat_map(|_| [shade, 128, 255 - shade, 255]).collect();
        let tex = {
            let _g = gpu.lock();
            let canvas = gpu.create_texture(W, H, DXGI_FORMAT_R8G8B8A8_UNORM, Some(&px), false).expect("canvas");
            conv.convert(gpu, &canvas.srv, (W, H))
        };
        enc.push(tex, i * s.frame_duration());
        enc.pump(&mut out)?;
    }
    enc.drain(&mut out)?;
    Ok(out)
}

fn check(vendor: u32, choice: EncoderChoice, file: &str) {
    let Ok(gpu) = GpuDevice::on_vendor(vendor) else {
        eprintln!("{choice:?}: нет GPU {vendor:04x} — пропущено");
        return;
    };
    for opts in [
        EncoderOptions::default(),
        EncoderOptions { low_latency: true, preset: 1, ..Default::default() },
        EncoderOptions { b_frames: 2, ..Default::default() },
    ] {
        let out = match run(&gpu, choice, opts) {
            Ok(out) => out,
            Err(EncodeError::Unavailable(why)) => {
                eprintln!("{choice:?}: недоступен ({why}) — пропущено");
                return;
            }
            Err(e) => panic!("{choice:?} {opts:?}: {e}"),
        };
        assert_eq!(out.len(), 120, "{opts:?}: по пакету на кадр");
        assert!(out[0].keyframe, "первый пакет — ключевой");
        assert!(avc::find_parameter_sets(&out[0].data).is_some(), "SPS/PPS в первом ключевом кадре");
        let keys = out.iter().filter(|p| p.keyframe).count();
        assert_eq!(keys, 2, "GOP 1 с при 60 к/с → 2 ключевых за 120 кадров");
        assert!(out.windows(2).all(|w| w[1].dts > w[0].dts), "{opts:?}: DTS монотонны");
        assert!(out.iter().all(|p| p.dts <= p.pts), "{opts:?}: декодирование не позже показа");
        let mut pts: Vec<i64> = out.iter().map(|p| p.pts).collect();
        pts.sort_unstable();
        assert!(pts.windows(2).all(|w| w[1] > w[0]), "{opts:?}: все времена показа разные");
        let stream: Vec<u8> = out.iter().flat_map(|p| p.data.iter().copied()).collect();
        std::fs::write(std::env::temp_dir().join(file), stream).expect("файл");
    }
}

#[test]
#[ignore = "нужен GPU AMD"]
fn amf_encodes_frames() {
    check(0x1002, EncoderChoice::Amf, "cs-amf-test.h264");
}

#[test]
#[ignore = "нужен GPU Intel"]
fn qsv_encodes_frames() {
    check(0x8086, EncoderChoice::Qsv, "cs-qsv-test.h264");
}

/// Автовыбор на любом GPU даёт рабочий энкодер (на машине без AMD/Intel — NVENC или MF).
#[test]
#[ignore = "нужен GPU"]
fn auto_picks_working_encoder() {
    let gpu = GpuDevice::new(cs_gpu::DriverKind::Hardware).expect("gpu");
    let s = VideoSettings {
        width: W,
        height: H,
        fps_num: 60,
        fps_den: 1,
        bitrate_kbps: 4000,
        keyframe_interval_s: 1,
        rate_control: RateControl::Cbr,
    };
    let enc = VideoEncoder::open(&gpu.device, &s, EncoderChoice::Auto, &EncoderOptions::default()).expect("encoder");
    eprintln!("автовыбор: {}", enc.name());
}
