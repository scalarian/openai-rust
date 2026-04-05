use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{Shutdown, TcpStream},
    thread,
    time::Duration,
};

use reqwest::{Client, Response as AsyncResponse, blocking::Client as BlockingClient};
use serde::de::DeserializeOwned;
use tokio::time::sleep;
use url::Url;

use crate::{
    core::{
        metadata::ResponseMetadata,
        request::{PreparedRequest, ResolvedRequestOptions},
        response::ApiResponse,
    },
    error::{ApiErrorKind, ApiErrorPayload, ErrorKind, OpenAIError},
};

pub(crate) fn execute_json<T>(
    request: &PreparedRequest,
    options: &ResolvedRequestOptions,
) -> Result<ApiResponse<T>, OpenAIError>
where
    T: DeserializeOwned,
{
    let client = build_blocking_client(options)?;

    let mut last_error = None;
    for attempt in 0..=options.max_retries {
        match execute_once_json(&client, request, options) {
            Ok(response) => return Ok(response),
            Err(error) => {
                let should_retry = attempt < options.max_retries && should_retry(&error);
                let retry_delay = retry_delay(&error, attempt);
                last_error = Some(error);
                if should_retry {
                    thread::sleep(retry_delay);
                    continue;
                }
                break;
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        OpenAIError::new(
            ErrorKind::Transport,
            "shared transport exited without a response or error",
        )
    }))
}

pub(crate) fn execute_unit(
    request: &PreparedRequest,
    options: &ResolvedRequestOptions,
) -> Result<ApiResponse<()>, OpenAIError> {
    let client = build_blocking_client(options)?;

    let mut last_error = None;
    for attempt in 0..=options.max_retries {
        match execute_once_unit(&client, request, options) {
            Ok(response) => return Ok(response),
            Err(error) => {
                let should_retry = attempt < options.max_retries && should_retry(&error);
                let retry_delay = retry_delay(&error, attempt);
                last_error = Some(error);
                if should_retry {
                    thread::sleep(retry_delay);
                    continue;
                }
                break;
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        OpenAIError::new(
            ErrorKind::Transport,
            "shared transport exited without a response or error",
        )
    }))
}

pub(crate) fn execute_bytes(
    request: &PreparedRequest,
    options: &ResolvedRequestOptions,
) -> Result<ApiResponse<Vec<u8>>, OpenAIError> {
    let client = build_blocking_client(options)?;

    let mut last_error = None;
    for attempt in 0..=options.max_retries {
        match execute_once_bytes(&client, request, options) {
            Ok(response) => {
                return Ok(ApiResponse {
                    output: response.body,
                    metadata: response.metadata,
                });
            }
            Err(error) => {
                let should_retry = attempt < options.max_retries && should_retry(&error);
                let retry_delay = retry_delay(&error, attempt);
                last_error = Some(error);
                if should_retry {
                    thread::sleep(retry_delay);
                    continue;
                }
                break;
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        OpenAIError::new(
            ErrorKind::Transport,
            "shared transport exited without a response or error",
        )
    }))
}

#[allow(dead_code)]
pub(crate) fn execute_text(
    request: &PreparedRequest,
    options: &ResolvedRequestOptions,
) -> Result<ApiResponse<String>, OpenAIError> {
    let client = build_blocking_client(options)?;

    let mut last_error = None;
    for attempt in 0..=options.max_retries {
        match execute_once_text(&client, request, options) {
            Ok(response) => return Ok(response),
            Err(error) => {
                let should_retry = attempt < options.max_retries && should_retry(&error);
                let retry_delay = retry_delay(&error, attempt);
                last_error = Some(error);
                if should_retry {
                    thread::sleep(retry_delay);
                    continue;
                }
                break;
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        OpenAIError::new(
            ErrorKind::Transport,
            "shared transport exited without a response or error",
        )
    }))
}

pub(crate) struct StreamingTextResponse {
    pub metadata: ResponseMetadata,
    pub response: AsyncResponse,
}

pub(crate) async fn execute_text_stream(
    request: &PreparedRequest,
    options: &ResolvedRequestOptions,
) -> Result<StreamingTextResponse, OpenAIError> {
    let client = build_async_client(options)?;

    let mut last_error = None;
    for attempt in 0..=options.max_retries {
        match execute_once_text_stream(&client, request).await {
            Ok(response) => return Ok(response),
            Err(error) => {
                let should_retry = attempt < options.max_retries && should_retry(&error);
                let retry_delay = retry_delay(&error, attempt);
                last_error = Some(error);
                if should_retry {
                    sleep(retry_delay).await;
                    continue;
                }
                break;
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        OpenAIError::new(
            ErrorKind::Transport,
            "shared transport exited without a response or error",
        )
    }))
}

