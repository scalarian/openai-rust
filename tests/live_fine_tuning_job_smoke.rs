use std::time::Duration;

use openai_rust::{
    DEFAULT_BASE_URL, OpenAI,
    resources::{
        files::{FileCreateParams, FilePurpose, FileUpload, WaitForProcessingOptions},
        fine_tuning::{FineTuningJobCreateParams, FineTuningJobListParams, FineTuningJobStatus},
    },
};

#[test]
#[ignore = "requires live OpenAI credentials"]
fn live_fine_tuning_job_smoke_proves_create_retrieve_list_cancel() {
    let client = OpenAI::builder().build();
    let resolved = client
        .resolved_config()
        .expect("live fine-tuning client should resolve configuration");
    assert_eq!(resolved.base_url, DEFAULT_BASE_URL);

    let uploaded = client
        .files()
        .create(FileCreateParams {
            file: FileUpload::new(
                "live-fine-tuning-smoke.jsonl",
                "application/jsonl",
                br#"{"messages":[{"role":"system","content":"You answer in one word."},{"role":"user","content":"Weather?"},{"role":"assistant","content":"sunny"}]}"#
                    .to_vec(),
            ),
            purpose: FilePurpose::FineTune,
            expires_after: None,
        })
        .expect("live fine-tuning training file upload should succeed");
    let training_file_id = uploaded.output.id.clone();
    let upload_request_id = uploaded
        .request_id()
        .expect("live fine-tuning file upload should expose a request id");
    assert!(!upload_request_id.trim().is_empty());

    let processed_file = client
        .files()
        .wait_for_processing(
            &training_file_id,
            WaitForProcessingOptions {
                poll_interval: Duration::from_secs(2),
                max_wait: Duration::from_secs(90),
            },
        )
        .expect("live fine-tuning training file should finish processing");
    assert_eq!(processed_file.output.id, training_file_id);

    let created = client
        .fine_tuning()
        .jobs()
        .create(FineTuningJobCreateParams {
            model: String::from("gpt-4o-mini-2024-07-18"),
            training_file: training_file_id.clone(),
            suffix: Some(String::from("sdk-smoke")),
            ..Default::default()
        })
        .expect("live fine-tuning job create should succeed");
    let job_id = created.output.id.clone();
    let create_request_id = created
        .request_id()
        .expect("live fine-tuning job create should expose a request id");
    assert!(!create_request_id.trim().is_empty());

    let retrieved = client
        .fine_tuning()
        .jobs()
        .retrieve(&job_id)
        .expect("live fine-tuning job retrieve should succeed");
    assert_eq!(retrieved.output.id, job_id);
    assert_eq!(retrieved.output.training_file, training_file_id);

    let listed = client
        .fine_tuning()
        .jobs()
        .list(FineTuningJobListParams {
            after: None,
            limit: Some(20),
            metadata: None,
        })
        .expect("live fine-tuning job list should succeed");
    assert!(listed.output.data.iter().any(|job| job.id == job_id));

    let cancelled = client
        .fine_tuning()
        .jobs()
        .cancel(&job_id)
        .expect("live fine-tuning job cancel should succeed");
    assert_eq!(cancelled.output.id, job_id);
    assert_eq!(
        cancelled.output.status,
        FineTuningJobStatus::Cancelled,
        "live fine-tuning cancel should return the cancelled job status"
    );

    let events = client
        .fine_tuning()
        .jobs()
        .list_events(&job_id, Default::default())
        .expect("live fine-tuning job events should succeed after cancellation");
    assert!(
        events.output.data.iter().any(|event| {
            event.message.contains(&job_id)
                || event
                    .data
                    .as_ref()
                    .map(|value| value.to_string().contains(&job_id))
                    .unwrap_or(false)
        }) || !events.output.data.is_empty(),
        "live fine-tuning job events should include at least one event for the created job"
    );

    println!("live fine-tuning upload request id: {upload_request_id}");
    println!(
        "live fine-tuning file processing request id: {}",
        processed_file.request_id().unwrap_or("<missing>")
    );
    println!("live fine-tuning job id: {job_id}");
    println!("live fine-tuning create request id: {create_request_id}");
    println!(
        "live fine-tuning retrieve request id: {}",
        retrieved.request_id().unwrap_or("<missing>")
    );
    println!(
        "live fine-tuning list request id: {}",
        listed.request_id().unwrap_or("<missing>")
    );
    println!(
        "live fine-tuning cancel request id: {}",
        cancelled.request_id().unwrap_or("<missing>")
    );
    println!(
        "live fine-tuning events request id: {}",
        events.request_id().unwrap_or("<missing>")
    );
    println!(
        "live fine-tuning cancel status: {}",
        cancelled.output.status.as_str()
    );

    match client.files().delete(&training_file_id) {
        Ok(deleted) => println!(
            "live fine-tuning training file delete request id: {}",
            deleted.request_id().unwrap_or("<missing>")
        ),
        Err(error) => println!(
            "live fine-tuning cleanup could not delete training file {training_file_id}: {error}"
        ),
    }
}
