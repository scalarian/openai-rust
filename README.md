# openai-rust

Independent Rust SDK for the OpenAI API with Responses-first ergonomics, realtime support, and typed compatibility helpers.

[![CI](https://github.com/scalarian/openai-rust/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/scalarian/openai-rust/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)

## Quickstart

```sh
cargo add scalarian-openai-rust
```

```rust,no_run
use openai_rust::OpenAI;
use openai_rust::resources::responses::ResponseCreateParams;
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OpenAI::builder().build();

    let response = client.responses().create(ResponseCreateParams {
        model: "gpt-4.1-mini".into(),
        input: Some(json!("Say hello from Rust.")),
        ..Default::default()
    })?;

    println!("{}", response.output.output_text());
    Ok(())
}
```

From a local checkout, validate the published surfaces with:

```sh
cargo fmt --all --check
cargo test --workspace
cargo check --examples --all-features
cargo test --doc
```

If you want a zero-cost configuration check before making a live request, use `client.prepare_request("GET", "/models")?` to confirm env-based configuration resolves correctly.

## Capability Overview

- Responses-first request and streaming flows, including structured outputs and tool schemas.
- Coverage for chat completions, legacy completions, embeddings, files, uploads, vector stores, images, audio, evals, fine-tuning, moderations, and webhooks.
- Realtime session models plus an opt-in blocking facade for synchronous integrations.
- Example-driven docs and contract tests that keep the published API aligned with the repository.

## Start Here

Choose the path that matches your job:

- [Quickstart](docs/quickstart.md) for a fresh integration.
- [Responses Guide](docs/responses-guide.md) for structured outputs, streaming, and tool-heavy flows.
- [Migration Guide](docs/migration-guide.md) for chat-completions and legacy completions compatibility paths.
- [Files and Vector Stores](docs/files-and-vector-stores.md) for uploads and retrieval workflows.
- [Architecture Note](docs/architecture-note.md) for runtime, transport, and realtime notes.
- [API Coverage](docs/api-coverage.md) and [Intentional Gaps](docs/intentional-gaps.md) for surface-level status.

## Crate Map

- `openai_rust::OpenAI`: client builder, environment-driven auth, and request preparation.
- `openai_rust::resources`: typed REST surfaces for the API families exposed by the crate.
- `openai_rust::realtime`: session, event, and state models for Realtime workflows.
- `openai_rust::helpers`: multipart uploads, pagination, SSE, structured output, and webhook utilities.
- `openai_rust::blocking`: feature-gated blocking facade for non-async callers.

## Contributing and Releases

- Start with [docs/maintenance.md](docs/maintenance.md) for validation commands and release-ready checks.
- Use [SUPPORT.md](SUPPORT.md) to choose between bug reports, docs issues, feature requests, and usage questions.
- Review [SECURITY.md](SECURITY.md) before reporting sensitive issues.
- See [CHANGELOG.md](CHANGELOG.md) for release notes and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for participation expectations.
- Maintainer: Chaitanya Mishra ([@staticpayload](https://github.com/staticpayload)).

## License

Apache-2.0. See [LICENSE](LICENSE).

> [!WARNING]
> This project is not an official OpenAI product. It is not affiliated with, endorsed by, or supported by OpenAI.
