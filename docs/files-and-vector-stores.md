# Files, uploads, and vector stores

Use the public file and upload helpers when moving content into downstream retrieval workflows.

## Chunked uploads

```rust
use openai_rust::resources::uploads::{ChunkedUploadSource, UploadChunkedParams, UploadPurpose};

let params = UploadChunkedParams {
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

let _ = params;
```

## Vector-store attach/upload helpers

```rust
use openai_rust::resources::{
    files::FileUpload,
    vector_stores::VectorStoreFileUploadParams,
};
use serde_json::json;

let params = VectorStoreFileUploadParams {
    file: FileUpload::new("notes.txt", "text/plain", b"retrieval ready".to_vec()),
    attributes: Some(json!({"topic":"sdk"})),
    chunking_strategy: None,
};

let _ = params;
```

## Runnable upload-to-downstream example

```sh
cargo run --example upload_to_vector_store
```
