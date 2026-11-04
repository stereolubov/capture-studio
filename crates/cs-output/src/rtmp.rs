//! RTMP outbound chunk framing (Adobe RTMP 1.0 §5.3).
//! Every message starts with fmt=0; continuations use fmt=3. No previous-message state is needed.

use std::io::{self, Read, Write};

pub mod amf;
pub mod chunks;
pub mod client;

/// Performs the simple RTMP v3 handshake, before any chunk messages are exchanged.
/// `random` must contain fresh random bytes supplied by the transport owner.
/// The transport owner also sets read/write deadlines (and TLS for RTMPS).
pub fn handshake(transport: &mut (impl Read + Write), time_ms: u32, random: &[u8; 1528]) -> io::Result<()> {
    let mut c1 = [0u8; 1536];
    c1[..4].copy_from_slice(&time_ms.to_be_bytes());
    c1[8..].copy_from_slice(random);
    transport.write_all(&[3])?;
    transport.write_all(&c1)?;
    transport.flush()?;

    let mut version = [0];
    transport.read_exact(&mut version)?;
    if version[0] != 3 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "unsupported RTMP server version"));
    }
    let mut s1 = [0u8; 1536];
    transport.read_exact(&mut s1)?;
    // C2 echoes the server's time and random data. Its second word is the local receive time.
    let mut c2 = s1;
    c2[4..8].copy_from_slice(&time_ms.to_be_bytes());
    transport.write_all(&c2)?;
    transport.flush()?;

    let mut s2 = [0u8; 1536];
    transport.read_exact(&mut s2)?;
    // The peer's receive time is independent of our clock; only time and random must match C1.
    if s2[..4] != c1[..4] || s2[8..] != c1[8..] {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "RTMP handshake echo mismatch"));
    }
    Ok(())
}

/// Writes one message. Chunk size starts at 128 and changes only after Set Chunk Size is sent.
/// The caller serializes access to the transport so messages cannot interleave mid-chunk.
pub fn write_message(
    out: &mut impl Write,
    chunk_size: u32,
    chunk_stream: u32,
    message_stream: u32,
    kind: u8,
    timestamp: u32,
    payload: &[u8],
) -> io::Result<()> {
    if chunk_size == 0 || chunk_size > 0x7fff_ffff || !(2..=65599).contains(&chunk_stream) || payload.len() > 0xff_ffff
    {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid RTMP chunk parameters"));
    }
    basic_header(out, 0, chunk_stream)?;
    out.write_all(&timestamp.min(0xff_ffff).to_be_bytes()[1..])?;
    out.write_all(&(payload.len() as u32).to_be_bytes()[1..])?;
    out.write_all(&[kind])?;
    out.write_all(&message_stream.to_le_bytes())?;
    if timestamp >= 0xff_ffff {
        out.write_all(&timestamp.to_be_bytes())?;
    }
    for (index, part) in payload.chunks(chunk_size as usize).enumerate() {
        if index != 0 {
            basic_header(out, 3, chunk_stream)?;
            if timestamp >= 0xff_ffff {
                out.write_all(&timestamp.to_be_bytes())?;
            }
        }
        out.write_all(part)?;
    }
    Ok(())
}

