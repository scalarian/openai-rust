use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

/// Captured HTTP request data for mock transport tests.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CapturedRequest {
    pub method: String,
    pub path: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
    pub received_after: Duration,
}

/// Scripted HTTP response returned by the mock server.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptedResponse {
    pub status_code: u16,
    pub reason: &'static str,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub chunked: bool,
    pub delay: Duration,
}

impl Default for ScriptedResponse {
    fn default() -> Self {
        Self {
            status_code: 200,
            reason: "OK",
            headers: vec![(String::from("content-length"), String::from("0"))],
            body: Vec::new(),
            chunked: false,
            delay: Duration::ZERO,
        }
    }
}

/// Single-request loopback mock HTTP harness.
#[allow(dead_code)]
pub struct MockHttpServer {
    addr: SocketAddr,
    captured: mpsc::Receiver<CapturedRequest>,
    worker: Option<thread::JoinHandle<()>>,
}

impl MockHttpServer {
    /// Spawns a loopback server for a single scripted response.
    #[allow(dead_code)]
    pub fn spawn(response: ScriptedResponse) -> std::io::Result<Self> {
        Self::spawn_sequence(vec![response])
    }

    /// Spawns a loopback server for a scripted response sequence.
    pub fn spawn_sequence(responses: Vec<ScriptedResponse>) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(false)?;
        let addr = listener.local_addr()?;
        let (tx, rx) = mpsc::channel();
        let started_at = Instant::now();
        let worker = thread::spawn(move || {
            for response in responses {
                if let Ok((mut stream, _)) = listener.accept() {
                    let request = read_request(&mut stream, started_at).unwrap_or_default();
                    let _ = tx.send(request);
                    if !response.delay.is_zero() {
                        thread::sleep(response.delay);
                    }
                    let mut response_bytes =
                        format!("HTTP/1.1 {} {}\r\n", response.status_code, response.reason)
                            .into_bytes();
                    for (name, value) in &response.headers {
                        response_bytes
                            .extend_from_slice(format!("{}: {}\r\n", name, value).as_bytes());
                    }
                    if response.chunked {
                        response_bytes.extend_from_slice(b"transfer-encoding: chunked\r\n");
                    }
                    response_bytes.extend_from_slice(b"connection: close\r\n");
                    response_bytes.extend_from_slice(b"\r\n");
                    if response.chunked {
                        response_bytes.extend_from_slice(&encode_chunked_body(&response.body));
                    } else {
                        response_bytes.extend_from_slice(&response.body);
                    }
                    let _ = stream.write_all(&response_bytes);
                    let _ = stream.flush();
                    let _ = stream.shutdown(Shutdown::Write);
                } else {
                    break;
                }
            }
        });
        thread::sleep(Duration::from_millis(10));

        Ok(Self {
            addr,
            captured: rx,
            worker: Some(worker),
        })
    }

    /// Returns the loopback base URL.
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Waits for the captured request.
    #[allow(dead_code)]
    pub fn captured_request(&self) -> Option<CapturedRequest> {
        self.captured.recv_timeout(Duration::from_secs(2)).ok()
    }

    /// Waits for `count` captured requests in order.
    #[allow(dead_code)]
    pub fn captured_requests(&self, count: usize) -> Option<Vec<CapturedRequest>> {
        let mut captured = Vec::with_capacity(count);
        for _ in 0..count {
            captured.push(self.captured.recv_timeout(Duration::from_secs(3)).ok()?);
        }
        Some(captured)
    }
}

fn encode_chunked_body(body: &[u8]) -> Vec<u8> {
    if body.is_empty() {
        return b"0\r\n\r\n".to_vec();
    }

    let midpoint = (body.len() / 2).max(1).min(body.len());
    let mut encoded = Vec::new();
    for chunk in [&body[..midpoint], &body[midpoint..]] {
        if chunk.is_empty() {
            continue;
        }
        encoded.extend_from_slice(format!("{:X}\r\n", chunk.len()).as_bytes());
        encoded.extend_from_slice(chunk);
        encoded.extend_from_slice(b"\r\n");
    }
    encoded.extend_from_slice(b"0\r\n\r\n");
    encoded
}

impl Drop for MockHttpServer {
    fn drop(&mut self) {
        let _ = TcpStream::connect(self.addr);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn read_request(stream: &mut TcpStream, started_at: Instant) -> std::io::Result<CapturedRequest> {
    let mut buffer = Vec::new();
    let mut header_end = None;
    loop {
        let mut chunk = [0_u8; 1024];
        let bytes_read = stream.read(&mut chunk)?;
        if bytes_read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..bytes_read]);
        if let Some(position) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            header_end = Some(position);
            break;
        }
    }

    let Some(header_end) = header_end else {
        return Ok(CapturedRequest::default());
    };
    let body_start = header_end + 4;
    let header_text = String::from_utf8_lossy(&buffer[..body_start]);
    let mut lines = header_text.lines();
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();
    let mut headers = BTreeMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    while buffer.len().saturating_sub(body_start) < content_length {
        let mut chunk = [0_u8; 1024];
        let bytes_read = stream.read(&mut chunk)?;
        if bytes_read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..bytes_read]);
    }

    Ok(CapturedRequest {
        method,
        path,
        headers,
        body: buffer[body_start..].to_vec(),
        received_after: started_at.elapsed(),
    })
}
