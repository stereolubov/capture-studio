//! Diagnostic publishing probe: cargo run -p cs-output --example publish_flv -- input.flv server-url
//! Stream key is read from CS_STREAM_KEY, never printed. Input must be H.264/AAC FLV.
use cs_output::rtmp::client::{Endpoint, Publisher};
use std::{
    io::{self, Read},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

fn main() -> io::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err(io::Error::other("usage: publish_flv input.flv server-url (key in CS_STREAM_KEY)"));
    }
    let endpoint = Endpoint::parse(&args[2])?;
    let key = std::env::var("CS_STREAM_KEY").map_err(|_| io::Error::other("CS_STREAM_KEY is required"))?;
    let stop = AtomicBool::new(false);
    let mut file = std::fs::File::open(&args[1])?;
    let mut header = [0; 13];
    file.read_exact(&mut header)?;
    if &header[..3] != b"FLV" || header[3] != 1 || header[5..9] != [0, 0, 0, 9] {
        return Err(io::Error::other("unsupported FLV header"));
    }
    let mut publisher = Publisher::connect(&endpoint, &key, &stop)?;
    let start = Instant::now();
    let mut last_poll = Instant::now();
    loop {
        let mut tag = [0; 11];
        match file.read(&mut tag[..1])? {
            0 => break,
            _ => file.read_exact(&mut tag[1..])?,
        }
        let length = (usize::from(tag[1]) << 16) | (usize::from(tag[2]) << 8) | usize::from(tag[3]);
        let timestamp = u32::from_be_bytes([tag[7], tag[4], tag[5], tag[6]]);
        let mut body = vec![0; length];
        file.read_exact(&mut body)?;
        let mut previous = [0; 4];
        file.read_exact(&mut previous)?;
        if u32::from_be_bytes(previous) != 11 + length as u32 {
            return Err(io::Error::other("invalid previous tag size"));
        }
        while start.elapsed() < Duration::from_millis(u64::from(timestamp)) {
            publisher.poll(&stop)?;
            std::thread::sleep(Duration::from_millis(1));
        }
        if last_poll.elapsed() >= Duration::from_millis(100) {
            publisher.poll(&stop)?;
            last_poll = Instant::now();
        }
        if [8, 9, 18].contains(&tag[0]) {
            publisher.media(tag[0], timestamp, &body)?;
        }
    }
    publisher.close()
}
