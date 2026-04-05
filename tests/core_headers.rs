use std::{
    io::{Read, Write},
    net::{Shutdown, TcpStream},
};

use openai_rust::OpenAI;
use url::Url;

#[path = "support/mock_http.rs"]
mod mock_http;

#[test]
fn auth_org_and_project_headers_are_conditional() {
    let configured_server =
        mock_http::MockHttpServer::spawn(mock_http::ScriptedResponse::default()).unwrap();
    let configured_client = OpenAI::builder()
        .api_key("configured-key")
        .base_url(configured_server.url())
        .organization("org_123")
        .project("proj_456")
        .build();

    let configured_request = configured_client.prepare_request("GET", "/models").unwrap();
    send_prepared_request(&configured_request).unwrap();
    let configured_capture = configured_server.captured_request().unwrap();

    assert_eq!(
        configured_capture
            .headers
            .get("authorization")
            .map(String::as_str),
        Some("Bearer configured-key")
    );
    assert_eq!(
        configured_capture
            .headers
            .get("openai-organization")
            .map(String::as_str),
        Some("org_123")
    );
    assert_eq!(
        configured_capture
            .headers
            .get("openai-project")
            .map(String::as_str),
        Some("proj_456")
    );

    let plain_server =
        mock_http::MockHttpServer::spawn(mock_http::ScriptedResponse::default()).unwrap();
    let plain_client = OpenAI::builder()
        .api_key("plain-key")
        .base_url(plain_server.url())
        .build();

    let plain_request = plain_client.prepare_request("GET", "/models").unwrap();
    send_prepared_request(&plain_request).unwrap();
    let plain_capture = plain_server.captured_request().unwrap();

    assert_eq!(
        plain_capture
            .headers
            .get("authorization")
            .map(String::as_str),
        Some("Bearer plain-key")
    );
    assert!(!plain_capture.headers.contains_key("openai-organization"));
    assert!(!plain_capture.headers.contains_key("openai-project"));
}

#[test]
fn user_agent_defaults_and_overrides() {
    let default_server =
        mock_http::MockHttpServer::spawn(mock_http::ScriptedResponse::default()).unwrap();
    let default_request = OpenAI::builder()
        .api_key("default-key")
        .base_url(default_server.url())
        .build()
        .prepare_request("GET", "/models")
        .unwrap();
    send_prepared_request(&default_request).unwrap();
    let default_capture = default_server.captured_request().unwrap();

    let default_user_agent = default_capture.headers.get("user-agent").cloned().unwrap();
    assert_eq!(
        default_user_agent,
        format!("openai-rust/{}", env!("CARGO_PKG_VERSION"))
    );

    let custom_server =
        mock_http::MockHttpServer::spawn(mock_http::ScriptedResponse::default()).unwrap();
    let custom_request = OpenAI::builder()
        .api_key("custom-key")
        .base_url(custom_server.url())
        .user_agent("integration-suite/1.0")
        .build()
        .prepare_request("GET", "/models")
        .unwrap();
    send_prepared_request(&custom_request).unwrap();
    let custom_capture = custom_server.captured_request().unwrap();

    let custom_user_agent = custom_capture.headers.get("user-agent").cloned().unwrap();
    println!("captured user-agent: {custom_user_agent}");
    assert_eq!(
        custom_user_agent,
        format!(
            "integration-suite/1.0 openai-rust/{}",
            env!("CARGO_PKG_VERSION")
        )
    );
}

fn send_prepared_request(
    request: &openai_rust::core::request::PreparedRequest,
) -> std::io::Result<()> {
    let url = Url::parse(&request.url).unwrap();
    let host = url.host_str().unwrap();
    let port = url.port_or_known_default().unwrap();
    let mut stream = TcpStream::connect((host, port))?;

    let mut raw = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
        request.method,
        url.path(),
        host
    );
    for (name, value) in &request.headers {
        raw.push_str(name);
        raw.push_str(": ");
        raw.push_str(value);
        raw.push_str("\r\n");
    }
    raw.push_str("\r\n");

    stream.write_all(raw.as_bytes())?;
    stream.shutdown(Shutdown::Write)?;

    let mut response = Vec::new();
    let _ = stream.read_to_end(&mut response);
    Ok(())
}
