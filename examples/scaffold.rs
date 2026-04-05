use openai_rust::OpenAI;

fn main() {
    let client = OpenAI::builder().build();
    let _ = client.responses();
    let _ = client.realtime();
}
