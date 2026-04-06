use openai_rust::{
    ErrorKind, OpenAI,
    realtime::{
        RealtimeCallAcceptParams, RealtimeCallCreateParams, RealtimeCallReferParams,
        RealtimeCallRejectParams, RealtimeClientSecretCreateParams, RealtimeOutputModality,
        RealtimeSessionConfig, RealtimeSessionType, RealtimeSessionTTL,
    },
};
use serde_json::json;

#[path = "support/mock_http.rs"]
mod mock_http;
#[path = "support/multipart.rs"]
mod multipart;

#[test]
fn client_secret_creation_and_call_helpers_preserve_routes_and_wire_shapes() {
    let sdp_answer = "v=0\r\no=- 1 2 IN IP4 127.0.0.1\r\ns=-\r\n".to_string();
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(
            json!({
                "value": "ek_test_123",
                "expires_at": 1_740_000_000,
                "session": {
                    "id": "sess_client_secret",
                    "type": "realtime",
                    "model": "gpt-realtime-mini",
                    "output_modalities": ["text"],
                    "instructions": "Answer tersely."
                }
            })
            .to_string(),
        ),
        sdp_response(sdp_answer.clone()),
        sdp_response(sdp_answer.clone()),
        empty_response(),
        empty_response(),
        empty_response(),
        empty_response(),
    ])
    .unwrap();

    let client = OpenAI::builder()
        .api_key("test-key")
        .base_url(server.url())
        .max_retries(0)
        .build();

    let secret = client
        .realtime()
        .client_secrets()
        .create(RealtimeClientSecretCreateParams {
            expires_after: Some(RealtimeSessionTTL {
                anchor: String::from("created_at"),
                seconds: 60,
            }),
            session: Some(RealtimeSessionConfig {
                session_type: RealtimeSessionType::Realtime,
                model: Some(String::from("gpt-realtime-mini")),
                output_modalities: Some(vec![RealtimeOutputModality::Text]),
                instructions: Some(String::from("Answer tersely.")),
                ..Default::default()
            }),
        })
        .unwrap();
    assert_eq!(secret.output().client_secret.value, "ek_test_123");
    assert_eq!(secret.output().client_secret.expires_at, 1_740_000_000);
    assert_eq!(secret.output().session.session_type, RealtimeSessionType::Realtime);
    assert_eq!(
        secret.output().session.output_modalities,
        Some(vec![RealtimeOutputModality::Text])
    );

    let sdp_only = client
        .realtime()
        .calls()
        .create(RealtimeCallCreateParams {
            sdp: String::from("v=0\r\n"),
            session: None,
        })
        .unwrap();
    assert_eq!(String::from_utf8_lossy(sdp_only.output()), sdp_answer);

    let sdp_with_session = client
        .realtime()
        .calls()
        .create(RealtimeCallCreateParams {
            sdp: String::from("v=0\r\n"),
            session: Some(RealtimeSessionConfig {
                session_type: RealtimeSessionType::Realtime,
                model: Some(String::from("gpt-realtime-mini")),
                output_modalities: Some(vec![RealtimeOutputModality::Text]),
                ..Default::default()
            }),
        })
        .unwrap();
    assert_eq!(String::from_utf8_lossy(sdp_with_session.output()), sdp_answer);

    client
        .realtime()
        .calls()
        .accept(
            "call_accept",
            RealtimeCallAcceptParams {
                session_type: RealtimeSessionType::Realtime,
                model: Some(String::from("gpt-realtime-mini")),
                output_modalities: Some(vec![RealtimeOutputModality::Text]),
                instructions: Some(String::from("Stay concise.")),
                ..Default::default()
            },
        )
        .unwrap();

    client.realtime().calls().hangup("call_hangup").unwrap();

    client
        .realtime()
        .calls()
        .refer(
            "call_refer",
            RealtimeCallReferParams {
                target_uri: String::from("sip:agent@example.com"),
            },
        )
        .unwrap();

    client
        .realtime()
        .calls()
        .reject(
            "call_reject",
            RealtimeCallRejectParams {
                status_code: Some(486),
            },
        )
        .unwrap();

    let requests = server.captured_requests(7).expect("captured requests");

    let client_secret_body: serde_json::Value =
        serde_json::from_slice(&requests[0].body).expect("client secret json body");
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/v1/realtime/client_secrets");
    assert_eq!(client_secret_body["expires_after"]["anchor"], "created_at");
    assert_eq!(client_secret_body["expires_after"]["seconds"], 60);
    assert_eq!(client_secret_body["session"]["type"], "realtime");
    assert_eq!(client_secret_body["session"]["output_modalities"][0], "text");

    assert_eq!(requests[1].path, "/v1/realtime/calls");
    assert_eq!(
        requests[1].headers.get("content-type").map(String::as_str),
        Some("application/sdp")
    );
    assert_eq!(
        requests[1].headers.get("accept").map(String::as_str),
        Some("application/sdp")
    );
    assert_eq!(String::from_utf8_lossy(&requests[1].body), "v=0\r\n");

    assert_eq!(requests[2].path, "/v1/realtime/calls");
    let multipart_content_type = requests[2]
        .headers
        .get("content-type")
        .expect("multipart content type");
    let boundary = multipart_content_type
        .split("boundary=")
        .nth(1)
        .expect("boundary");
    let parsed = multipart::parse_multipart(&requests[2].body, boundary).expect("multipart body");
    assert_eq!(parsed.parts.len(), 2);
    assert_eq!(parsed.parts[0].name.as_deref(), Some("sdp"));
    assert_eq!(
        parsed.parts[0]
            .headers
            .get("content-type")
            .map(String::as_str),
        Some("application/sdp")
    );
    assert_eq!(parsed.parts[0].body, b"v=0\r\n");
    assert_eq!(parsed.parts[1].name.as_deref(), Some("session"));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&parsed.parts[1].body).unwrap()["type"],
        "realtime"
    );

    let accept_body: serde_json::Value =
        serde_json::from_slice(&requests[3].body).expect("accept json body");
    assert_eq!(requests[3].path, "/v1/realtime/calls/call_accept/accept");
    assert_eq!(
        requests[3].headers.get("accept").map(String::as_str),
        Some("*/*")
    );
    assert_eq!(accept_body["type"], "realtime");
    assert_eq!(accept_body["output_modalities"][0], "text");

    assert_eq!(requests[4].path, "/v1/realtime/calls/call_hangup/hangup");
    assert!(requests[4].body.is_empty());

    let refer_body: serde_json::Value =
        serde_json::from_slice(&requests[5].body).expect("refer json body");
    assert_eq!(requests[5].path, "/v1/realtime/calls/call_refer/refer");
    assert_eq!(refer_body["target_uri"], "sip:agent@example.com");

    let reject_body: serde_json::Value =
        serde_json::from_slice(&requests[6].body).expect("reject json body");
    assert_eq!(requests[6].path, "/v1/realtime/calls/call_reject/reject");
    assert_eq!(reject_body["status_code"], 486);

    let blank_call_id = client
        .realtime()
        .calls()
        .hangup(" ")
        .expect_err("blank call_id should be rejected locally");
    assert_eq!(blank_call_id.kind, ErrorKind::Validation);
}

fn json_response(body: String) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        headers: vec![
            (String::from("content-type"), String::from("application/json")),
            (String::from("content-length"), body.len().to_string()),
        ],
        body: body.into_bytes(),
        ..Default::default()
    }
}

fn sdp_response(body: String) -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        headers: vec![
            (String::from("content-type"), String::from("application/sdp")),
            (String::from("content-length"), body.len().to_string()),
        ],
        body: body.into_bytes(),
        ..Default::default()
    }
}

fn empty_response() -> mock_http::ScriptedResponse {
    mock_http::ScriptedResponse {
        headers: vec![(String::from("content-length"), String::from("0"))],
        ..Default::default()
    }
}
