use openai_rust::{
    DEFAULT_BASE_URL, OpenAI,
    resources::fine_tuning::{
        FineTuningGrader, FineTuningGraderRunParams, FineTuningGraderValidateParams,
    },
};

#[test]
#[ignore = "requires live OpenAI credentials"]
fn live_fine_tuning_graders_smoke_captures_request_ids_and_scores() {
    let client = OpenAI::builder().build();
    let resolved = client
        .resolved_config()
        .expect("live graders client should resolve configuration");
    assert_eq!(resolved.base_url, DEFAULT_BASE_URL);

    let grader = FineTuningGrader::StringCheck {
        input: String::from("{{sample.output_text}}"),
        name: String::from("exact_match"),
        operation: String::from("eq"),
        reference: String::from("sunny"),
    };

    let validated = client
        .fine_tuning()
        .alpha()
        .graders()
        .validate(FineTuningGraderValidateParams {
            grader: grader.clone(),
        })
        .expect("live grader validation should succeed");
    let validate_request_id = validated
        .request_id()
        .expect("live grader validation should expose a request id");
    assert!(!validate_request_id.trim().is_empty());

    let run = client
        .fine_tuning()
        .alpha()
        .graders()
        .run(FineTuningGraderRunParams {
            grader,
            model_sample: String::from("sunny"),
            item: Some(serde_json::json!({"reference": "sunny"})),
        })
        .expect("live grader run should succeed");
    let run_request_id = run
        .request_id()
        .expect("live grader run should expose a request id");
    assert!(!run_request_id.trim().is_empty());
    assert!(run.output.reward >= 0.0);
    println!("live grader validate request id: {validate_request_id}");
    println!("live grader run request id: {run_request_id}");
    println!("live grader reward: {}", run.output.reward);
    println!("live grader sub_rewards: {:?}", run.output.sub_rewards);
}