fn build_blocking_client(options: &ResolvedRequestOptions) -> Result<BlockingClient, OpenAIError> {
    BlockingClient::builder()
        .timeout(options.timeout)
        .build()
        .map_err(|error| {
            OpenAIError::new(
                ErrorKind::Transport,
                format!("failed to build shared HTTP client: {error}"),
            )
            .with_source(error)
        })
}

fn build_async_client(options: &ResolvedRequestOptions) -> Result<Client, OpenAIError> {
    Client::builder()
        .timeout(options.timeout)
        .build()
        .map_err(|error| {
            OpenAIError::new(
                ErrorKind::Transport,
                format!("failed to build shared HTTP client: {error}"),
            )
            .with_source(error)
        })
}

fn execute_once_json<T>(
    client: &BlockingClient,
    request: &PreparedRequest,
    options: &ResolvedRequestOptions,
) -> Result<ApiResponse<T>, OpenAIError>
where
    T: DeserializeOwned,
{
    let response = execute_once_bytes(client, request, options)?;
    let ResponseBytes { metadata, body } = response;
    let output = serde_json::from_slice::<T>(&body).map_err(|error| {
        OpenAIError::new(
            ErrorKind::Parse,
            format!("failed to parse OpenAI success response: {error}"),
        )
        .with_response_metadata(
            metadata.status_code,
            metadata.headers.clone(),
            metadata.request_id.clone(),
        )
        .with_source(error)
    })?;

    Ok(ApiResponse { output, metadata })
}

fn execute_once_unit(
    client: &BlockingClient,
    request: &PreparedRequest,
    options: &ResolvedRequestOptions,
) -> Result<ApiResponse<()>, OpenAIError> {
    let response = execute_once_bytes(client, request, options)?;
    Ok(ApiResponse {
        output: (),
        metadata: response.metadata,
    })
}

#[allow(dead_code)]
fn execute_once_text(
    client: &BlockingClient,
    request: &PreparedRequest,
    options: &ResolvedRequestOptions,
) -> Result<ApiResponse<String>, OpenAIError> {
    let response = execute_once_bytes(client, request, options)?;
    let ResponseBytes { metadata, body } = response;
    let output = String::from_utf8(body).map_err(|error| {
        OpenAIError::new(
            ErrorKind::Parse,
            format!("failed to decode OpenAI text response as UTF-8: {error}"),
        )
        .with_response_metadata(
            metadata.status_code,
            metadata.headers.clone(),
            metadata.request_id.clone(),
        )
        .with_source(error)
    })?;

    Ok(ApiResponse { output, metadata })
}

struct ResponseBytes {
    metadata: ResponseMetadata,
    body: Vec<u8>,
}

fn execute_once_bytes(
    client: &BlockingClient,
    request: &PreparedRequest,
    options: &ResolvedRequestOptions,
) -> Result<ResponseBytes, OpenAIError> {
    let url = Url::parse(&request.url).map_err(|error| {
        OpenAIError::new(
            ErrorKind::Validation,
            format!("invalid request URL `{}`: {error}", request.url),
        )
        .with_source(error)
    })?;

    if url.scheme() == "http" {
        return execute_http_loopback(&url, request, options.timeout);
    }

    let mut builder = client.request(parse_method(&request.method)?, &request.url);
    for (name, value) in &request.headers {
        builder = builder.header(name.as_str(), value.as_str());
    }
    if let Some(body) = &request.body {
        builder = builder.body(body.clone());
    }

    let response = builder.send().map_err(map_transport_error)?;
    let status = response.status();
    let headers = normalize_headers(response.headers());
    let request_id = headers.get("x-request-id").cloned();
    let status_code = status.as_u16();
    let body = response.bytes().map_err(map_transport_error)?.to_vec();

    if !status.is_success() {
        let mut error = OpenAIError::new(
            ErrorKind::Api(classify_status(status_code)),
            format!("OpenAI API request failed with status {status_code}"),
        )
        .with_response_metadata(status_code, headers.clone(), request_id);

        if let Ok(payload) = parse_api_error_payload(&body) {
            error = error.with_api_error(payload);
        }

        return Err(error);
    }

    Ok(ResponseBytes {
        metadata: ResponseMetadata {
            status_code,
            headers,
            request_id,
        },
        body,
    })
}

