use openai_rust::{
    ErrorKind,
    core::metadata::ResponseMetadata,
    resources::responses::{
        FunctionTool, ParsedResponse, ResponseFormatTextConfig, ResponseFormatTextJSONSchemaConfig,
        ResponseStream, ResponseTextConfig,
    },
};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize, PartialEq)]
struct LocationAnswer {
    city: String,
    temperature_c: i64,
}

#[test]
fn structured_outputs_parse_only_at_completion_boundaries() {
    let metadata = ResponseMetadata {
        status_code: 200,
        ..Default::default()
    };
    let transcript = concat!(
        "event: response.created\n",
        "data: {\"id\":\"resp_json\",\"object\":\"response\",\"created_at\":1,\"status\":\"in_progress\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"\"}]}],\"usage\":{}}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"output_index\":0,\"content_index\":0,\"delta\":\"{\\\"city\\\":\\\"Paris\\\",\"}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"output_index\":0,\"content_index\":0,\"delta\":\"\\\"temperature_c\\\":21}\"}\n\n",
        "event: response.output_text.done\n",
        "data: {\"output_index\":0,\"content_index\":0,\"text\":\"{\\\"city\\\":\\\"Paris\\\",\\\"temperature_c\\\":21}\"}\n\n",
        "event: response.completed\n",
        "data: {\"id\":\"resp_json\",\"object\":\"response\",\"created_at\":1,\"status\":\"completed\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"{\\\"city\\\":\\\"Paris\\\",\\\"temperature_c\\\":21}\"}]}],\"usage\":{}}\n\n",
        "data: [DONE]\n\n"
    );

    let stream = ResponseStream::from_sse_chunks(metadata, vec![transcript]).expect("stream");
    let format = Some(ResponseTextConfig {
        format: Some(ResponseFormatTextConfig::JsonSchema(
            ResponseFormatTextJSONSchemaConfig {
                name: String::from("location_answer"),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "city": {"type": "string"},
                        "temperature_c": {"type": "integer"}
                    },
                    "required": ["city", "temperature_c"],
                    "additionalProperties": false
                }),
                description: None,
                strict: Some(true),
            },
        )),
        verbosity: None,
    });

    assert!(
        stream
            .parse_final::<LocationAnswer>(format.clone(), &[])
            .is_ok()
    );
    let parsed: ParsedResponse<LocationAnswer> = stream
        .parse_final(format, &[])
        .expect("structured output should parse");
    assert_eq!(
        parsed.output_parsed(),
        Some(&LocationAnswer {
            city: String::from("Paris"),
            temperature_c: 21,
        })
    );
}

#[test]
fn malformed_json_and_refusals_fail_at_completion_boundary() {
    let metadata = ResponseMetadata {
        status_code: 200,
        ..Default::default()
    };
    let malformed = concat!(
        "event: response.created\n",
        "data: {\"id\":\"resp_bad\",\"object\":\"response\",\"created_at\":1,\"status\":\"in_progress\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"\"}]}],\"usage\":{}}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"output_index\":0,\"content_index\":0,\"delta\":\"{\\\"city\\\":\"}\n\n",
        "event: response.completed\n",
        "data: {\"id\":\"resp_bad\",\"object\":\"response\",\"created_at\":1,\"status\":\"completed\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"{\\\"city\\\":\"}]}],\"usage\":{}}\n\n",
        "data: [DONE]\n\n"
    );
    let malformed_stream = ResponseStream::from_sse_chunks(metadata.clone(), vec![malformed])
        .expect("malformed transcript still streams");

    let format = Some(ResponseTextConfig {
        format: Some(ResponseFormatTextConfig::JsonSchema(
            ResponseFormatTextJSONSchemaConfig {
                name: String::from("location_answer"),
                schema: json!({"type": "object"}),
                description: None,
                strict: Some(true),
            },
        )),
        verbosity: None,
    });
    let error = malformed_stream
        .parse_final::<LocationAnswer>(format.clone(), &[])
        .expect_err("malformed json should fail once complete");
    assert_eq!(error.kind, ErrorKind::Parse);

    let refusal = concat!(
        "event: response.created\n",
        "data: {\"id\":\"resp_refusal\",\"object\":\"response\",\"created_at\":1,\"status\":\"in_progress\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"refusal\",\"text\":\"\"}]}],\"usage\":{}}\n\n",
        "event: response.refusal.delta\n",
        "data: {\"output_index\":0,\"content_index\":0,\"delta\":\"No\"}\n\n",
        "event: response.completed\n",
        "data: {\"id\":\"resp_refusal\",\"object\":\"response\",\"created_at\":1,\"status\":\"completed\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"refusal\",\"text\":\"No\"}]}],\"usage\":{}}\n\n",
        "data: [DONE]\n\n"
    );
    let refusal_stream =
        ResponseStream::from_sse_chunks(metadata, vec![refusal]).expect("refusal transcript");
    let refusal_error = refusal_stream
        .parse_final::<LocationAnswer>(
            format,
            &[FunctionTool {
                name: String::from("weather"),
                parameters: json!({"type": "object"}),
                strict: Some(true),
                description: None,
                defer_loading: None,
            }],
        )
        .expect_err("refusal should remain explicit");
    assert_eq!(refusal_error.kind, ErrorKind::Parse);
}
