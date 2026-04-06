#[path = "support/mock_http.rs"]
mod mock_http;
#[path = "support/multipart.rs"]
mod multipart_support;

use std::time::Duration;

use openai_rust::{
    DEFAULT_BASE_URL, ErrorKind, OpenAI,
    resources::videos::{
        VideoCharacter, VideoCreateCharacterParams, VideoCreateParams, VideoCreateReference,
        VideoCreateSeconds, VideoEditParams, VideoExtendParams, VideoExtendSeconds,
        VideoListParams, VideoModel, VideoOrder, VideoPollOptions, VideoReferenceAsset,
        VideoRemixParams, VideoSize, VideoSource, VideoStatus, VideoUpload,
    },
};
use serde_json::json;

#[test]
fn video_job_lifecycle_and_transforms_preserve_typed_ids_polling_and_character_flows() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(video_payload("vid_create", "queued", 5, None)),
        json_response(video_payload("vid_create", "in_progress", 42, None))
            .with_header("openai-poll-after-ms", "1"),
        json_response(video_payload("vid_create", "completed", 100, None)),
        json_response(video_payload("vid_create", "completed", 100, None)),
        json_response(video_list_payload()),
        json_response(video_payload(
            "vid_edit",
            "queued",
            0,
            Some(("vid_create", "prompt edit")),
        )),
        json_response(video_payload(
            "vid_upload_edit",
            "queued",
            0,
            Some(("vid_source", "upload edit")),
        )),
        json_response(video_payload(
            "vid_extend",
            "queued",
            0,
            Some(("vid_create", "prompt extend")),
        )),
        json_response(video_payload(
            "vid_upload_extend",
            "queued",
            0,
            Some(("vid_source", "upload extend")),
        )),
        json_response(video_payload(
            "vid_remix",
            "queued",
            0,
            Some(("vid_create", "prompt remix")),
        )),
        json_response(character_payload("char_123")),
        json_response(character_payload("char_123")),
        json_response(
            json!({
                "id": "vid_create",
                "object": "video.deleted",
                "deleted": true
            })
            .to_string(),
        ),
    ])
    .unwrap();
    let client = client(&server.url());

    let created = client
        .videos()
        .create(VideoCreateParams {
            prompt: String::from("a robot walking in the rain"),
            input_reference: Some(VideoCreateReference::Asset(VideoReferenceAsset::image_url(
                "https://example.com/reference.png",
            ))),
            model: Some(VideoModel::Sora2Pro),
            seconds: Some(VideoCreateSeconds::S8),
            size: Some(VideoSize::Landscape720),
        })
        .unwrap();
    assert_eq!(created.output.id, "vid_create");
    assert_eq!(created.output.status, VideoStatus::Queued);
    assert_eq!(created.output.progress, 5);

    let polled = client
        .videos()
        .create_and_poll(
            VideoCreateParams {
                prompt: String::from("a robot walking in the rain"),
                input_reference: Some(VideoCreateReference::Asset(VideoReferenceAsset::file_id(
                    "file_ref_123",
                ))),
                model: Some(VideoModel::Sora2),
                seconds: Some(VideoCreateSeconds::S4),
                size: Some(VideoSize::Portrait720),
            },
            VideoPollOptions {
                poll_interval: None,
                max_wait: Duration::from_secs(2),
            },
        )
        .unwrap();
    assert_eq!(polled.output.status, VideoStatus::Completed);
    assert_eq!(polled.output.progress, 100);

    let retrieved = client.videos().retrieve("vid_create").unwrap();
    assert_eq!(retrieved.output.id, "vid_create");
    assert_eq!(retrieved.output.expires_at, Some(1_717_172_999));

    let listed = client
        .videos()
        .list(VideoListParams {
            after: Some(String::from("vid_prev")),
            limit: Some(2),
            order: Some(VideoOrder::Asc),
        })
        .unwrap();
    assert_eq!(listed.output.data.len(), 2);
    assert_eq!(listed.output.next_after(), Some("vid_beta"));

    let edited = client
        .videos()
        .edit(VideoEditParams {
            prompt: String::from("prompt edit"),
            video: VideoSource::id("vid_create"),
        })
        .unwrap();
    assert_eq!(
        edited.output.remixed_from_video_id.as_deref(),
        Some("vid_create")
    );

    let upload_edit = client
        .videos()
        .edit(VideoEditParams {
            prompt: String::from("upload edit"),
            video: VideoSource::upload(VideoUpload::new(
                "edit.mp4",
                "video/mp4",
                b"edit-video".to_vec(),
            )),
        })
        .unwrap();
    assert_eq!(upload_edit.output.id, "vid_upload_edit");

    let extended = client
        .videos()
        .extend(VideoExtendParams {
            prompt: String::from("prompt extend"),
            seconds: VideoExtendSeconds::S16,
            video: VideoSource::id("vid_create"),
        })
        .unwrap();
    assert_eq!(extended.output.prompt.as_deref(), Some("prompt extend"));

    let upload_extend = client
        .videos()
        .extend(VideoExtendParams {
            prompt: String::from("upload extend"),
            seconds: VideoExtendSeconds::S20,
            video: VideoSource::upload(VideoUpload::new(
                "extend.mp4",
                "video/mp4",
                b"extend-video".to_vec(),
            )),
        })
        .unwrap();
    assert_eq!(upload_extend.output.id, "vid_upload_extend");

    let remixed = client
        .videos()
        .remix(
            "vid_create",
            VideoRemixParams {
                prompt: String::from("prompt remix"),
            },
        )
        .unwrap();
    assert_eq!(remixed.output.id, "vid_remix");

    let created_character = client
        .videos()
        .create_character(VideoCreateCharacterParams {
            name: String::from("Neo"),
            video: VideoUpload::new("character.mp4", "video/mp4", b"character".to_vec()),
        })
        .unwrap();
    assert_eq!(
        created_character.output,
        VideoCharacter {
            id: Some(String::from("char_123")),
            created_at: 1_717_171_919,
            name: Some(String::from("Neo")),
            extra: Default::default(),
        }
    );

    let retrieved_character = client.videos().get_character("char_123").unwrap();
    assert_eq!(retrieved_character.output.id.as_deref(), Some("char_123"));

    let deleted = client.videos().delete("vid_create").unwrap();
    assert!(deleted.output.deleted);

    let requests = server.captured_requests(13).unwrap();
    assert_eq!(requests[0].path, "/v1/videos");
    let create_content_type = requests[0].headers.get("content-type").unwrap();
    assert!(create_content_type.starts_with("multipart/form-data; boundary="));
    let create_boundary = create_content_type.split("boundary=").nth(1).unwrap();
    let create_multipart =
        multipart_support::parse_multipart(&requests[0].body, create_boundary).unwrap();
    assert_text_part(&create_multipart, "prompt", "a robot walking in the rain");
    assert_text_part(&create_multipart, "model", "sora-2-pro");
    assert_text_part(&create_multipart, "seconds", "8");
    assert_text_part(&create_multipart, "size", "1280x720");
    assert_text_part(
        &create_multipart,
        "input_reference[image_url]",
        "https://example.com/reference.png",
    );

    let create_and_poll_content_type = requests[1].headers.get("content-type").unwrap();
    assert!(create_and_poll_content_type.starts_with("multipart/form-data; boundary="));
    let create_and_poll_boundary = create_and_poll_content_type
        .split("boundary=")
        .nth(1)
        .unwrap();
    let create_and_poll_multipart =
        multipart_support::parse_multipart(&requests[1].body, create_and_poll_boundary).unwrap();
    assert_text_part(
        &create_and_poll_multipart,
        "prompt",
        "a robot walking in the rain",
    );
    assert_text_part(&create_and_poll_multipart, "model", "sora-2");
    assert_text_part(&create_and_poll_multipart, "seconds", "4");
    assert_text_part(&create_and_poll_multipart, "size", "720x1280");
    assert_text_part(
        &create_and_poll_multipart,
        "input_reference[file_id]",
        "file_ref_123",
    );
    assert_eq!(
        requests[2]
            .headers
            .get("x-stainless-poll-helper")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(requests[2].path, "/v1/videos/vid_create");
    assert_eq!(requests[3].path, "/v1/videos/vid_create");
    assert_eq!(
        requests[4].path,
        "/v1/videos?after=vid_prev&limit=2&order=asc"
    );

    assert_eq!(requests[5].path, "/v1/videos/edits");
    let edit_content_type = requests[5].headers.get("content-type").unwrap();
    assert!(edit_content_type.starts_with("multipart/form-data; boundary="));
    let edit_boundary = edit_content_type.split("boundary=").nth(1).unwrap();
    let edit_multipart =
        multipart_support::parse_multipart(&requests[5].body, edit_boundary).unwrap();
    assert_text_part(&edit_multipart, "prompt", "prompt edit");
    assert_text_part(&edit_multipart, "video[id]", "vid_create");

    assert_eq!(requests[6].path, "/v1/videos/edits");
    let edit_upload_content_type = requests[6].headers.get("content-type").unwrap();
    assert!(edit_upload_content_type.starts_with("multipart/form-data; boundary="));
    let edit_upload_boundary = edit_upload_content_type.split("boundary=").nth(1).unwrap();
    let edit_upload_multipart =
        multipart_support::parse_multipart(&requests[6].body, edit_upload_boundary).unwrap();
    assert_eq!(
        edit_upload_multipart.parts[0].name.as_deref(),
        Some("prompt")
    );
    assert_eq!(
        edit_upload_multipart.parts[1].name.as_deref(),
        Some("video")
    );
    assert_eq!(
        edit_upload_multipart.parts[1].filename.as_deref(),
        Some("edit.mp4")
    );

    assert_eq!(requests[7].path, "/v1/videos/extensions");
    let extend_content_type = requests[7].headers.get("content-type").unwrap();
    assert!(extend_content_type.starts_with("multipart/form-data; boundary="));
    let extend_boundary = extend_content_type.split("boundary=").nth(1).unwrap();
    let extend_multipart =
        multipart_support::parse_multipart(&requests[7].body, extend_boundary).unwrap();
    assert_text_part(&extend_multipart, "prompt", "prompt extend");
    assert_text_part(&extend_multipart, "seconds", "16");
    assert_text_part(&extend_multipart, "video[id]", "vid_create");

    let extend_upload_content_type = requests[8].headers.get("content-type").unwrap();
    let extend_upload_boundary = extend_upload_content_type
        .split("boundary=")
        .nth(1)
        .unwrap();
    let extend_upload_multipart =
        multipart_support::parse_multipart(&requests[8].body, extend_upload_boundary).unwrap();
    let extend_video_part = extend_upload_multipart
        .parts
        .iter()
        .find(|part| part.name.as_deref() == Some("video"))
        .unwrap();
    assert_eq!(extend_video_part.filename.as_deref(), Some("extend.mp4"));

    let remix_body: serde_json::Value = serde_json::from_slice(&requests[9].body).unwrap();
    assert_eq!(requests[9].path, "/v1/videos/vid_create/remix");
    assert_eq!(remix_body, json!({"prompt": "prompt remix"}));

    let character_content_type = requests[10].headers.get("content-type").unwrap();
    let character_boundary = character_content_type.split("boundary=").nth(1).unwrap();
    let character_multipart =
        multipart_support::parse_multipart(&requests[10].body, character_boundary).unwrap();
    assert_eq!(requests[10].path, "/v1/videos/characters");
    assert_eq!(character_multipart.parts[0].body, b"Neo");
    assert_eq!(
        character_multipart.parts[1].filename.as_deref(),
        Some("character.mp4")
    );
    assert_eq!(requests[11].path, "/v1/videos/characters/char_123");
    assert_eq!(requests[12].path, "/v1/videos/vid_create");

    let blank = client.videos().retrieve(" ").unwrap_err();
    assert!(matches!(blank.kind, ErrorKind::Validation));
}

