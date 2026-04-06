#[path = "support/mock_http.rs"]
mod mock_http;

use openai_rust::{
    ApiErrorKind, ErrorKind, OpenAI,
    resources::vector_stores::{
        StaticChunkingStrategy, VectorStoreCreateParams, VectorStoreDeleteResponse,
        VectorStoreExpiresAfter, VectorStoreListParams, VectorStoreSearchParams,
        VectorStoreSearchQuery, VectorStoreSearchRankingOptions, VectorStoreStatus,
        VectorStoreUpdateParams,
    },
};
use serde_json::json;

#[test]
fn crud_and_errors() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(vector_store_payload(
            "vs_123",
            "Knowledge Base",
            "in_progress",
        )),
        json_response(vector_store_payload(
            "vs_123",
            "Knowledge Base",
            "completed",
        )),
        json_response(vector_store_payload(
            "vs_123",
            "Knowledge Base v2",
            "completed",
        )),
        json_response(
            json!({
                "id": "vs_123",
                "object": "vector_store.deleted",
                "deleted": true
            })
            .to_string(),
        ),
        not_found_response("No vector store found for id vs_missing"),
    ])
    .unwrap();
    let client = client(&server.url());

    let created = client
        .vector_stores()
        .create(VectorStoreCreateParams {
            name: Some(String::from("Knowledge Base")),
            description: Some(String::from("Customer support snippets")),
            file_ids: vec![String::from("file_1"), String::from("file_2")],
            metadata: Some(json!({"env": "test"})),
            expires_after: Some(VectorStoreExpiresAfter {
                anchor: String::from("last_active_at"),
                days: 7,
            }),
            chunking_strategy: Some(
                openai_rust::resources::vector_stores::FileChunkingStrategy::Static {
                    static_config: StaticChunkingStrategy {
                        max_chunk_size_tokens: 512,
                        chunk_overlap_tokens: 128,
                    },
                },
            ),
        })
        .unwrap();
    assert_eq!(created.output.id, "vs_123");
    assert_eq!(created.output.status, Some(VectorStoreStatus::InProgress));

    let retrieved = client.vector_stores().retrieve("vs_123").unwrap();
    assert_eq!(retrieved.output.id, "vs_123");
    assert_eq!(retrieved.output.status, Some(VectorStoreStatus::Completed));

    let updated = client
        .vector_stores()
        .update(
            "vs_123",
            VectorStoreUpdateParams {
                name: Some(String::from("Knowledge Base v2")),
                metadata: Some(json!({"env": "prod"})),
                expires_after: Some(VectorStoreExpiresAfter {
                    anchor: String::from("last_active_at"),
                    days: 30,
                }),
            },
        )
        .unwrap();
    assert_eq!(updated.output.name.as_deref(), Some("Knowledge Base v2"));

    let deleted = client.vector_stores().delete("vs_123").unwrap();
    assert_eq!(
        deleted.output,
        VectorStoreDeleteResponse {
            id: String::from("vs_123"),
            object: String::from("vector_store.deleted"),
            deleted: true,
            extra: Default::default(),
        }
    );

    let error = client.vector_stores().retrieve("vs_missing").unwrap_err();
    assert!(matches!(error.kind, ErrorKind::Api(ApiErrorKind::NotFound)));
    assert_eq!(error.status_code(), Some(404));

    let requests = server.captured_requests(5).unwrap();
    assert_eq!(requests[0].path, "/v1/vector_stores");
    assert_eq!(requests[1].path, "/v1/vector_stores/vs_123");
    assert_eq!(requests[2].path, "/v1/vector_stores/vs_123");
    assert_eq!(requests[3].path, "/v1/vector_stores/vs_123");
    assert_eq!(requests[4].path, "/v1/vector_stores/vs_missing");
    for request in requests {
        assert_eq!(
            request.headers.get("openai-beta").map(String::as_str),
            Some("assistants=v2")
        );
    }

    let blank_id = client.vector_stores().retrieve(" ").unwrap_err();
    assert!(matches!(blank_id.kind, ErrorKind::Validation));
}