fn basic_header(out: &mut impl Write, fmt: u8, id: u32) -> io::Result<()> {
    let prefix = fmt << 6;
    match id {
        2..=63 => out.write_all(&[prefix | id as u8]),
        64..=319 => out.write_all(&[prefix, (id - 64) as u8]),
        _ => out.write_all(&[prefix | 1, (id - 64) as u8, ((id - 64) >> 8) as u8]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_against_local_server() {
        use std::net::{TcpListener, TcpStream};
        use std::time::Duration;
        let listener = TcpListener::bind("127.0.0.1:0").expect("test operation succeeds");
        let address = listener.local_addr().expect("test operation succeeds");
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().expect("test operation succeeds");
            socket.set_read_timeout(Some(Duration::from_secs(3))).expect("test operation succeeds");
            socket.set_write_timeout(Some(Duration::from_secs(3))).expect("test operation succeeds");
            let mut request = [0u8; 1537];
            socket.read_exact(&mut request).expect("test operation succeeds");
            assert_eq!(request[0], 3);
            assert_eq!(&request[5..9], &[0; 4]);
            let mut s1 = [0x5a; 1536];
            s1[..4].copy_from_slice(&123u32.to_be_bytes());
            s1[4..8].fill(0);
            // Split writes deliberately: the client must handle partial reads.
            socket.write_all(&[3]).expect("test operation succeeds");
            for part in s1.chunks(73) {
                socket.write_all(part).expect("test operation succeeds");
            }
            let mut c2 = [0; 1536];
            socket.read_exact(&mut c2).expect("test operation succeeds");
            assert_eq!(&c2[..4], &s1[..4]);
            assert_eq!(&c2[8..], &s1[8..]);
            let mut s2 = request[1..].to_vec();
            s2[4..8].copy_from_slice(&456u32.to_be_bytes());
            socket.write_all(&s2).expect("test operation succeeds");
        });
        let mut client = TcpStream::connect(address).expect("test operation succeeds");
        client.set_read_timeout(Some(Duration::from_secs(3))).expect("test operation succeeds");
        client.set_write_timeout(Some(Duration::from_secs(3))).expect("test operation succeeds");
        handshake(&mut client, 42, &[0x17; 1528]).expect("test operation succeeds");
        server.join().expect("test operation succeeds");
    }

    #[test]
    fn handshake_rejects_wrong_version_echo_and_truncation() {
        struct Peer {
            input: io::Cursor<Vec<u8>>,
        }
        impl Read for Peer {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                self.input.read(buf)
            }
        }
        impl Write for Peer {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        for response in [vec![2], vec![3], {
            let mut bytes = vec![0; 3073];
            bytes[0] = 3;
            bytes
        }] {
            let mut peer = Peer { input: io::Cursor::new(response) };
            assert!(handshake(&mut peer, 42, &[1; 1528]).is_err());
        }
    }

    #[test]
    fn framing_endianness_and_continuation() {
        let mut out = Vec::new();
        write_message(&mut out, 2, 6, 0x12345678, 9, 42, &[1, 2, 3]).expect("test operation succeeds");
        assert_eq!(out, [6, 0, 0, 42, 0, 0, 3, 9, 0x78, 0x56, 0x34, 0x12, 1, 2, 0xc6, 3]);
    }

    #[test]
    fn extended_time_repeated_on_continuation() {
        for timestamp in [0xff_ffff, 0x12345678, u32::MAX] {
            let mut out = Vec::new();
            write_message(&mut out, 1, 6, 1, 9, timestamp, &[1, 2]).expect("test operation succeeds");
            assert_eq!(&out[1..4], &[255; 3]);
            assert_eq!(&out[12..16], &timestamp.to_be_bytes());
            assert_eq!(out[17], 0xc6);
            assert_eq!(&out[18..22], &timestamp.to_be_bytes());
            assert_eq!(out[22], 2);
        }
    }

    #[test]
    fn all_basic_header_sizes_and_empty_message() {
        for (id, expected) in [
            (2, vec![2]),
            (63, vec![63]),
            (64, vec![0, 0]),
            (319, vec![0, 255]),
            (320, vec![1, 0, 1]),
            (65599, vec![1, 255, 255]),
        ] {
            let mut out = Vec::new();
            write_message(&mut out, 128, id, 0, 1, 0, &[]).expect("test operation succeeds");
            assert_eq!(&out[..expected.len()], &expected);
            assert_eq!(out.len(), expected.len() + 11);
        }
    }

    #[test]
    fn invalid_input_writes_nothing() {
        for (size, id) in [(0, 3), (0x80000000, 3), (128, 1), (128, 65600)] {
            let mut out = Vec::new();
            assert!(write_message(&mut out, size, id, 1, 9, 0, &[]).is_err());
            assert!(out.is_empty());
        }
    }
}
