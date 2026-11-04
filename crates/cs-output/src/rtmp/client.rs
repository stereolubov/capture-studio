//! Synchronous publishing client, intended to run on a dedicated output thread.
//! Secrets are excluded from Debug and all errors; server descriptions are untrusted.
use super::{
    amf::{self, Value, object, string},
    chunks::{Decoder, Message},
    handshake, write_message,
};
use std::{
    io::{self, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

const CHUNK_SIZE: u32 = 4096;

pub struct Endpoint {
    host: String,
    port: u16,
    tls: bool,
    app: String,
    tc_url: String,
}
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn cancelled(stop: &AtomicBool) -> io::Result<()> {
    if stop.load(Ordering::Acquire) {
        Err(io::Error::new(io::ErrorKind::Interrupted, "stream stopped"))
    } else {
        Ok(())
    }
}
impl Endpoint {
    /// `server` is the application URL, without a stream key. Query parameters belong to app.
    pub fn parse(server: &str) -> io::Result<Self> {
        let url = url::Url::parse(server).map_err(|_| invalid("invalid RTMP server URL"))?;
        let tls = match url.scheme() {
            "rtmp" => false,
            "rtmps" => true,
            _ => return Err(invalid("expected rtmp:// or rtmps://")),
        };
        if !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
            return Err(invalid("credentials and fragments are not allowed in server URL"));
        }
        let host = match url.host().ok_or_else(|| invalid("missing RTMP server host"))? {
            url::Host::Domain(s) => s.to_owned(),
            url::Host::Ipv4(ip) => ip.to_string(),
            url::Host::Ipv6(ip) => ip.to_string(),
        };
        let path = url.path().trim_matches('/');
        if path.is_empty() {
            return Err(invalid("missing RTMP application path"));
        }
        let mut app = path.to_owned();
        if let Some(query) = url.query() {
            app.push('?');
            app.push_str(query);
        }
        Ok(Self {
            host,
            port: url.port().unwrap_or(if tls { 443 } else { 1935 }),
            tls,
            app,
            tc_url: server.trim_end_matches('/').to_owned(),
        })
    }
}

enum Transport {
    Tcp(TcpStream),
    Tls(Box<native_tls::TlsStream<TcpStream>>),
}
impl Read for Transport {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Tcp(s) => s.read(buf),
            Self::Tls(s) => s.read(buf),
        }
    }
}
impl Write for Transport {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Tcp(s) => s.write(buf),
            Self::Tls(s) => s.write(buf),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Tcp(s) => s.flush(),
            Self::Tls(s) => s.flush(),
        }
    }
}