async fn execute_once_text_stream(
    client: &Client,
    request: &PreparedRequest,
) -> Result<StreamingTextResponse, OpenAIError> {
    let mut builder = client.request(parse_method(&request.method)?, &request.url);
    for (name, value) in &request.headers {
        builder = builder.header(name.as_str(), value.as_str());
    }
    if let Some(body) = &request.body {
        builder = builder.body(body.clone());
    }

    let response = builder.send().await.map_err(map_transport_error)?;
    let status = response.status();
    let headers = normalize_headers(response.headers());
    let request_id = headers.get("x-request-id").cloned();
    let status_code = status.as_u16();

    if !status.is_success() {
        let body = response
            .bytes()
            .await
            .map_err(map_transport_error)?
            .to_vec();
        let mut error = OpenAIError::new(
            ErrorKind::Api(classify_status(status_code)),
            format!("OpenAI API request failed with status {status_code}"),
        )
        .with_response_metadata(status_code, headers.clone(), request_id);

        if let Ok(payload) = parse_api_error_payload(&body) {
            error = error.with_api_error(payload);
        }

        return Err(error);
    }

    Ok(StreamingTextResponse {
        metadata: ResponseMetadata {
            status_code,
            headers,
            request_id,
        },
        response,
    })
}

fn execute_http_loopback(
    url: &Url,
    request: &PreparedRequest,
    timeout: Duration,
) -> Result<ResponseBytes, OpenAIError> {
    let host = url.host_str().ok_or_else(|| {
        OpenAIError::new(
            ErrorKind::Validation,
            format!("request URL `{}` is missing a host", request.url),
        )
    })?;
    let port = url.port_or_known_default().ok_or_else(|| {
        OpenAIError::new(
            ErrorKind::Validation,
            format!("request URL `{}` is missing a port", request.url),
        )
    })?;

    let mut stream = TcpStream::connect((host, port)).map_err(map_io_error)?;
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));
    let request_target = match url.query() {
        Some(query) => format!("{}?{}", url.path(), query),
        None => url.path().to_string(),
    };
    let mut raw_request = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
        request.method, request_target, host
    );
    for (name, value) in &request.headers {
        raw_request.push_str(name);
        raw_request.push_str(": ");
        raw_request.push_str(value);
        raw_request.push_str("\r\n");
    }
    if let Some(body) = &request.body {
        raw_request.push_str("content-length: ");
        raw_request.push_str(&body.len().to_string());
        raw_request.push_str("\r\n\r\n");
        let mut request_bytes = raw_request.into_bytes();
        request_bytes.extend_from_slice(body);
        stream.write_all(&request_bytes).map_err(map_io_error)?;
    } else {
        raw_request.push_str("\r\n");
        stream
            .write_all(raw_request.as_bytes())
            .map_err(map_io_error)?;
    }
    stream.flush().map_err(map_io_error)?;
    let _ = stream.shutdown(Shutdown::Write);

    let mut raw_response = Vec::new();
    stream
        .read_to_end(&mut raw_response)
        .map_err(map_io_error)?;

    parse_raw_http_response(&raw_response)
}

fn map_io_error(error: std::io::Error) -> OpenAIError {
    let kind = match error.kind() {
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => ErrorKind::Timeout,
        _ => ErrorKind::Transport,
    };
    OpenAIError::new(kind, error.to_string()).with_source(error)
}

fn parse_raw_http_response(raw_response: &[u8]) -> Result<ResponseBytes, OpenAIError> {
    let Some(header_end) = raw_response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
    else {
        return Err(OpenAIError::new(
            ErrorKind::Transport,
            "received malformed HTTP response without header terminator",
        ));
    };

    let body_start = header_end + 4;
    let header_text = String::from_utf8_lossy(&raw_response[..header_end]);
    let mut lines = header_text.lines();
    let status_line = lines.next().unwrap_or_default();
    let mut status_parts = status_line.split_whitespace();
    let _http_version = status_parts.next();
    let status_code = status_parts
        .next()
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| {
            OpenAIError::new(
                ErrorKind::Transport,
                format!("received malformed HTTP status line `{status_line}`"),
            )
        })?;

    let mut headers = BTreeMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    let request_id = headers.get("x-request-id").cloned();
    let body = decode_http_body(&raw_response[body_start..], &headers)?;

    if !(200..300).contains(&status_code) {
        let mut error = OpenAIError::new(
            ErrorKind::Api(classify_status(status_code)),
            format!("OpenAI API request failed with status {status_code}"),
        )
        .with_response_metadata(status_code, headers.clone(), request_id);

        if let Ok(payload) = parse_api_error_payload(&body) {
            error = error.with_api_error(payload);
        }

        return Err(error);
    }

    Ok(ResponseBytes {
        metadata: ResponseMetadata {
            status_code,
            headers,
            request_id,
        },
        body,
    })
}

