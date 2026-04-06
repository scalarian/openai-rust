# Maintenance commands

Run these commands from the repository root when validating publish-facing docs and examples:

```sh
cargo fmt --all --check
cargo check --all-targets --all-features
cargo test --all-features -- --test-threads=5
cargo clippy --all-targets --all-features -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --all-features
cargo check --examples --all-features
cargo test --test readme_contract
cargo test --test docs_contract
cargo test --doc
```

Representative runnable examples:

```sh
cargo run --example responses_quickstart
cargo run --example responses_streaming
cargo run --example structured_outputs
cargo run --example request_metadata
cargo run --example embeddings
cargo run --example upload_to_vector_store
cargo run --example chat_completions_migration
```
