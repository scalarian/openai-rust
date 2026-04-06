use openai_rust::resources::{
    files::FileUpload,
    uploads::{ChunkedUploadSource, UploadChunkedParams, UploadPurpose},
    vector_stores::VectorStoreFileUploadParams,
};
use serde_json::json;

fn main() {
    let chunked = UploadChunkedParams {
        source: ChunkedUploadSource::InMemory {
            bytes: b"hello from rust".to_vec(),
            filename: Some("notes.txt".into()),
            byte_length: Some(15),
        },
        mime_type: "text/plain".into(),
        purpose: UploadPurpose::Assistants,
        part_size: Some(8),
        md5: None,
    };

    let attach = VectorStoreFileUploadParams {
        file: FileUpload::new("notes.txt", "text/plain", b"retrieval ready".to_vec()),
        attributes: Some(json!({"topic":"sdk"})),
        chunking_strategy: None,
    };

    println!("Chunked upload source prepared for {:?}", chunked.purpose);
    println!("Vector-store upload file: {}", attach.file.filename);
}