/// One published connection. Call `poll` regularly even while no media is available.
pub struct Publisher {
    transport: Transport,
    decoder: Decoder,
    stream: u32,
    received: u64,
    acknowledged: u64,
    ack_window: u32,
    pub sent_bytes: u64,
    pub rtt: Option<Duration>,
    ping: Option<(u32, Instant)>,
    last_ping: Instant,
}
impl Publisher {
    pub fn connect(endpoint: &Endpoint, key: &str, stop: &AtomicBool) -> io::Result<Self> {
        if key.is_empty() || key.len() > 4096 || key.chars().any(char::is_control) {
            return Err(invalid("missing or invalid stream key"));
        }
        cancelled(stop)?;
        // Name resolution is performed only on the output worker, never on the render/UI thread.
        let addresses = (endpoint.host.as_str(), endpoint.port)
            .to_socket_addrs()
            .map_err(|_| invalid("RTMP server name resolution failed"))?;
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut connected = None;
        for address in addresses.take(8) {
            cancelled(stop)?;
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            if let Ok(s) = TcpStream::connect_timeout(&address, remaining.min(Duration::from_secs(1))) {
                connected = Some(s);
                break;
            }
        }
        let socket = connected.ok_or_else(|| invalid("RTMP TCP connection failed"))?;
        socket.set_nodelay(true)?;
        socket.set_read_timeout(Some(Duration::from_secs(3)))?;
        socket.set_write_timeout(Some(Duration::from_secs(3)))?;
        let mut transport = if endpoint.tls {
            let connector = native_tls::TlsConnector::builder()
                .min_protocol_version(Some(native_tls::Protocol::Tlsv12))
                .build()
                .map_err(|_| invalid("TLS initialization failed"))?;
            let tls = connector
                .connect(&endpoint.host, socket)
                .map_err(|_| invalid("RTMPS TLS handshake or certificate validation failed"))?;
            Transport::Tls(Box::new(tls))
        } else {
            Transport::Tcp(socket)
        };
        cancelled(stop)?;
        let mut random = [0; 1528];
        getrandom::fill(&mut random).map_err(|_| invalid("RTMP random generation failed"))?;
        handshake(&mut transport, 0, &random)?;
        match &transport {
            Transport::Tcp(s) => s,
            Transport::Tls(s) => s.get_ref(),
        }
        .set_read_timeout(Some(Duration::from_millis(20)))?;
        let mut publisher = Self {
            transport,
            decoder: Decoder::default(),
            stream: 0,
            received: 0,
            acknowledged: 0,
            ack_window: 0,
            sent_bytes: 0,
            rtt: None,
            ping: None,
            last_ping: Instant::now(),
        };
        // Announce the new outbound size using the default size first.
        write_message(&mut publisher.transport, 128, 2, 0, 1, 0, &CHUNK_SIZE.to_be_bytes())?;
        publisher.command(
            0,
            &[
                string("connect"),
                Value::Number(1.),
                object(&[
                    ("app", string(&endpoint.app)),
                    ("tcUrl", string(&endpoint.tc_url)),
                    ("flashVer", string("FMLE/3.0 (compatible; Capture Studio)")),
                    ("fpad", Value::Bool(false)),
                    ("capabilities", Value::Number(15.)),
                    ("audioCodecs", Value::Number(0x0400 as f64)),
                    ("videoCodecs", Value::Number(0x0080 as f64)),
                    ("objectEncoding", Value::Number(0.)),
                ]),
            ],
        )?;
        publisher.wait(stop, |values, _| if transaction(values, 1.) { Ok(true) } else { Ok(false) })?;
        // FMLE compatibility commands have no required replies.
        publisher.command(0, &[string("releaseStream"), Value::Number(0.), Value::Null, string(key)])?;
        publisher.command(0, &[string("FCPublish"), Value::Number(0.), Value::Null, string(key)])?;
        publisher.command(0, &[string("createStream"), Value::Number(2.), Value::Null])?;
        publisher.wait(stop, |values, stream| {
            if !transaction(values, 2.) {
                return Ok(false);
            }
            let id = values.get(3).and_then(Value::number).ok_or_else(|| invalid("missing RTMP stream ID"))?;
            if !id.is_finite() || id < 1. || id > u32::MAX as f64 || id.fract() != 0. {
                return Err(invalid("invalid RTMP stream ID"));
            }
            *stream = id as u32;
            Ok(true)
        })?;
        publisher.command(
            publisher.stream,
            &[string("publish"), Value::Number(0.), Value::Null, string(key), string("live")],
        )?;
        publisher.wait(stop, |values, _| Ok(status(values) == Some("NetStream.Publish.Start")))?;
        Ok(publisher)
    }

