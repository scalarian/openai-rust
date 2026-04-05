#![forbid(unsafe_code)]
#![doc = r#"
Clean-room Rust SDK scaffold for the OpenAI API.

```no_run
use openai_rust::OpenAI;

let client = OpenAI::builder().build();
let _responses = client.responses();
```
"#]

pub mod blocking;
pub mod client;
pub mod config;
pub mod core;
pub mod error;
pub mod helpers;
pub mod realtime;
pub mod resources;

pub use client::{OpenAI, OpenAIBuilder};
pub use config::ClientConfig;
pub use error::{ErrorKind, OpenAIError};

pub const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
