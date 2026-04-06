#[path = "support/mock_http.rs"]
mod mock_http;

use openai_rust::{
    ErrorKind, OpenAI,
    resources::videos::{VideoContentVariant, VideoDownloadContentParams},
};

#[test]
fn download_content_preserves_variant_query_and_binary_semantics() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        binary_response(b"\x00video-bytes"),
        binary_response(b"\x00thumbnail-bytes"),
        binary_response(b"\x00spritesheet-bytes"),
    ])
    .unwrap();
    let client = client(&server.url());

    let default_video = client
        .videos()
        .download_content("vid_123", VideoDownloadContentParams::default())
        .unwrap();
    assert_eq!(default_video.output, b"\x00video-bytes");

    let thumbnail = client
        .videos()
        .download_content(
            "vid_123",
            VideoDownloadContentParams {
                variant: Some(VideoContentVariant::Thumbnail),
            },
        )
        .unwrap();
    assert_eq!(thumbnail.output, b"\x00thumbnail-bytes");

    let spritesheet = client
        .videos()
        .download_content(
            "vid_123",
            VideoDownloadContentParams {
                variant: Some(VideoContentVariant::Spritesheet),
            },
        )
        .unwrap();
    assert_eq!(spritesheet.output, b"\x00spritesheet-bytes");

    let requests = server.captured_requests(3).unwrap();
    assert_eq!(requests[0].path, "/v1/videos/vid_123/content");
    assert_eq!(
        requests[0].headers.get("accept").map(String::as_str),
        Some("application/binary")
    );
    assert_eq!(
        requests[1].path,
        "/v1/videos/vid_123/content?variant=thumbnail"
    );
    assert_eq!(
        requests[2].path,
        "/v1/videos/vid_123/content?variant=spritesheet"
    );

    let blank = client
        .videos()
        .download_content(" ", VideoDownloadContentParams::default())
        .unwrap_err();
    assert!(matches!(blank.kind, ErrorKind::Validation));
}

fn client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .build()
}

fn binary_response(body: &[u8]) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        headers: vec![
            (String::from("content-length"), body.len().to_string()),
            (
                String::from("content-type"),
                String::from("application/binary"),
            ),
            (
                String::from("x-request-id"),
                String::from("req_video_download"),
            ),
        ],
        body: body.to_vec(),
        ..Default::default()
    }
}
