use std::{fs, path::Path};

use openai_rust::{
    ApiResponse, OpenAI, ResponseMetadata,
    resources::{
        chat::ChatCompletionCreateParams,
        completions::CompletionCreateParams,
        embeddings::{EmbeddingCreateParams, EmbeddingEncodingFormat},
        responses::{
            FunctionTool, ResponseCreateParams, ResponseFormatTextConfig,
            ResponseFormatTextJSONSchemaConfig, ResponseParseParams, ResponseTextConfig,
        },
        uploads::{ChunkedUploadSource, UploadChunkedParams, UploadPurpose},
    },
};
use serde_json::json;

#[test]
fn readme_exists_and_contains_publish_facing_sections() {
    let readme = fs::read_to_string(repo_root().join("README.md")).expect("README.md should exist");
    for section in [
        "# openai-rust",
        "## Quickstart",
        "## Capability Overview",
        "## Start Here",
        "## Crate Map",
        "## Contributing and Releases",
        "## License",
        "> [!WARNING]",
    ] {
        assert!(
            readme.contains(section),
            "README.md should contain section `{section}`"
        );
    }

    for command in [
        "cargo add openai-rust",
        "cargo fmt --all --check",
        "cargo test --workspace",
        "cargo check --examples --all-features",
        "cargo test --doc",
    ] {
        assert!(
            readme.contains(command),
            "README.md should document `{command}`"
        );
    }

    assert!(
        readme.contains("not an official OpenAI product"),
        "README.md should include the non-affiliation warning"
    );
    assert!(
        readme.contains("actions/workflows/ci.yml/badge.svg"),
        "README.md should include the CI badge"
    );
}

#[test]
fn quickstart_and_claims_match_public_api() {
    let client = OpenAI::builder().api_key("sk-test").build();
    let request = client
        .prepare_request("GET", "/models")
        .expect("quickstart request should prepare");
    assert_eq!(request.method, "GET");
    assert!(request.url.ends_with("/models"));

    let _response_params = ResponseCreateParams {
        model: "gpt-4.1-mini".into(),
        input: Some(json!("Say hello from Rust.")),
        ..Default::default()
    };

    let _parse_params = ResponseParseParams {
        model: "gpt-4.1-mini".into(),
        input: Some(json!("Return {\"language\":\"rust\"}")),
        text: Some(ResponseTextConfig {
            format: Some(ResponseFormatTextConfig::JsonSchema(
                ResponseFormatTextJSONSchemaConfig {
                    name: "language".into(),
                    schema: json!({
                        "type": "object",
                        "properties": { "language": { "type": "string" } },
                        "required": ["language"],
                        "additionalProperties": false
                    }),
                    description: Some("Structured quickstart response".into()),
                    strict: Some(true),
                },
            )),
            verbosity: None,
        }),
        tools: vec![FunctionTool {
            name: "lookup_model".into(),
            parameters: json!({
                "type": "object",
                "properties": { "model": { "type": "string" } },
                "required": ["model"],
                "additionalProperties": false
            }),
            strict: Some(true),
            description: Some("Example tool schema".into()),
            defer_loading: None,
        }],
        ..Default::default()
    };

    let _chat_params = ChatCompletionCreateParams {
        model: "gpt-4.1-mini".into(),
        messages: vec![json!({"role":"user","content":"Say hello"})],
        ..Default::default()
    };

    let _legacy_params = CompletionCreateParams {
        model: "gpt-3.5-turbo-instruct".into(),
        prompt: Some(json!("Say hello")),
        ..Default::default()
    };

    let _embedding_params = EmbeddingCreateParams {
        model: "text-embedding-3-small".into(),
        input: json!(["rust", "responses"]),
        encoding_format: Some(EmbeddingEncodingFormat::Float),
        ..Default::default()
    };

    let _chunked_upload = UploadChunkedParams {
        source: ChunkedUploadSource::InMemory {
            bytes: b"hello from rust".to_vec(),
            filename: Some("notes.txt".into()),
            byte_length: Some(15),
        },
        mime_type: "text/plain".into(),
        purpose: UploadPurpose::Assistants,
        part_size: Some(8),
        md5: None,
    };

    let metadata = ResponseMetadata {
        status_code: 200,
        headers: [("x-request-id".into(), "req_readme".into())]
            .into_iter()
            .collect(),
        request_id: Some("req_readme".into()),
    };
    let response = ApiResponse {
        output: vec!["ok"],
        metadata,
    };
    assert_eq!(response.request_id(), Some("req_readme"));
}

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}
