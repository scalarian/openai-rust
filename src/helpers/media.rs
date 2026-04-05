use serde::de::DeserializeOwned;

use crate::{
    OpenAIError,
    core::{metadata::ResponseMetadata, response::ApiResponse},
    error::ErrorKind,
    helpers::sse::{SseFrame, SseParser},
};

/// Shared decode mode for media-family response bodies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MediaDecodeMode {
    Json,
    Text,
    Binary,
    Sse,
}

/// Shared decoded media body.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DecodedMedia<T> {
    Json(T),
    Text(String),
    Binary(Vec<u8>),
    Sse { text: String, frames: Vec<SseFrame> },
}

pub(crate) fn decode_media_response<T>(
    response: ApiResponse<Vec<u8>>,
    mode: MediaDecodeMode,
    label: &str,
) -> Result<ApiResponse<DecodedMedia<T>>, OpenAIError>
where
    T: DeserializeOwned,
{
    let (metadata, body) = response.into_parts();
    let output = match mode {
        MediaDecodeMode::Json => DecodedMedia::Json(parse_json_body(&body, &metadata)?),
        MediaDecodeMode::Text => DecodedMedia::Text(parse_text_body(&body, &metadata, label)?),
        MediaDecodeMode::Binary => DecodedMedia::Binary(body),
        MediaDecodeMode::Sse => {
            let text = parse_text_body(&body, &metadata, label)?;
            let frames = parse_sse_frames([text.as_str()])?;
            DecodedMedia::Sse { text, frames }
        }
    };
    Ok(ApiResponse { output, metadata })
}

pub(crate) fn parse_sse_frames<I, B>(chunks: I) -> Result<Vec<SseFrame>, OpenAIError>
where
    I: IntoIterator<Item = B>,
    B: AsRef<str>,
{
    let mut parser = SseParser::default();
    let mut frames = Vec::new();
    for chunk in chunks {
        frames.extend(parser.push(chunk.as_ref().as_bytes())?);
    }
    frames.extend(parser.finish()?);
    Ok(frames)
}

fn parse_json_body<T>(body: &[u8], metadata: &ResponseMetadata) -> Result<T, OpenAIError>
where
    T: DeserializeOwned,
{
    serde_json::from_slice(body).map_err(|error| {
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
    })
}

fn parse_text_body(
    body: &[u8],
    metadata: &ResponseMetadata,
    label: &str,
) -> Result<String, OpenAIError> {
    String::from_utf8(body.to_vec()).map_err(|error| {
        OpenAIError::new(
            ErrorKind::Parse,
            format!("failed to decode {label} response as UTF-8: {error}"),
        )
        .with_response_metadata(
            metadata.status_code,
            metadata.headers.clone(),
            metadata.request_id.clone(),
        )
        .with_source(error)
    })
}
