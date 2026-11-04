//! H.264/AAC message bodies shared by FLV and RTMP (Adobe FLV specification, Annex E).
//! Inputs are AVCC (four-byte NAL lengths) and raw AAC, never Annex B or ADTS.

use std::io::{self, Write};

pub const AUDIO: u8 = 8;
pub const VIDEO: u8 = 9;
const MAX_SIZE: usize = 0xff_ffff;

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

/// AVCDecoderConfigurationRecord from the encoder, sent before the first keyframe.
pub fn avc_sequence_header(avcc: &[u8]) -> io::Result<Vec<u8>> {
    if avcc.len() < 7 || avcc[0] != 1 || avcc[4] & 3 != 3 {
        return Err(invalid("FLV requires an AVC configuration with four-byte NAL lengths"));
    }
    let mut body = vec![0x17, 0, 0, 0, 0];
    body.extend_from_slice(avcc);
    Ok(body)
}

/// AVC access unit; composition offset is signed milliseconds (PTS minus DTS).
pub fn avc_frame(data: &[u8], keyframe: bool, composition_ms: i64) -> io::Result<Vec<u8>> {
    if !(-0x80_0000..=0x7f_ffff).contains(&composition_ms) {
        return Err(invalid("FLV composition offset exceeds signed 24 bits"));
    }
    let mut rest = data;
    if rest.is_empty() {
        return Err(invalid("empty AVC access unit"));
    }
    while !rest.is_empty() {
        let length = rest.get(..4).ok_or_else(|| invalid("truncated AVC NAL length"))?;
        let size = u32::from_be_bytes(length.try_into().expect("four-byte slice")) as usize;
        if size == 0 || size > rest.len() - 4 {
            return Err(invalid("invalid AVC NAL length"));
        }
        rest = &rest[4 + size..];
    }
    let mut body = vec![if keyframe { 0x17 } else { 0x27 }, 1];
    body.extend_from_slice(&(composition_ms as i32).to_be_bytes()[1..]);
    body.extend_from_slice(data);
    Ok(body)
}

/// AudioSpecificConfig (sequence=true), or a raw AAC access unit.
/// For AAC the FLV rate/size/channel flags are fixed; the config carries the actual format.
pub fn aac(data: &[u8], sequence: bool) -> io::Result<Vec<u8>> {
    if data.is_empty() || (sequence && data.len() < 2) {
        return Err(invalid("empty or truncated AAC data"));
    }
    let mut body = vec![0xaf, u8::from(!sequence)];
    body.extend_from_slice(data);
    Ok(body)
}

/// Writes the file header, including PreviousTagSize0. RTMP does not use this wrapper.
pub fn write_header(out: &mut impl Write, audio: bool, video: bool) -> io::Result<()> {
    out.write_all(&[b'F', b'L', b'V', 1, (u8::from(audio) * 4) | u8::from(video), 0, 0, 0, 9, 0, 0, 0, 0])
}

/// Writes an FLV tag with a millisecond decoding timestamp and PreviousTagSize.
pub fn write_tag(out: &mut impl Write, kind: u8, timestamp: u32, body: &[u8]) -> io::Result<()> {
    if ![AUDIO, VIDEO, 18].contains(&kind) || body.len() > MAX_SIZE {
        return Err(invalid("invalid FLV tag type or size"));
    }
    out.write_all(&[kind])?;
    out.write_all(&(body.len() as u32).to_be_bytes()[1..])?;
    let time = timestamp.to_be_bytes();
    out.write_all(&[time[1], time[2], time[3], time[0], 0, 0, 0])?;
    out.write_all(body)?;
    out.write_all(&(11 + body.len() as u32).to_be_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn b_frame_and_nal_validation() {
        assert_eq!(
            avc_frame(&[0, 0, 0, 2, 0x41, 0], false, -33).expect("test operation succeeds"),
            [0x27, 1, 0xff, 0xff, 0xdf, 0, 0, 0, 2, 0x41, 0]
        );
        for data in [&[][..], &[0, 0], &[0, 0, 0, 0], &[0, 0, 0, 2, 1]] {
            assert!(avc_frame(data, true, 0).is_err());
        }
        assert!(avc_frame(&[0, 0, 0, 1, 1], true, 0x80_0000).is_err());
    }

    #[test]
    fn tag_layout_and_extended_timestamp() {
        let mut bytes = Vec::new();
        write_header(&mut bytes, true, true).expect("test operation succeeds");
        let body = aac(&[0x11, 0x90], true).expect("test operation succeeds");
        write_tag(&mut bytes, AUDIO, 0x1234_5678, &body).expect("test operation succeeds");
        assert_eq!(&bytes[..13], b"FLV\x01\x05\0\0\0\x09\0\0\0\0");
        assert_eq!(&bytes[13..24], &[8, 0, 0, 4, 0x34, 0x56, 0x78, 0x12, 0, 0, 0]);
        assert_eq!(&bytes[24..], &[0xaf, 0, 0x11, 0x90, 0, 0, 0, 15]);
    }

    #[test]
    fn sequence_headers() {
        assert!(avc_sequence_header(&[1, 100, 0, 40, 0xfe, 0, 0]).is_err());
        assert_eq!(
            &avc_sequence_header(&[1, 100, 0, 40, 0xff, 0, 0]).expect("test operation succeeds")[..5],
            &[0x17, 0, 0, 0, 0]
        );
        assert_eq!(aac(&[1, 2], false).expect("test operation succeeds"), [0xaf, 1, 1, 2]);
    }
}
