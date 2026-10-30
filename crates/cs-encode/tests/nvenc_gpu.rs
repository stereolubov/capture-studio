//! Прямой NVENC на реальном GPU NVIDIA: NV12-текстуры → пакеты H.264 с SPS/PPS.
//! Требует GPU NVIDIA, поэтому `#[ignore]` (§18.2); запуск: `cargo test -- --ignored`.
//! Поток пишется в `%TEMP%/cs-nvenc-test.h264` — проверка декодированием: `ffmpeg -i … -f null -`.

#![cfg(windows)]

use cs_encode::avc;
use cs_encode::nvenc::{NvencEncoder, NvencOptions, pipeline_depth};
use cs_encode::{RateControl, VideoSettings};
use cs_gpu::{ColorConverter, ColorParams, ConvertOptions, DriverKind, GpuDevice};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM;

fn run(opts: NvencOptions) -> Vec<cs_encode::Packet> {
    let gpu = GpuDevice::new(DriverKind::Hardware).expect("gpu");
    let s = VideoSettings {
        width: 1280,
        height: 720,
        fps_num: 60,
        fps_den: 1,
        bitrate_kbps: 4000,
        keyframe_interval_s: 1,
        rate_control: RateControl::Cbr,
    };
    let mut enc = NvencEncoder::new(&gpu.device, &s, &opts).expect("NVENC");
    let mut conv = {
        let _g = gpu.lock();
        let co = ConvertOptions { pool: 8 + pipeline_depth(&opts), ..Default::default() };
        ColorConverter::new(&gpu, 1280, 720, ColorParams { bt601: false, full_range: false }, co).expect("conv")
    };
    let mut out = Vec::new();
    for i in 0..120i64 {
        let shade = (i * 2) as u8;
        let px: Vec<u8> = (0..1280 * 720).flat_map(|_| [shade, 128, 255 - shade, 255]).collect();
        let tex = {
            let _g = gpu.lock();
            let canvas = gpu.create_texture(1280, 720, DXGI_FORMAT_R8G8B8A8_UNORM, Some(&px), false).expect("canvas");
            conv.convert(&gpu, &canvas.srv, (1280, 720))
        };
        enc.push(tex, i * s.frame_duration());
        enc.pump(&mut out).expect("pump");
    }
    enc.drain(&mut out).expect("drain");
    out
}

#[test]
#[ignore = "нужен GPU NVIDIA"]
fn nvenc_encodes_frames() {
    for opts in [
        NvencOptions::default(),
        NvencOptions { lookahead: true, low_latency: true, preset: 1, ..Default::default() },
        NvencOptions { b_frames: 2, ..Default::default() },
        NvencOptions { b_frames: 3, lookahead: true, preset: 7, ..Default::default() },
    ] {
        let out = run(opts);
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
        let reordered = out.windows(2).any(|w| w[1].pts < w[0].pts);
        assert_eq!(reordered, opts.b_frames > 0, "{opts:?}: перестановка кадров только с B-кадрами");
        let stream: Vec<u8> = out.iter().flat_map(|p| p.data.iter().copied()).collect();
        std::fs::write(std::env::temp_dir().join("cs-nvenc-test.h264"), stream).expect("файл");
    }
}
