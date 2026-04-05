use openai_rust::OpenAI;

#[test]
#[ignore = "requires live OpenAI credentials"]
fn live_image_stream_smoke_captures_request_id_and_completed_event() {
    let client = OpenAI::builder().build();

    let mut stream = client
        .images()
        .generate_stream(openai_rust::resources::images::ImageGenerateParams {
            prompt: String::from("A tiny monochrome icon of a lighthouse"),
            model: Some(String::from("gpt-image-1")),
            n: Some(1),
            output_format: Some(String::from("png")),
            partial_images: Some(1),
            size: Some(String::from("1024x1024")),
            ..Default::default()
        })
        .expect("live image stream should start");

    let mut partial_events = 0usize;
    while let Some(event) = stream.next_event() {
        if matches!(
            event,
            openai_rust::resources::images::ImageGenerationStreamEvent::PartialImage(_)
        ) {
            partial_events += 1;
        }
    }

    let completed = stream
        .final_completed()
        .expect("live image stream should finish with a completed event");
    assert!(!completed.b64_json.trim().is_empty());

    let request_id = stream
        .metadata()
        .request_id()
        .expect("live image stream metadata should expose a request id");
    assert!(!request_id.trim().is_empty());

    println!("live image request id: {request_id}");
    println!("partial image events observed: {partial_events}");
    println!(
        "completed image bytes (base64 chars): {}",
        completed.b64_json.len()
    );
}