    fn send(&mut self, csid: u32, stream: u32, kind: u8, timestamp: u32, data: &[u8]) -> io::Result<()> {
        write_message(&mut self.transport, CHUNK_SIZE, csid, stream, kind, timestamp, data)?;
        self.transport.flush()?;
        self.sent_bytes += data.len() as u64;
        Ok(())
    }
    fn command(&mut self, stream: u32, values: &[Value]) -> io::Result<()> {
        self.send(if stream == 0 { 3 } else { 8 }, stream, 20, 0, &amf::encode(values)?)
    }
    fn read(&mut self) -> io::Result<()> {
        let mut buf = [0; 8192];
        match self.transport.read(&mut buf) {
            Ok(0) => Err(io::Error::new(io::ErrorKind::UnexpectedEof, "RTMP server disconnected")),
            Ok(n) => {
                self.received += n as u64;
                self.decoder.feed(&buf[..n])?;
                if self.ack_window != 0 && self.received - self.acknowledged >= u64::from(self.ack_window) {
                    self.send(2, 0, 3, 0, &(self.received as u32).to_be_bytes())?;
                    self.acknowledged = self.received;
                }
                Ok(())
            }
            Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => Ok(()),
            Err(e) => Err(e),
        }
    }
    fn handle(&mut self, message: Message) -> io::Result<Option<Vec<Value>>> {
        let data = &message.data;
        match message.kind {
            1..=3 => {}
            5 => {
                if data.len() != 4 {
                    return Err(invalid("invalid RTMP acknowledgement window"));
                }
                self.ack_window = u32::from_be_bytes(data[..4].try_into().expect("four bytes"));
                if self.ack_window == 0 {
                    return Err(invalid("zero RTMP acknowledgement window"));
                }
            }
            6 => {
                if data.len() != 5 {
                    return Err(invalid("invalid RTMP peer bandwidth"));
                }
                self.send(2, 0, 5, 0, &data[..4])?;
            }
            4 => {
                if data.len() < 2 {
                    return Err(invalid("invalid RTMP user control"));
                }
                let event = u16::from_be_bytes([data[0], data[1]]);
                if event == 6 {
                    if data.len() != 6 {
                        return Err(invalid("invalid RTMP ping"));
                    }
                    let mut reply = data.clone();
                    reply[1] = 7;
                    self.send(2, 0, 4, 0, &reply)?;
                } else if event == 7 && data.len() == 6 {
                    let token = u32::from_be_bytes(data[2..].try_into().expect("four bytes"));
                    if let Some((sent, started)) = self.ping
                        && sent == token
                    {
                        self.rtt = Some(started.elapsed());
                        self.ping = None;
                    }
                }
            }
            17 | 20 => {
                let body = if message.kind == 17 {
                    if data.first() != Some(&0) {
                        return Err(invalid("unsupported AMF3 RTMP command"));
                    }
                    &data[1..]
                } else {
                    data.as_slice()
                };
                let values = amf::decode(body)?;
                if values.first().and_then(Value::text) == Some("_error")
                    || values.iter().any(|v| v.field("level").and_then(Value::text) == Some("error"))
                {
                    return Err(io::Error::new(io::ErrorKind::PermissionDenied, "RTMP server rejected publishing"));
                }
                if matches!(status(&values), Some("NetStream.Unpublish.Success" | "NetStream.Play.Stop")) {
                    return Err(invalid("RTMP publishing stopped by server"));
                }
                return Ok(Some(values));
            }
            _ => {}
        }
        Ok(None)
    }
    fn wait(
        &mut self,
        stop: &AtomicBool,
        mut ready: impl FnMut(&[Value], &mut u32) -> io::Result<bool>,
    ) -> io::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            cancelled(stop)?;
            if Instant::now() >= deadline {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "RTMP command timed out"));
            }
            if let Some(message) = self.decoder.next_message()? {
                if let Some(values) = self.handle(message)?
                    && ready(&values, &mut self.stream)?
                {
                    return Ok(());
                }
            } else {
                self.read()?;
            }
        }
    }
    /// Sends an FLV audio/video body or AMF0 metadata on the published stream.
    pub fn media(&mut self, kind: u8, timestamp: u32, body: &[u8]) -> io::Result<()> {
        let csid = match kind {
            8 => 4,
            9 => 6,
            18 => 5,
            _ => return Err(invalid("invalid RTMP media type")),
        };
        self.send(csid, self.stream, kind, timestamp, body)
    }
    /// Reads available control messages with a short bounded wait.
    pub fn poll(&mut self, stop: &AtomicBool) -> io::Result<()> {
        cancelled(stop)?;
        if self.last_ping.elapsed() >= Duration::from_secs(5) && self.ping.is_none() {
            let token = (self.sent_bytes & 0xffff_ffff) as u32;
            let mut data = vec![0, 6];
            data.extend(token.to_be_bytes());
            self.send(2, 0, 4, 0, &data)?;
            self.ping = Some((token, Instant::now()));
            self.last_ping = Instant::now();
        }
        if self.ping.is_some_and(|(_, started)| started.elapsed() > Duration::from_secs(30)) {
            return Err(invalid("RTMP ping timed out"));
        }
        // Bound work per poll even if a peer continuously sends control messages.
        self.read()?;
        for _ in 0..64 {
            let Some(message) = self.decoder.next_message()? else {
                break;
            };
            self.handle(message)?;
        }
        Ok(())
    }
    pub fn close(mut self) -> io::Result<()> {
        self.command(0, &[string("deleteStream"), Value::Number(0.), Value::Null, Value::Number(self.stream as f64)])
    }
}
fn transaction(values: &[Value], id: f64) -> bool {
    values.first().and_then(Value::text) == Some("_result") && values.get(1).and_then(Value::number) == Some(id)
}
fn status(values: &[Value]) -> Option<&str> {
    if values.first().and_then(Value::text) != Some("onStatus") {
        return None;
    }
    values.iter().find_map(|v| v.field("code").and_then(Value::text))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoint_validation_and_ipv6() {
        let e = Endpoint::parse("rtmps://[::1]:443/live2?backup=1").expect("endpoint");
        assert_eq!(e.host, "::1");
        assert_eq!(e.app, "live2?backup=1");
        assert!(e.tls);
        assert_eq!(e.port, 443);
        for url in [
            "https://example.com/app",
            "rtmp://user:secret@example.com/app",
            "rtmp://example.com",
            "rtmp://example.com/app#secret",
        ] {
            assert!(Endpoint::parse(url).is_err());
        }
    }

    #[test]
    fn local_publish_control_messages_and_media() {
        use std::net::TcpListener;
        fn message(socket: &mut TcpStream, decoder: &mut Decoder) -> Message {
            loop {
                if let Some(m) = decoder.next_message().expect("chunk") {
                    return m;
                }
                let mut bytes = [0; 8192];
                let count = socket.read(&mut bytes).expect("read");
                assert!(count > 0);
                decoder.feed(&bytes[..count]).expect("feed");
            }
        }
        fn command(socket: &mut TcpStream, decoder: &mut Decoder) -> Vec<Value> {
            loop {
                let m = message(socket, decoder);
                if m.kind == 20 {
                    return amf::decode(&m.data).expect("AMF");
                }
            }
        }
        fn reply(socket: &mut TcpStream, values: &[Value]) {
            write_message(socket, 128, 3, 0, 20, 0, &amf::encode(values).expect("AMF")).expect("reply");
        }
        let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
        let address = listener.local_addr().expect("address");
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().expect("accept");
            socket.set_read_timeout(Some(Duration::from_secs(3))).expect("timeout");
            socket.set_write_timeout(Some(Duration::from_secs(3))).expect("timeout");
            let mut c = [0; 1537];
            socket.read_exact(&mut c).expect("C0C1");
            socket.write_all(&[3]).expect("S0");
            socket.write_all(&[0; 1536]).expect("S1");
            socket.write_all(&c[1..]).expect("S2");
            let mut c2 = [0; 1536];
            socket.read_exact(&mut c2).expect("C2");
            let mut decoder = Decoder::default();
            let connect = command(&mut socket, &mut decoder);
            assert_eq!(connect[0].text(), Some("connect"));
            assert_eq!(connect[2].field("app").and_then(Value::text), Some("live"));
            write_message(&mut socket, 128, 2, 0, 5, 0, &1u32.to_be_bytes()).expect("ack window");
            write_message(&mut socket, 128, 2, 0, 4, 0, &[0, 6, 0, 0, 0, 42]).expect("ping");
            reply(
                &mut socket,
                &[
                    string("_result"),
                    Value::Number(1.),
                    Value::Null,
                    object(&[("code", string("NetConnection.Connect.Success"))]),
                ],
            );
            for name in ["releaseStream", "FCPublish", "createStream"] {
                assert_eq!(command(&mut socket, &mut decoder)[0].text(), Some(name));
            }
            reply(&mut socket, &[string("_result"), Value::Number(2.), Value::Null, Value::Number(7.)]);
            let publish = command(&mut socket, &mut decoder);
            assert_eq!(publish[0].text(), Some("publish"));
            assert_eq!(publish[3].text(), Some("test-key"));
            reply(
                &mut socket,
                &[
                    string("onStatus"),
                    Value::Number(0.),
                    Value::Null,
                    object(&[("level", string("status")), ("code", string("NetStream.Publish.Start"))]),
                ],
            );
            loop {
                let m = message(&mut socket, &mut decoder);
                if m.kind == 9 {
                    assert_eq!(m.stream, 7);
                    assert_eq!(m.timestamp, 33);
                    assert_eq!(m.data, [0x17, 1, 0, 0, 0, 0, 0, 0, 1, 0x65]);
                    break;
                }
            }
            assert_eq!(command(&mut socket, &mut decoder)[0].text(), Some("deleteStream"));
        });
        let endpoint = Endpoint::parse(&format!("rtmp://{address}/live")).expect("endpoint");
        let mut publisher = Publisher::connect(&endpoint, "test-key", &AtomicBool::new(false)).expect("publish");
        publisher.media(9, 33, &[0x17, 1, 0, 0, 0, 0, 0, 0, 1, 0x65]).expect("video");
        publisher.close().expect("close");
        server.join().expect("server");
    }

    #[test]
    fn rtmps_rejects_untrusted_certificate_without_sending_key() {
        use std::net::TcpListener;
        // Synthetic self-signed localhost identity; not a production credential.
        let identity = native_tls::Identity::from_pkcs12(
            include_bytes!("../../tests/fixtures/localhost-untrusted.p12"),
            "test-only",
        )
        .expect("test identity");
        let acceptor = native_tls::TlsAcceptor::new(identity).expect("TLS acceptor");
        let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
        let port = listener.local_addr().expect("address").port();
        let server = std::thread::spawn(move || {
            let (socket, _) = listener.accept().expect("accept");
            socket.set_read_timeout(Some(Duration::from_secs(3))).expect("timeout");
            socket.set_write_timeout(Some(Duration::from_secs(3))).expect("timeout");
            if let Ok(mut tls) = acceptor.accept(socket) {
                let mut application_data = [0; 1537];
                assert!(!matches!(tls.read(&mut application_data), Ok(n) if n > 0));
            }
        });
        let endpoint = Endpoint::parse(&format!("rtmps://localhost:{port}/live")).expect("endpoint");
        let result = Publisher::connect(&endpoint, "secret-never-sent", &AtomicBool::new(false));
        let error = result.err().expect("untrusted certificate rejected");
        assert!(error.to_string().contains("certificate validation failed"));
        assert!(!error.to_string().contains("secret-never-sent"));
        server.join().expect("server");
    }
}
