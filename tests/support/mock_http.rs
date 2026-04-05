use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::mpsc,
    thread,
    time::Duration,
};

/// Captured HTTP request data for mock transport tests.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CapturedRequest {
    pub method: String,
    pub path: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

/// Scripted HTTP response returned by the mock server.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptedResponse {
    pub status_code: u16,
    pub reason: &'static str,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Default for ScriptedResponse {
    fn default() -> Self {
        Self {
            status_code: 200,
            reason: "OK",
            headers: vec![(String::from("content-length"), String::from("0"))],
            body: Vec::new(),
        }
    }
}

/// Single-request loopback mock HTTP harness.
pub struct MockHttpServer {
    addr: SocketAddr,
    captured: mpsc::Receiver<CapturedRequest>,
    worker: Option<thread::JoinHandle<()>>,
}

impl MockHttpServer {
    /// Spawns a loopback server for a single scripted response.
    pub fn spawn(response: ScriptedResponse) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(false)?;
        let addr = listener.local_addr()?;
        let (tx, rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let request = read_request(&mut stream).unwrap_or_default();
                let _ = tx.send(request);
                let mut response_bytes =
                    format!("HTTP/1.1 {} {}\r\n", response.status_code, response.reason)
                        .into_bytes();
                for (name, value) in &response.headers {
                    response_bytes.extend_from_slice(format!("{}: {}\r\n", name, value).as_bytes());
                }
                response_bytes.extend_from_slice(b"\r\n");
                response_bytes.extend_from_slice(&response.body);
                let _ = stream.write_all(&response_bytes);
                let _ = stream.flush();
            }
        });

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
    pub fn captured_request(&self) -> Option<CapturedRequest> {
        self.captured.recv_timeout(Duration::from_secs(2)).ok()
    }
}

impl Drop for MockHttpServer {
    fn drop(&mut self) {
        let _ = TcpStream::connect(self.addr);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<CapturedRequest> {
    let mut buffer = Vec::new();
    stream.read_to_end(&mut buffer)?;

    let Some(header_end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") else {
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

    Ok(CapturedRequest {
        method,
        path,
        headers,
        body: buffer[body_start..].to_vec(),
    })
}