#[test]
fn create_request_shape_preserves_nested_reference_configuration_without_flattening() {
    let server = mock_http::MockHttpServer::spawn(json_response(video_payload(
        "vid_create",
        "queued",
        0,
        None,
    )))
    .unwrap();
    let client = client(&server.url());

    client
        .videos()
        .create(VideoCreateParams {
            prompt: String::from("with nested input reference"),
            input_reference: Some(VideoCreateReference::Asset(VideoReferenceAsset {
                file_id: Some(String::from("file_nested")),
                image_url: Some(String::from("data:image/png;base64,AAAA")),
            })),
            model: Some(VideoModel::Custom(String::from("sora-2-2025-12-08"))),
            seconds: Some(VideoCreateSeconds::S12),
            size: Some(VideoSize::Portrait1024),
        })
        .unwrap();

    let request = server.captured_request().unwrap();
    assert_eq!(request.path, "/v1/videos");
    let content_type = request.headers.get("content-type").unwrap();
    assert!(content_type.starts_with("multipart/form-data; boundary="));
    let boundary = content_type.split("boundary=").nth(1).unwrap();
    let multipart = multipart_support::parse_multipart(&request.body, boundary).unwrap();
    assert_text_part(&multipart, "prompt", "with nested input reference");
    assert_text_part(&multipart, "input_reference[file_id]", "file_nested");
    assert_text_part(
        &multipart,
        "input_reference[image_url]",
        "data:image/png;base64,AAAA",
    );
    assert_text_part(&multipart, "model", "sora-2-2025-12-08");
    assert_text_part(&multipart, "seconds", "12");
    assert_text_part(&multipart, "size", "1024x1792");
}

