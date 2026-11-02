//! Синтетическая проверка задержки AAC MF без захвата экрана/аудиоустройства.
//! `cargo run -p cs-encode --example aac_timing -- <папка результатов>`

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;

    let dir = std::path::PathBuf::from(std::env::args_os().nth(1).ok_or("укажите папку результатов")?);
    std::fs::create_dir_all(&dir)?;
    let mut enc = cs_encode::mf_aac::MfAacEncoder::new(48_000, 160)?;
    let mut packets = Vec::new();
    let mut input = std::fs::File::create(dir.join("input.f32"))?;
    // Щелчки в 0,5/1,5/2,5 с: известная позиция на шкале входного PCM.
    for block in 0..300i64 {
        let pcm: Vec<f32> = (0..480)
            .flat_map(|i| {
                let frame = block * 480 + i;
                let phase = frame % 48_000 - 24_000;
                let v = if (0..480).contains(&phase) {
                    (phase as f32 * std::f32::consts::TAU / 48.0).sin() * 0.5
                } else {
                    0.0
                };
                [v, v]
            })
            .collect();
        for sample in &pcm {
            input.write_all(&sample.to_le_bytes())?;
        }
        enc.encode(&pcm, block * 100_000, &mut packets)?;
    }
    enc.drain(&mut packets)?;
    let mut aac = std::fs::File::create(dir.join("encoded.aac"))?;
    let mut pts = std::fs::File::create(dir.join("packets.csv"))?;
    writeln!(pts, "index,pts_100ns,bytes")?;
    for (i, p) in packets.iter().enumerate() {
        let len = p.data.len() + 7;
        if len >= 8192 {
            return Err("AAC-пакет не помещается в ADTS".into());
        }
        // ADTS: MPEG-4, AAC-LC, 48 кГц (index 3), stereo, без CRC.
        let header =
            [0xff, 0xf1, 0x4c, 0x80 | ((len >> 11) as u8), (len >> 3) as u8, ((len & 7) as u8) << 5 | 0x1f, 0xfc];
        aac.write_all(&header)?;
        aac.write_all(&p.data)?;
        writeln!(pts, "{i},{},{}", p.pts, p.data.len())?;
    }
    println!("AAC: {} пакетов; результаты: {}", packets.len(), dir.display());
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Проверка AAC MF доступна только в Windows");
}
