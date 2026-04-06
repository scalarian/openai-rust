use openai_rust::{
    ResponseMetadata,
    resources::responses::{ResponseStream, ResponseStreamEvent, ResponseStreamTerminal},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let metadata = ResponseMetadata {
        status_code: 200,
        request_id: Some("req_example_stream".into()),
        ..Default::default()
    };

    let transcript = concat!(
        "event: response.created\n",
        "data: {\"id\":\"resp_example\",\"object\":\"response\",\"created_at\":1,\"status\":\"in_progress\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"\"}]}],\"usage\":{}}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"output_index\":0,\"content_index\":0,\"delta\":\"Hel\"}\n\n",
        "event: response.output_text.done\n",
        "data: {\"output_index\":0,\"content_index\":0,\"text\":\"Hello\"}\n\n",
        "event: response.completed\n",
        "data: {\"id\":\"resp_example\",\"object\":\"response\",\"created_at\":1,\"status\":\"completed\",\"output\":[{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"Hello\"}]}],\"usage\":{}}\n\n",
        "data: [DONE]\n\n"
    );

    let mut stream = ResponseStream::from_sse_chunks(metadata, [transcript])?;
    while let Some(event) = stream.next_event() {
        match event {
            ResponseStreamEvent::Created { response } => {
                println!("created {}", response.id);
            }
            ResponseStreamEvent::OutputTextDelta { delta, .. } => {
                println!("delta: {delta}");
            }
            ResponseStreamEvent::OutputTextDone { text, .. } => {
                println!("done: {text}");
            }
            ResponseStreamEvent::Completed { response } => {
                println!("completed: {}", response.output_text());
            }
            other => println!("event: {other:?}"),
        }
    }

    if let Some(ResponseStreamTerminal::Completed(response)) = stream.terminal_state() {
        println!("terminal output: {}", response.output_text());
    }

    Ok(())
}
