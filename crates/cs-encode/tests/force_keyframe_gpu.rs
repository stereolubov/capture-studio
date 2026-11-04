//! Real-driver recovery probe. GOP is ten seconds: IDRs at frame 20/40 must be explicitly requested.
#![cfg(windows)]
use cs_encode::{EncoderChoice, EncoderOptions, RateControl, VideoEncoder, VideoSettings, avc};
use cs_gpu::{ColorConverter, ColorParams, ConvertOptions, DriverKind, GpuDevice};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM;

fn check(choice: EncoderChoice, label: &str) {
    let gpu = GpuDevice::new(DriverKind::Hardware).expect("GPU");
    let settings = VideoSettings {
        width: 640,
        height: 360,
        fps_num: 60,
        fps_den: 1,
        bitrate_kbps: 1500,
        keyframe_interval_s: 10,
        rate_control: RateControl::Cbr,
    };
    let options = EncoderOptions { low_latency: true, b_frames: 0, lookahead: false, ..Default::default() };
    let mut encoder = VideoEncoder::open_one(&gpu.device, &settings, choice, &options).expect("requested encoder");
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
    let start = std::time::Instant::now();
    let frame_time = settings.frame_duration();
    let interval = std::time::Duration::from_nanos(frame_time as u64 * 100);
    let mut packets = Vec::new();
    for frame in 0..60i64 {
        let pixels: Vec<_> = (0..640 * 360).flat_map(|p| [frame as u8, (p % 256) as u8, 160, 255]).collect();
        let texture = {
            let _lock = gpu.lock();
            let canvas =
                gpu.create_texture(640, 360, DXGI_FORMAT_R8G8B8A8_UNORM, Some(&pixels), false).expect("canvas");
            converter.convert(&gpu, &canvas.srv, (640, 360))
        };
        if [20, 40].contains(&frame) {
            // Repeated identical requests before the next input must coalesce.
            encoder.request_keyframe();
            encoder.request_keyframe();
        }
        encoder.push(texture, frame * frame_time);
        let due = start + interval * (frame as u32 + 1);
        loop {
            encoder.pump(&mut packets).expect("pump");
            if std::time::Instant::now() >= due {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    encoder.drain(&mut packets).expect("drain");
    assert_eq!(packets.len(), 60, "one output per input");
    for frame in [20, 40] {
        let packet = packets.iter().find(|p| p.pts == frame * frame_time).expect("requested frame");
        assert!(avc::is_idr(&packet.data), "{label}: requested frame {frame} must be IDR");
        assert!(avc::find_parameter_sets(&packet.data).is_some(), "{label}: recovery IDR carries SPS/PPS");
        let before = packets.iter().find(|p| p.pts == (frame - 1) * frame_time).expect("previous frame");
        assert!(!avc::is_idr(&before.data), "not merely an all-intra stream");
    }
    if choice != EncoderChoice::Software {
        assert_eq!(cs_gpu::gpu_to_cpu_copies(), copies, "hardware path stays on GPU");
    }
    let tail: Vec<_> =
        packets.iter().filter(|p| p.pts >= 20 * frame_time).flat_map(|p| p.data.iter().copied()).collect();
    std::fs::write(std::env::temp_dir().join(format!("cs-forced-idr-{label}.h264")), tail).expect("recovery bitstream");
}

#[test]
#[ignore = "requires NVIDIA GPU"]
fn nvenc_forces_recovery_idr() {
    check(EncoderChoice::Nvenc, "nvenc");
}
#[test]
#[ignore = "requires a hardware Media Foundation H.264 encoder"]
fn mf_forces_recovery_idr() {
    check(EncoderChoice::MediaFoundation, "mf");
}
#[test]
#[ignore = "requires AMD GPU"]
fn amf_forces_recovery_idr() {
    check(EncoderChoice::Amf, "amf");
}
#[test]
#[ignore = "requires Intel GPU"]
fn qsv_forces_recovery_idr() {
    check(EncoderChoice::Qsv, "qsv");
}
#[test]
#[ignore = "requires a previously downloaded Cisco module in TEMP/cs-openh264-test"]
fn openh264_forces_recovery_idr() {
    cs_encode::openh264::set_dir(std::env::temp_dir().join("cs-openh264-test"));
    check(EncoderChoice::Software, "openh264");
}
