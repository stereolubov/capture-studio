//! Разбор потока H.264: Annex B ↔ длины NAL, SPS/PPS → `AVCDecoderConfigurationRecord` (avcC).

/// NAL-единицы потока Annex B (без стартовых кодов).
pub fn annexb_nals(data: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= data.len() {
        if data[i] == 0
            && data[i + 1] == 0
            && (data[i + 2] == 1 || (i + 4 <= data.len() && data[i + 2] == 0 && data[i + 3] == 1))
        {
            let len = if data[i + 2] == 1 { 3 } else { 4 };
            starts.push((i, i + len));
            i += len;
        } else {
            i += 1;
        }
    }
    let mut out = Vec::with_capacity(starts.len());
    for (k, &(_, payload)) in starts.iter().enumerate() {
        let end = starts.get(k + 1).map_or(data.len(), |&(s, _)| s);
        // Хвостовые нули относятся к следующему стартовому коду.
        let mut e = end;
        while e > payload && data[e - 1] == 0 && k + 1 < starts.len() {
            e -= 1;
        }
        if e > payload {
            out.push(&data[payload..e]);
        }
    }
    out
}

/// Тип NAL-единицы.
pub fn nal_type(nal: &[u8]) -> u8 {
    nal.first().map_or(0, |b| b & 0x1F)
}

/// Annex B → NAL с 4-байтовыми длинами (формат MKV/MP4).
pub fn annexb_to_length_prefixed(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 16);
    for nal in annexb_nals(data) {
        out.extend_from_slice(&(nal.len() as u32).to_be_bytes());
        out.extend_from_slice(nal);
    }
    out
}

/// SPS и PPS из ключевого кадра (Annex B).
pub fn find_parameter_sets(data: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let nals = annexb_nals(data);
    let sps = nals.iter().find(|n| nal_type(n) == 7)?;
    let pps = nals.iter().find(|n| nal_type(n) == 8)?;
    Some((sps.to_vec(), pps.to_vec()))
}

/// `AVCDecoderConfigurationRecord` (CodecPrivate в MKV, `avcC` в MP4).
pub fn avcc(sps: &[u8], pps: &[u8]) -> Vec<u8> {
    let mut v = vec![
        1,
        sps.get(1).copied().unwrap_or(100),
        sps.get(2).copied().unwrap_or(0),
        sps.get(3).copied().unwrap_or(40),
        0xFF, // 4-байтовые длины NAL
        0xE1, // 1 SPS
    ];
    v.extend_from_slice(&(sps.len() as u16).to_be_bytes());
    v.extend_from_slice(sps);
    v.push(1);
    v.extend_from_slice(&(pps.len() as u16).to_be_bytes());
    v.extend_from_slice(pps);
    v
}

/// `AudioSpecificConfig` для AAC-LC.
pub fn aac_asc(sample_rate: u32, channels: u8) -> [u8; 2] {
    const RATES: [u32; 13] = [96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350];
    let idx = RATES.iter().position(|r| *r == sample_rate).unwrap_or(3) as u8;
    let object = 2u8; // AAC LC
    [(object << 3) | (idx >> 1), ((idx & 1) << 7) | (channels << 3)]
}

#[cfg(test)]
mod tests {
    use super::*;

    const STREAM: &[u8] = &[
        0, 0, 0, 1, 0x67, 0x64, 0x00, 0x28, 0xAA, // SPS
        0, 0, 0, 1, 0x68, 0xEE, 0x3C, 0x80, // PPS
        0, 0, 1, 0x65, 0x88, 0x84, 0x00, // IDR
    ];

    #[test]
    fn parses_annexb() {
        let n = annexb_nals(STREAM);
        assert_eq!(n.len(), 3);
        assert_eq!(nal_type(n[0]), 7);
        assert_eq!(nal_type(n[1]), 8);
        assert_eq!(nal_type(n[2]), 5);
        assert_eq!(n[2], &[0x65, 0x88, 0x84, 0x00]);
    }

    #[test]
    fn length_prefixed_and_avcc() {
        let lp = annexb_to_length_prefixed(STREAM);
        assert_eq!(&lp[..4], &[0, 0, 0, 5]);
        assert_eq!(lp.len(), 4 + 5 + 4 + 4 + 4 + 4);
        let (sps, pps) = find_parameter_sets(STREAM).expect("sps/pps");
        let c = avcc(&sps, &pps);
        assert_eq!(&c[..6], &[1, 0x64, 0x00, 0x28, 0xFF, 0xE1]);
        assert_eq!(c.len(), 6 + 2 + 5 + 1 + 2 + 4);
    }

    #[test]
    fn asc_48k_stereo() {
        assert_eq!(aac_asc(48000, 2), [0x11, 0x90]);
        assert_eq!(aac_asc(44100, 2), [0x12, 0x10]);
    }
}
