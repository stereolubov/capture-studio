//! Incremental incoming RTMP chunk decoder; timeouts never discard a partial header.
use std::{collections::HashMap, io};

const MAX_MESSAGE: usize = 1 << 20;
const MAX_BUFFERED: usize = 4 << 20;
#[derive(Clone, Default)]
struct Stream {
    timestamp: u32,
    delta: u32,
    length: usize,
    kind: u8,
    stream: u32,
    extended: bool,
    data: Vec<u8>,
    active: bool,
}
pub struct Message {
    pub timestamp: u32,
    pub stream: u32,
    pub kind: u8,
    pub data: Vec<u8>,
}
pub struct Decoder {
    input: Vec<u8>,
    streams: HashMap<u32, Stream>,
    chunk_size: usize,
}
fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid or oversized RTMP chunk")
}
fn be24(b: &[u8]) -> u32 {
    (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2])
}
fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes(b[..4].try_into().expect("four bytes"))
}

impl Default for Decoder {
    fn default() -> Self {
        Self { input: Vec::new(), streams: HashMap::new(), chunk_size: 128 }
    }
}
impl Decoder {
    pub fn feed(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.input.len() + bytes.len() > MAX_BUFFERED {
            return Err(invalid());
        }
        self.input.extend(bytes);
        Ok(())
    }
    /// Applies Set Chunk Size / Abort before parsing the next chunk from the same read.
    pub fn next_message(&mut self) -> io::Result<Option<Message>> {
        loop {
            let b = &self.input;
            if b.is_empty() {
                return Ok(None);
            }
            let fmt = b[0] >> 6;
            let (id, mut pos) = match b[0] & 63 {
                0 => {
                    if b.len() < 2 {
                        return Ok(None);
                    }
                    (64 + u32::from(b[1]), 2)
                }
                1 => {
                    if b.len() < 3 {
                        return Ok(None);
                    }
                    (64 + u32::from(b[1]) + (u32::from(b[2]) << 8), 3)
                }
                id => (u32::from(id), 1),
            };
            if !self.streams.contains_key(&id) && (fmt != 0 || self.streams.len() >= 64) {
                return Err(invalid());
            }
            let previous = self.streams.get(&id);
            let header_len = [11, 7, 3, 0][fmt as usize];
            if b.len() < pos + header_len {
                return Ok(None);
            }
            // Clone only the header, never an accumulated payload.
            let mut s = previous.map(|s| Stream { data: Vec::new(), ..s.header() }).unwrap_or_default();
            let continuing = previous.is_some_and(|s| s.active);
            if continuing && fmt != 3 {
                return Err(invalid());
            }
            let mut time = 0;
            if fmt < 3 {
                time = be24(&b[pos..]);
                s.extended = time == 0xff_ffff;
            }
            if fmt <= 1 {
                s.length = be24(&b[pos + 3..]) as usize;
                s.kind = b[pos + 6];
            }
            if fmt == 0 {
                s.stream = u32::from_le_bytes(b[pos + 7..pos + 11].try_into().expect("four bytes"));
            }
            pos += header_len;
            if s.extended {
                if b.len() < pos + 4 {
                    return Ok(None);
                }
                time = be32(&b[pos..]);
                pos += 4;
            }
            if !continuing {
                match fmt {
                    0 => {
                        s.timestamp = time;
                        s.delta = time;
                    }
                    1 | 2 => {
                        s.delta = time;
                        s.timestamp = s.timestamp.wrapping_add(time);
                    }
                    _ => s.timestamp = s.timestamp.wrapping_add(s.delta),
                }
            }
            if s.length > MAX_MESSAGE {
                return Err(invalid());
            }
            let accumulated = previous.filter(|_| continuing).map_or(0, |s| s.data.len());
            let count = (s.length - accumulated).min(self.chunk_size);
            if b.len() < pos + count {
                return Ok(None);
            }
            let total: usize = self.streams.values().map(|s| s.data.len()).sum();
            if total + count > MAX_BUFFERED {
                return Err(invalid());
            }
            if continuing {
                s.data = std::mem::take(&mut self.streams.get_mut(&id).expect("existing stream").data);
            }
            s.data.extend_from_slice(&self.input[pos..pos + count]);
            self.input.drain(..pos + count);
            s.active = s.data.len() != s.length;
            let message = (!s.active).then(|| Message {
                timestamp: s.timestamp,
                stream: s.stream,
                kind: s.kind,
                data: std::mem::take(&mut s.data),
            });
            self.streams.insert(id, s);
            if let Some(m) = message {
                if m.kind == 1 {
                    if m.stream != 0 || m.data.len() != 4 {
                        return Err(invalid());
                    }
                    let size = be32(&m.data);
                    if size == 0 || size > MAX_MESSAGE as u32 {
                        return Err(invalid());
                    }
                    self.chunk_size = size as usize;
                } else if m.kind == 2 {
                    if m.data.len() != 4 {
                        return Err(invalid());
                    }
                    if let Some(s) = self.streams.get_mut(&be32(&m.data)) {
                        s.data.clear();
                        s.active = false;
                    }
                }
                return Ok(Some(m));
            }
        }
    }
}
impl Stream {
    fn header(&self) -> Self {
        Self {
            timestamp: self.timestamp,
            delta: self.delta,
            length: self.length,
            kind: self.kind,
            stream: self.stream,
            extended: self.extended,
            active: self.active,
            data: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rtmp::write_message;
    #[test]
    fn bytewise_fragmentation_and_chunk_size_change() {
        let mut wire = Vec::new();
        write_message(&mut wire, 128, 2, 0, 1, 0, &2u32.to_be_bytes()).expect("control");
        write_message(&mut wire, 2, 320, 7, 20, 0x12345678, &[1, 2, 3, 4, 5]).expect("message");
        let mut decoder = Decoder::default();
        let mut messages = Vec::new();
        for byte in wire {
            decoder.feed(&[byte]).expect("feed");
            while let Some(m) = decoder.next_message().expect("parse") {
                messages.push(m);
            }
        }
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].timestamp, 0x12345678);
        assert_eq!(messages[1].stream, 7);
        assert_eq!(messages[1].data, [1, 2, 3, 4, 5]);
    }
    #[test]
    fn compressed_headers_and_timestamp_wrap() {
        let mut wire = Vec::new();
        write_message(&mut wire, 128, 3, 1, 20, u32::MAX - 1, &[1]).expect("first");
        wire.extend([0x43, 0, 0, 3, 0, 0, 1, 20, 2]); // fmt1: delta 3
        wire.extend([0x83, 0, 0, 4, 3]); // fmt2: delta 4
        wire.extend([0xc3, 4]); // fmt3: another delta 4
        let mut decoder = Decoder::default();
        decoder.feed(&wire).expect("feed");
        for time in [u32::MAX - 1, 1, 5, 9] {
            assert_eq!(decoder.next_message().expect("parse").expect("message").timestamp, time);
        }
    }
    #[test]
    fn rejects_unknown_compressed_stream_and_large_message() {
        let mut d = Decoder::default();
        d.feed(&[0xc3]).expect("feed");
        assert!(d.next_message().is_err());
        let mut d = Decoder::default();
        d.feed(&[3, 0, 0, 0, 0xff, 0xff, 0xff, 20, 0, 0, 0, 0]).expect("feed");
        assert!(d.next_message().is_err());
    }
}
