use std::{
    io::Write,
    net::{Shutdown, TcpStream},
    sync::{Mutex, OnceLock},
};

use openai_rust::{DEFAULT_BASE_URL, ErrorKind, OpenAI};
use url::Url;

#[path = "support/mock_http.rs"]
mod mock_http;

static ENV_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

#[test]
fn missing_credentials_fail_before_transport_attempts() {
    let server = mock_http::MockHttpServer::spawn(mock_http::ScriptedResponse::default()).unwrap();

    with_env(
        &[
            ("OPENAI_API_KEY", None),
            ("OPENAI_BASE_URL", Some(server.url().as_str())),
            ("OPENAI_ORG_ID", None),
            ("OPENAI_PROJECT_ID", None),
        ],
        || {
            let error = OpenAI::new().prepare_request("GET", "/models").unwrap_err();
            assert_eq!(error.kind, ErrorKind::Configuration);
            assert!(error.message.contains("OPENAI_API_KEY"));
            assert_eq!(server.captured_request(), None);
        },
    );
}

#[test]
fn explicit_overrides_win_over_environment_values() {
    let server = mock_http::MockHttpServer::spawn(mock_http::ScriptedResponse::default()).unwrap();

    with_env(
        &[
            ("OPENAI_API_KEY", Some("env-key")),
            ("OPENAI_BASE_URL", Some("https://example.invalid/v1")),
            ("OPENAI_ORG_ID", Some("env-org")),
            ("OPENAI_PROJECT_ID", Some("env-project")),
        ],
        || {
            let client = OpenAI::builder()
                .api_key("explicit-key")
                .base_url(server.url())
                .organization("explicit-org")
                .project("explicit-project")
                .build();

            let resolved = client.resolved_config().unwrap();
            assert_eq!(resolved.api_key, "explicit-key");
            assert_eq!(resolved.base_url, server.url());
            assert_eq!(resolved.organization.as_deref(), Some("explicit-org"));
            assert_eq!(resolved.project.as_deref(), Some("explicit-project"));

            let request = client.prepare_request("GET", "/models").unwrap();
            assert_eq!(request.url, format!("{}/v1/models", server.url()));
            assert_eq!(
                request.headers.get("authorization").map(String::as_str),
                Some("Bearer explicit-key")
            );
            assert_eq!(
                request
                    .headers
                    .get("openai-organization")
                    .map(String::as_str),
                Some("explicit-org")
            );
            assert_eq!(
                request.headers.get("openai-project").map(String::as_str),
                Some("explicit-project")
            );
        },
    );
}

#[test]
fn default_base_url_is_used_for_missing_or_blank_overrides() {
    with_env(
        &[
            ("OPENAI_API_KEY", Some("env-key")),
            ("OPENAI_BASE_URL", None),
            ("OPENAI_ORG_ID", None),
            ("OPENAI_PROJECT_ID", None),
        ],
        || {
            let default_client = OpenAI::new();
            let default_request = default_client.prepare_request("GET", "/responses").unwrap();
            assert_eq!(
                default_client.resolved_config().unwrap().base_url,
                DEFAULT_BASE_URL
            );
            assert_eq!(default_request.url, "https://api.openai.com/v1/responses");
        },
    );

    with_env(
        &[
            ("OPENAI_API_KEY", Some("env-key")),
            ("OPENAI_BASE_URL", Some("   ")),
            ("OPENAI_ORG_ID", None),
            ("OPENAI_PROJECT_ID", None),
        ],
        || {
            let blank_env_request = OpenAI::new().prepare_request("GET", "v1/models").unwrap();
            assert_eq!(blank_env_request.url, "https://api.openai.com/v1/models");
        },
    );

    let explicit_blank_request = OpenAI::builder()
        .api_key("explicit-key")
        .base_url("   ")
        .build()
        .prepare_request("GET", "models")
        .unwrap();
    assert_eq!(
        explicit_blank_request.url,
        "https://api.openai.com/v1/models"
    );
}

#[test]
fn blank_api_key_values_are_rejected_before_network() {
    let server = mock_http::MockHttpServer::spawn(mock_http::ScriptedResponse::default()).unwrap();

    for api_key in ["", "   "] {
        let error = OpenAI::builder()
            .api_key(api_key)
            .base_url(server.url())
            .build()
            .prepare_request("GET", "/models")
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::Configuration);
        assert!(error.message.contains("OpenAI API key"));
    }

    assert_eq!(server.captured_request(), None);
}

#[test]
fn env_loaded_request_can_be_sent_to_a_loopback_server() {
    let server = mock_http::MockHttpServer::spawn(mock_http::ScriptedResponse::default()).unwrap();

    with_env(
        &[
            ("OPENAI_API_KEY", Some("env-key")),
            ("OPENAI_BASE_URL", Some(server.url().as_str())),
            ("OPENAI_ORG_ID", Some("env-org")),
            ("OPENAI_PROJECT_ID", Some("env-project")),
        ],
        || {
            let request = OpenAI::new().prepare_request("GET", "/models").unwrap();
            send_prepared_request(&request).unwrap();

            let captured = server.captured_request().unwrap();
            assert_eq!(captured.path, "/v1/models");
            assert_eq!(
                captured.headers.get("authorization").map(String::as_str),
                Some("Bearer env-key")
            );
            assert_eq!(
                captured
                    .headers
                    .get("openai-organization")
                    .map(String::as_str),
                Some("env-org")
            );
            assert_eq!(
                captured.headers.get("openai-project").map(String::as_str),
                Some("env-project")
            );
        },
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
    Ok(())
}

fn with_env(vars: &[(&str, Option<&str>)], test: impl FnOnce()) {
    let _guard = ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();
    let previous = vars
        .iter()
        .map(|(key, _)| ((*key).to_string(), std::env::var(key).ok()))
        .collect::<Vec<_>>();

    for (key, value) in vars {
        match value {
            Some(value) => unsafe { std::env::set_var(key, value) },
            None => unsafe { std::env::remove_var(key) },
        }
    }

    test();

    for (key, value) in previous {
        match value {
            Some(value) => unsafe { std::env::set_var(&key, value) },
            None => unsafe { std::env::remove_var(&key) },
        }
    }
}
