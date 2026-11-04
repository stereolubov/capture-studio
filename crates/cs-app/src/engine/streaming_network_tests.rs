//! Short local development tests, not service acceptance or a soak run.
use super::*;
use cs_encode::{Packet, RateControl, VideoSettings};
use cs_output::rtmp::{
    amf::{self, Value, object, string},
    chunks::{Decoder, Message},
    write_message,
};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};

struct Peer {
    socket: TcpStream,
    decoder: Decoder,
}
impl Peer {
    fn accept(listener: &TcpListener) -> Self {
        listener.set_nonblocking(true).expect("nonblocking listener");
        let deadline = Instant::now() + Duration::from_secs(8);
        let socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "publisher did not reconnect");
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(e) => panic!("accept: {e}"),
            }
        };
        // Winsock inherits the listener's nonblocking flag on accepted sockets.
        socket.set_nonblocking(false).expect("blocking peer");
        socket.set_read_timeout(Some(Duration::from_secs(3))).expect("read timeout");
        socket.set_write_timeout(Some(Duration::from_secs(3))).expect("write timeout");
        Self { socket, decoder: Decoder::default() }
    }
    fn message(&mut self) -> Message {
        loop {
            if let Some(message) = self.decoder.next_message().expect("decode") {
                return message;
            }
            let mut bytes = [0; 8192];
            let n = self.socket.read(&mut bytes).expect("read");
            assert_ne!(n, 0, "unexpected disconnect");
            self.decoder.feed(&bytes[..n]).expect("feed");
        }
    }
    fn command(&mut self, expected: &str) {
        loop {
            let message = self.message();
            if message.kind == 20 {
                let values = amf::decode(&message.data).expect("AMF");
                assert_eq!(values.first().and_then(Value::text), Some(expected));
                return;
            }
        }
    }
    fn reply(&mut self, values: &[Value]) {
        write_message(&mut self.socket, 128, 3, 0, 20, 0, &amf::encode(values).expect("AMF")).expect("reply");
    }
    fn publish(&mut self, reject: bool) {
        let mut c = [0; 1537];
        self.socket.read_exact(&mut c).expect("C0C1");
        self.socket.write_all(&[3]).expect("S0");
        self.socket.write_all(&[0; 1536]).expect("S1");
        self.socket.write_all(&c[1..]).expect("S2");
        self.socket.read_exact(&mut [0; 1536]).expect("C2");
        self.command("connect");
        self.reply(&[
            string("_result"),
            Value::Number(1.),
            Value::Null,
            object(&[("code", string("NetConnection.Connect.Success"))]),
        ]);
        self.command("releaseStream");
        self.command("FCPublish");
        self.command("createStream");
        self.reply(&[string("_result"), Value::Number(2.), Value::Null, Value::Number(7.)]);
        self.command("publish");
        self.reply(&[
            string("onStatus"),
            Value::Number(0.),
            Value::Null,
            object(&[
                ("level", string(if reject { "error" } else { "status" })),
                ("code", string(if reject { "NetStream.Publish.BadName" } else { "NetStream.Publish.Start" })),
                ("description", string("untrusted server text: secret-marker")),
            ]),
        ]);
    }
    fn media(&mut self) {
        let (mut avc, mut aac, mut video, mut audio) = (false, false, 0, 0);
        while video < 3 || audio < 3 {
            let m = self.message();
            if ![8, 9].contains(&m.kind) {
                continue;
            }
            assert_eq!(m.stream, 7);
            assert!(m.data.len() >= 2);
            match (m.kind, m.data[1]) {
                (9, 0) => {
                    avc = true;
                    assert_eq!(m.timestamp, 0);
                }
                (8, 0) => {
                    aac = true;
                }
                (9, 1) => {
                    assert!(avc && aac, "headers must be resent after reconnect");
                    if video == 0 {
                        assert_eq!(m.timestamp, 0, "new connection has a fresh epoch");
                        assert_eq!(m.data[0], 0x17, "first video frame must be IDR");
                    }
                    video += 1;
                }
                (8, 1) => {
                    assert!(aac);
                    audio += 1;
                }
                _ => panic!("invalid media"),
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Scenario {
    Recover,
    Cancel,
    Reject,
}
fn scenario(scenario: Scenario) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("local listener");
    let address = listener.local_addr().expect("address");
    let (tx, rx) = crossbeam_channel::bounded(256);
    let (event_tx, event_rx) = crossbeam_channel::unbounded();
    let stats = Arc::new(RecordStats::default());
    *stats.stream_status.lock() = OutputStatus::Starting;
    let stop = Arc::new(AtomicBool::new(false));
    let server_stop = stop.clone();
    let server = std::thread::spawn(move || {
        let mut first = Peer::accept(&listener);
        first.publish(matches!(scenario, Scenario::Reject));
        if matches!(scenario, Scenario::Reject) {
            return;
        }
        first.media();
        drop(first); // real TCP EOF, not an injected mock error
        if matches!(scenario, Scenario::Recover) {
            let mut second = Peer::accept(&listener);
            second.publish(false);
            second.media();
            server_stop.store(true, Ordering::Release);
        }
    });
    let encoder_stats = stats.clone();
    let encoder_stop = stop.clone();
    let producer = std::thread::spawn(move || {
        let mut frame = 0i64;
        while !encoder_stop.load(Ordering::Acquire) {
            let idr = encoder_stats.request_keyframe.swap(false, Ordering::AcqRel);
            let data = if idr {
                vec![0, 0, 0, 1, 0x67, 0x64, 0, 0x28, 0xaa, 0, 0, 0, 1, 0x68, 0xee, 0x3c, 0x80, 0, 0, 1, 0x65, 0x88]
            } else {
                vec![0, 0, 1, 0x41, 0x88]
            };
            let time = frame * 333_333;
            encoder_stats.latest_dts.store(time, Ordering::Relaxed);
            // Deliberately mark every P-slice as keyframe: the output must inspect NAL type 5.
            if tx.send((0, Packet { data, pts: time, dts: time, keyframe: true })).is_err() {
                break;
            }
            if tx.send((1, Packet { data: vec![1, 2, 3], pts: time, dts: time, keyframe: true })).is_err() {
                break;
            }
            frame += 1;
            std::thread::sleep(Duration::from_millis(10));
        }
    });
    let output_stats = stats.clone();
    let output_stop = stop.clone();
    let output = std::thread::spawn(move || {
        run(
            Config {
                server: format!("rtmp://{address}/live"),
                key: "synthetic-key".into(),
                reconnect: true,
                dynamic_bitrate: false,
            },
            rx,
            output_stats,
            output_stop,
            event_tx,
            VideoSettings {
                width: 640,
                height: 360,
                fps_num: 30,
                fps_den: 1,
                bitrate_kbps: 1500,
                keyframe_interval_s: 10,
                rate_control: RateControl::Cbr,
            },
            160,
        )
    });
    let mut reconnects = 0;
    let mut starts = 0;
    let mut errors = 0;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !output.is_finished() && Instant::now() < deadline {
        if let Ok(event) = event_rx.recv_timeout(Duration::from_millis(20)) {
            match event {
                Event::StreamStarted => starts += 1,
                Event::StreamReconnecting { .. } => {
                    reconnects += 1;
                    if matches!(scenario, Scenario::Cancel) {
                        stop.store(true, Ordering::Release);
                    }
                }
                Event::Error(message) => {
                    errors += 1;
                    assert!(!message.contains("secret-marker"));
                }
                _ => {}
            }
        }
    }
    let finished = output.is_finished();
    stop.store(true, Ordering::Release);
    output.join().expect("output");
    producer.join().expect("producer");
    server.join().expect("server");
    for event in event_rx.try_iter() {
        match event {
            Event::StreamStarted => starts += 1,
            Event::StreamReconnecting { .. } => reconnects += 1,
            Event::Error(_) => errors += 1,
            _ => {}
        }
    }
    assert!(finished, "output must terminate within the bounded test");
    if matches!(scenario, Scenario::Reject) {
        assert_eq!((starts, reconnects, errors), (0, 0, 1), "auth failure must not retry");
    } else {
        assert_eq!((starts, reconnects, errors), (1, 1, 0));
        if matches!(scenario, Scenario::Recover) {
            assert!(stats.dropped_network.load(Ordering::Relaxed) > 0);
        }
    }
}

#[test]
#[ignore = "short development test requiring local TCP sockets"]
fn tcp_disconnect_republishes_headers_and_resumes_on_idr() {
    scenario(Scenario::Recover);
}
#[test]
#[ignore = "short development test requiring local TCP sockets"]
fn stop_interrupts_reconnect_backoff() {
    scenario(Scenario::Cancel);
}
#[test]
#[ignore = "short development test requiring local TCP sockets"]
fn rejected_key_does_not_retry_or_expose_server_text() {
    scenario(Scenario::Reject);
}