fn assert_text_part(multipart: &multipart_support::ParsedMultipart, name: &str, value: &str) {
    let part = multipart
        .parts
        .iter()
        .find(|part| part.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("missing multipart text part `{name}`"));
    assert_eq!(part.body, value.as_bytes());
}

#[test]
#[ignore = "requires live OpenAI credentials"]
fn live_videos_smoke_skips_explicitly_when_the_project_lacks_entitlement() {
    let client = OpenAI::builder().build();
    let resolved = client
        .resolved_config()
        .expect("live videos client should resolve configuration");
    assert_eq!(resolved.base_url, DEFAULT_BASE_URL);

    match client.videos().list(VideoListParams::default()) {
        Ok(response) => {
            println!(
                "live videos list request id: {}",
                response.request_id().unwrap_or("<missing>")
            );
        }
        Err(error) => {
            if let Some(reason) = entitlement_skip_reason(&error) {
                println!("videos live smoke skipped: {reason}");
                return;
            }
            panic!("live videos smoke should succeed or skip explicitly: {error}");
        }
    }
}

fn entitlement_skip_reason(error: &openai_rust::OpenAIError) -> Option<String> {
    let status = error.status_code()?;
    let payload = error.api_error()?;
    let code = payload.code.as_deref().unwrap_or("<missing-code>");
    let message = payload.message.trim();
    match status {
        403 | 404 => Some(format!("status={status}, code={code}, message={message}")),
        _ => None,
    }
}

