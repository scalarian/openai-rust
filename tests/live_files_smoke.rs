use openai_rust::{
    DEFAULT_BASE_URL, OpenAI,
    resources::files::{FileCreateParams, FilePurpose, FileUpload},
};

#[test]
#[ignore = "requires live OpenAI credentials"]
fn live_files_smoke_exercises_create_retrieve_content_and_delete() {
    let client = OpenAI::builder().build();
    let resolved = client
        .resolved_config()
        .expect("live files client should resolve configuration");
    assert_eq!(resolved.base_url, DEFAULT_BASE_URL);

    let created = client
        .files()
        .create(FileCreateParams {
            file: FileUpload::new(
                "live-files-smoke.jsonl",
                "application/jsonl",
                br#"{"custom_id":"live-files-smoke","method":"GET","url":"/v1/models"}"#.to_vec(),
            ),
            purpose: FilePurpose::Batch,
            expires_after: None,
        })
        .expect("live file create should succeed");
    let file_id = created.output.id.clone();
    let create_request_id = created
        .request_id()
        .expect("live file create should expose a request id");
    assert!(!create_request_id.trim().is_empty());

    let retrieved = client
        .files()
        .retrieve(&file_id)
        .expect("live file retrieve should succeed");
    assert_eq!(retrieved.output.id, file_id);

    let content = client
        .files()
        .content(&file_id)
        .expect("live file content should succeed");
    assert_eq!(
        content.output,
        br#"{"custom_id":"live-files-smoke","method":"GET","url":"/v1/models"}"#
    );

    let deleted = client
        .files()
        .delete(&file_id)
        .expect("live file delete should succeed");
    assert!(deleted.output.deleted);

    println!("live file id: {file_id}");
    println!("live file create request id: {create_request_id}");
    println!(
        "live file retrieve request id: {}",
        retrieved.request_id().unwrap_or("<missing>")
    );
    println!(
        "live file content request id: {}",
        content.request_id().unwrap_or("<missing>")
    );
    println!(
        "live file delete request id: {}",
        deleted.request_id().unwrap_or("<missing>")
    );
}