fn decode_http_body(
    body: &[u8],
    headers: &BTreeMap<String, String>,
) -> Result<Vec<u8>, OpenAIError> {
    let is_chunked = headers
        .get("transfer-encoding")
        .map(|value| {
            value
                .split(',')
                .any(|entry| entry.trim().eq_ignore_ascii_case("chunked"))
        })
        .unwrap_or(false);

    if is_chunked {
        decode_chunked_body(body)
    } else {
        Ok(body.to_vec())
    }
}

fn decode_chunked_body(mut body: &[u8]) -> Result<Vec<u8>, OpenAIError> {
    let mut decoded = Vec::new();

    loop {
        let Some(line_end) = body.windows(2).position(|window| window == b"\r\n") else {
            return Err(OpenAIError::new(
                ErrorKind::Transport,
                "received malformed chunked HTTP response without chunk size terminator",
            ));
        };

        let size_line = std::str::from_utf8(&body[..line_end]).map_err(|error| {
            OpenAIError::new(
                ErrorKind::Transport,
                format!("received malformed chunked HTTP response size line: {error}"),
            )
            .with_source(error)
        })?;
        let size_hex = size_line.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size_hex, 16).map_err(|error| {
            OpenAIError::new(
                ErrorKind::Transport,
                format!("received malformed chunked HTTP response size `{size_line}`: {error}"),
            )
            .with_source(error)
        })?;
        body = &body[line_end + 2..];

        if size == 0 {
            return Ok(decoded);
        }

        if body.len() < size + 2 || &body[size..size + 2] != b"\r\n" {
            return Err(OpenAIError::new(
                ErrorKind::Transport,
                "received malformed chunked HTTP response body",
            ));
        }

        decoded.extend_from_slice(&body[..size]);
        body = &body[size + 2..];
    }
}

fn parse_method(method: &str) -> Result<reqwest::Method, OpenAIError> {
    reqwest::Method::from_bytes(method.as_bytes()).map_err(|error| {
        OpenAIError::new(
            ErrorKind::Validation,
            format!("invalid HTTP method `{method}`: {error}"),
        )
        .with_source(error)
    })
}

fn map_transport_error(error: reqwest::Error) -> OpenAIError {
    let kind = if error.is_timeout() {
        ErrorKind::Timeout
    } else {
        ErrorKind::Transport
    };
    OpenAIError::new(kind, error.to_string()).with_source(error)
}

fn parse_api_error_payload(body: &[u8]) -> Result<ApiErrorPayload, serde_json::Error> {
    #[derive(serde::Deserialize)]
    struct Envelope {
        error: ApiErrorPayload,
    }

    serde_json::from_slice::<Envelope>(body).map(|envelope| envelope.error)
}

fn normalize_headers(headers: &reqwest::header::HeaderMap) -> BTreeMap<String, String> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_ascii_lowercase(), value.to_string()))
        })
        .collect()
}

fn classify_status(status_code: u16) -> ApiErrorKind {
    match status_code {
        400 => ApiErrorKind::BadRequest,
        401 => ApiErrorKind::Authentication,
        403 => ApiErrorKind::PermissionDenied,
        404 => ApiErrorKind::NotFound,
        409 => ApiErrorKind::Conflict,
        422 => ApiErrorKind::UnprocessableEntity,
        429 => ApiErrorKind::RateLimit,
        500..=599 => ApiErrorKind::Server,
        other => ApiErrorKind::Other(other),
    }
}

fn should_retry(error: &OpenAIError) -> bool {
    if let Some(header) = error.header("x-should-retry") {
        return matches!(
            header.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes"
        );
    }

    match error.kind {
        ErrorKind::Timeout | ErrorKind::Transport => true,
        ErrorKind::Api(ApiErrorKind::Conflict)
        | ErrorKind::Api(ApiErrorKind::RateLimit)
        | ErrorKind::Api(ApiErrorKind::Server) => true,
        ErrorKind::Api(ApiErrorKind::Other(408)) => true,
        ErrorKind::Api(_) | ErrorKind::Configuration | ErrorKind::Validation | ErrorKind::Parse => {
            false
        }
    }
}

fn retry_delay(error: &OpenAIError, attempt: u32) -> Duration {
    if let Some(header) = error.header("retry-after-ms") {
        if let Ok(milliseconds) = header.trim().parse::<u64>() {
            return Duration::from_millis(milliseconds);
        }
    }

    if let Some(header) = error.header("retry-after") {
        if let Ok(seconds) = header.trim().parse::<u64>() {
            return Duration::from_secs(seconds);
        }
    }

    let exponent = attempt.min(4);
    Duration::from_millis(100 * (1_u64 << exponent))
}
