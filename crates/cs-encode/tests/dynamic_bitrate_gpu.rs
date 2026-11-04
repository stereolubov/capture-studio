//! Six-second local driver test: CBR 4000 -> 2000 -> 4000 kbps without restarting the session.
#![cfg(windows)]
use cs_encode::{EncoderChoice, EncoderOptions, RateControl, VideoEncoder, VideoSettings};
use cs_gpu::{ColorConverter, ColorParams, ConvertOptions, DriverKind, GpuDevice};
use std::time::{Duration, Instant};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM;

fn check(choice: EncoderChoice, label: &str) {
    let gpu = GpuDevice::new(DriverKind::Hardware).expect("GPU");
    let settings = VideoSettings {
        width: 640,
        height: 360,
        fps_num: 30,
        fps_den: 1,
        bitrate_kbps: 4000,
        keyframe_interval_s: 2,
        rate_control: RateControl::Cbr,
    };
    let options = EncoderOptions { low_latency: true, b_frames: 0, lookahead: false, ..Default::default() };
    let mut encoder = VideoEncoder::open_one(&gpu.device, &settings, choice, &options).expect("encoder");
    let mut converter = {
        let _lock = gpu.lock();
        ColorConverter::new(
            &gpu,
            640,
            360,
            ColorParams { bt601: false, full_range: false },
            ConvertOptions { pool: 16, ..Default::default() },
        )
        .expect("converter")
    };
    let copies = cs_gpu::gpu_to_cpu_copies();
    let mut packets = Vec::new();
    let mut random = 0x12345678u32;
    let start = Instant::now();
    let frame_time = settings.frame_duration();
    for frame in 0..180i64 {
        if frame == 60 {
            encoder.set_bitrate(2000, &mut packets).expect("reduce bitrate");
        }
        if frame == 120 {
            encoder.set_bitrate(4000, &mut packets).expect("restore bitrate");
        }
        // High-entropy moving input makes the rate controller, rather than content simplicity, the limit.
        let pixels: Vec<_> = (0..640 * 360 * 4)
            .map(|_| {
                random ^= random << 13;
                random ^= random >> 17;
                random ^= random << 5;
                random as u8
            })
            .collect();
        let texture = {
            let _lock = gpu.lock();
            let canvas =
                gpu.create_texture(640, 360, DXGI_FORMAT_R8G8B8A8_UNORM, Some(&pixels), false).expect("canvas");
            converter.convert(&gpu, &canvas.srv, (640, 360))
        };
        encoder.push(texture, frame * frame_time);
        let due = start + Duration::from_nanos(frame_time as u64 * 100) * (frame as u32 + 1);
        loop {
            encoder.pump(&mut packets).expect("pump");
            if Instant::now() >= due {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    encoder.drain(&mut packets).expect("drain");
    if choice == EncoderChoice::Software {
        assert!(!packets.is_empty());
        assert_eq!(packets.len() as u64 + encoder.dropped(), 180, "software skips are counted");
    } else {
        assert_eq!(packets.len(), 180, "no frames lost during bitrate change");
    }
    assert!(packets.windows(2).all(|p| p[1].dts > p[0].dts), "timestamps remain monotonic");
    let rates: Vec<_> = [30..60, 90..120, 150..180]
        .into_iter()
        .map(|range| {
            packets.iter().filter(|p| range.contains(&(p.pts / frame_time))).map(|p| p.data.len() as u64).sum::<u64>()
                * 8
                / 1000
        })
        .collect();
    eprintln!("{label}: measured last second of each phase, kbps {rates:?}");
    assert!(rates[1] * 100 < rates[0] * 85, "reduction must change actual output rate");
    assert!(rates[2] * 100 > rates[1] * 115, "restoration must change actual output rate");
    if choice != EncoderChoice::Software {
        assert_eq!(cs_gpu::gpu_to_cpu_copies(), copies);
    }
    let data: Vec<_> = packets.iter().flat_map(|p| p.data.iter().copied()).collect();
    std::fs::write(std::env::temp_dir().join(format!("cs-dynamic-{label}.h264")), data).expect("bitstream");
}
#[test]
#[ignore = "requires NVIDIA GPU"]
fn nvenc_changes_live_bitrate() {
    check(EncoderChoice::Nvenc, "nvenc");
}
#[test]
#[ignore = "requires hardware MF encoder supporting dynamic bitrate"]
fn mf_changes_live_bitrate() {
    check(EncoderChoice::MediaFoundation, "mf");
}
#[test]
#[ignore = "requires AMD GPU"]
fn amf_changes_live_bitrate() {
    check(EncoderChoice::Amf, "amf");
}
#[test]
#[ignore = "requires Intel GPU"]
fn qsv_changes_live_bitrate() {
    check(EncoderChoice::Qsv, "qsv");
}
#[test]
#[ignore = "requires installed Cisco DLL in TEMP/cs-openh264-test"]
fn openh264_changes_live_bitrate() {
    cs_encode::openh264::set_dir(std::env::temp_dir().join("cs-openh264-test"));
    check(EncoderChoice::Software, "openh264");
}
