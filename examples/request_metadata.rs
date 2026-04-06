use openai_rust::{ApiResponse, ResponseMetadata};

fn main() {
    let response = ApiResponse {
        output: vec!["hello", "rust"],
        metadata: ResponseMetadata {
            status_code: 200,
            headers: [
                ("content-type".into(), "application/json".into()),
                ("x-request-id".into(), "req_example_meta".into()),
            ]
            .into_iter()
            .collect(),
            request_id: Some("req_example_meta".into()),
        },
    };

    println!("status: {}", response.status_code());
    println!("request_id: {:?}", response.request_id());
    println!("content-type: {:?}", response.header("content-type"));
}