#[test]
fn list_and_search_preserve_distinct_page_contracts() {
    let server = mock_http::MockHttpServer::spawn_sequence(vec![
        json_response(vector_store_page_payload()),
        json_response(vector_store_search_payload()),
    ])
    .unwrap();
    let client = client(&server.url());

    let listed = client
        .vector_stores()
        .list(VectorStoreListParams {
            after: Some(String::from("vs_000")),
            before: Some(String::from("vs_999")),
            limit: Some(2),
            order: Some(String::from("desc")),
        })
        .unwrap();
    assert_eq!(listed.output.data.len(), 2);
    assert!(listed.output.has_next_page());
    assert_eq!(listed.output.next_after(), Some("vs_222"));

    let searched = client
        .vector_stores()
        .search(
            "vs_123",
            VectorStoreSearchParams {
                query: VectorStoreSearchQuery::Multiple(vec![
                    String::from("refund policy"),
                    String::from("chargeback"),
                ]),
                filters: Some(json!({"type": "eq", "key": "department", "value": "support"})),
                max_num_results: Some(8),
                ranking_options: Some(VectorStoreSearchRankingOptions {
                    ranker: Some(String::from("default-2024-11-15")),
                    score_threshold: Some(0.42),
                }),
                rewrite_query: Some(true),
            },
        )
        .unwrap();
    assert_eq!(searched.output.data.len(), 1);
    assert_eq!(searched.output.data[0].filename, "policy.txt");
    assert_eq!(
        searched.output.data[0].content[0].text,
        "Refunds are handled within 5 business days."
    );

    let requests = server.captured_requests(2).unwrap();
    assert_eq!(
        requests[0].path,
        "/v1/vector_stores?after=vs_000&before=vs_999&limit=2&order=desc"
    );
    assert_eq!(requests[1].path, "/v1/vector_stores/vs_123/search");
    let search_body: serde_json::Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(search_body["max_num_results"], json!(8));
    assert_eq!(
        search_body["ranking_options"]["ranker"],
        json!("default-2024-11-15")
    );
    assert_eq!(
        search_body["ranking_options"]["score_threshold"],
        json!(0.42)
    );
    assert_eq!(search_body["rewrite_query"], json!(true));
}

fn client(base_url: &str) -> OpenAI {
    OpenAI::builder()
        .api_key("sk-test")
        .base_url(base_url)
        .build()
}

fn vector_store_payload(id: &str, name: &str, status: &str) -> String {
    json!({
        "id": id,
        "object": "vector_store",
        "created_at": 1_717_171_717,
        "name": name,
        "description": "Customer support snippets",
        "status": status,
        "usage_bytes": 2048,
        "last_active_at": 1_717_171_800,
        "metadata": {"env": "test"},
        "expires_after": {"anchor": "last_active_at", "days": 7},
        "file_counts": {
            "in_progress": 1,
            "completed": 2,
            "failed": 0,
            "cancelled": 0,
            "total": 3
        }
    })
    .to_string()
}

fn vector_store_page_payload() -> String {
    json!({
        "object": "list",
        "data": [
            serde_json::from_str::<serde_json::Value>(&vector_store_payload("vs_111", "KB A", "completed")).unwrap(),
            serde_json::from_str::<serde_json::Value>(&vector_store_payload("vs_222", "KB B", "completed")).unwrap()
        ],
        "has_more": true
    })
    .to_string()
}

fn vector_store_search_payload() -> String {
    json!({
        "object": "list",
        "data": [
            {
                "file_id": "file_123",
                "filename": "policy.txt",
                "score": 0.98,
                "attributes": {"department": "support"},
                "content": [
                    {
                        "type": "text",
                        "text": "Refunds are handled within 5 business days."
                    }
                ]
            }
        ],
        "has_more": false
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
        ],
        body: body.into_bytes(),
        ..Default::default()
    }
}

fn not_found_response(message: &str) -> mock_http::ScriptedResponse {
    let body = json!({
        "error": {
            "message": message,
            "type": "invalid_request_error",
            "code": "not_found"
        }
    })
    .to_string();
    mock_http::ScriptedResponse {
        status_code: 404,
        reason: "Not Found",
        headers: vec![
            (String::from("content-length"), body.len().to_string()),
            (
                String::from("content-type"),
                String::from("application/json"),
            ),
            (String::from("x-request-id"), String::from("req_vs_missing")),
        ],
        body: body.into_bytes(),
        ..Default::default()
    }
}
