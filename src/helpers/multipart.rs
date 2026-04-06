use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicU64, Ordering},
};

/// Binary multipart file part.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultipartFile {
    pub filename: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

impl MultipartFile {
    pub fn new(
        filename: impl Into<String>,
        content_type: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            filename: filename.into(),
            content_type: content_type.into(),
            bytes: bytes.into(),
        }
    }
}

/// Built multipart payload ready for transport.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultipartPayload {
    boundary: String,
    body: Vec<u8>,
}

impl MultipartPayload {
    pub fn content_type(&self) -> String {
        format!("multipart/form-data; boundary={}", self.boundary)
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }

    pub fn into_body(self) -> Vec<u8> {
        self.body
    }

    pub fn boundary(&self) -> &str {
        &self.boundary
    }
}

/// Simple multipart builder that preserves insertion order.
#[derive(Clone, Debug, Default)]
pub struct MultipartBuilder {
    parts: Vec<MultipartPart>,
}

impl MultipartBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_text(&mut self, name: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.parts.push(MultipartPart::Text {
            name: name.into(),
            value: value.into(),
        });
        self
    }

    pub fn add_file(&mut self, name: impl Into<String>, file: MultipartFile) -> &mut Self {
        self.parts.push(MultipartPart::File {
            name: name.into(),
            file,
            extra_headers: BTreeMap::new(),
        });
        self
    }

    pub fn build(self) -> MultipartPayload {
        static NEXT_BOUNDARY: AtomicU64 = AtomicU64::new(1);
        let boundary = format!(
            "{}-boundary-{}",
            env!("CARGO_PKG_NAME"),
            NEXT_BOUNDARY.fetch_add(1, Ordering::Relaxed)
        );
        let mut body = Vec::new();

        for part in self.parts {
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            match part {
                MultipartPart::Text { name, value } => {
                    body.extend_from_slice(
                        format!(
                            "Content-Disposition: form-data; name=\"{}\"\r\n\r\n",
                            escape_quotes(&name)
                        )
                        .as_bytes(),
                    );
                    body.extend_from_slice(value.as_bytes());
                    body.extend_from_slice(b"\r\n");
                }
                MultipartPart::File {
                    name,
                    file,
                    extra_headers,
                } => {
                    body.extend_from_slice(
                        format!(
                            "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n",
                            escape_quotes(&name),
                            escape_quotes(&file.filename)
                        )
                        .as_bytes(),
                    );
                    body.extend_from_slice(
                        format!("Content-Type: {}\r\n", file.content_type).as_bytes(),
                    );
                    for (header, value) in extra_headers {
                        body.extend_from_slice(format!("{header}: {value}\r\n").as_bytes());
                    }
                    body.extend_from_slice(b"\r\n");
                    body.extend_from_slice(&file.bytes);
                    body.extend_from_slice(b"\r\n");
                }
            }
        }

        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        MultipartPayload { boundary, body }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MultipartPart {
    Text {
        name: String,
        value: String,
    },
    File {
        name: String,
        file: MultipartFile,
        extra_headers: BTreeMap<String, String>,
    },
}

fn escape_quotes(value: &str) -> String {
    value.replace('"', "\\\"")
}