fn client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .build()
}

fn video_payload(id: &str, status: &str, progress: u64, remixed: Option<(&str, &str)>) -> String {
    json!({
        "id": id,
        "object": "video",
        "created_at": 1_717_171_717u64,
        "completed_at": if status == "completed" { json!(1_717_171_818u64) } else { json!(null) },
        "error": null,
        "expires_at": 1_717_172_999u64,
        "model": "sora-2",
        "progress": progress,
        "prompt": remixed.map(|(_, prompt)| prompt).unwrap_or("a robot walking in the rain"),
        "remixed_from_video_id": remixed.map(|(id, _)| id),
        "seconds": "8",
        "size": "1280x720",
        "status": status
    })
    .to_string()
}

fn video_list_payload() -> String {
    json!({
        "object": "list",
        "data": [
            serde_json::from_str::<serde_json::Value>(&video_payload("vid_create", "completed", 100, None)).unwrap(),
            serde_json::from_str::<serde_json::Value>(&video_payload("vid_beta", "failed", 100, Some(("vid_alpha", "beta")))).unwrap()
        ],
        "first_id": "vid_create",
        "last_id": "vid_beta",
        "has_more": true
    }).to_string()
}

fn character_payload(id: &str) -> String {
    json!({
        "id": id,
        "created_at": 1_717_171_919u64,
        "name": "Neo"
    })
    .to_string()
}

fn json_response(body: String) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        headers: vec![
            (String::from("content-length"), body.len().to_string()),
            (
                String::from("content-type"),
                String::from("application/json"),
            ),
            (String::from("x-request-id"), String::from("req_videos")),
        ],
        body: body.into_bytes(),
        ..Default::default()
    }
}

trait ResponseHeaderExt {
    fn with_header(self, name: &str, value: &str) -> Self;
}

impl ResponseHeaderExt for mock_http::ScriptedResponse {
    fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((String::from(name), String::from(value)));
        self
    }
}
