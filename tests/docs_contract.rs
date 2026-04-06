use std::{fs, path::PathBuf};

#[test]
fn docs_guides_notes_and_examples_exist() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for relative in [
        "docs/quickstart.md",
        "docs/responses-guide.md",
        "docs/migration-guide.md",
        "docs/files-and-vector-stores.md",
        "docs/architecture-note.md",
        "docs/api-coverage.md",
        "docs/intentional-gaps.md",
        "docs/maintenance.md",
        "examples/responses_quickstart.rs",
        "examples/responses_streaming.rs",
        "examples/structured_outputs.rs",
        "examples/request_metadata.rs",
        "examples/embeddings.rs",
        "examples/upload_to_vector_store.rs",
        "examples/chat_completions_migration.rs",
    ] {
        assert!(
            root.join(relative).exists(),
            "expected `{relative}` to exist"
        );
    }
}

#[test]
fn docs_reference_current_commands_and_examples() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let quickstart = fs::read_to_string(root.join("docs/quickstart.md")).expect("quickstart guide");
    let migration =
        fs::read_to_string(root.join("docs/migration-guide.md")).expect("migration guide");
    let maintenance =
        fs::read_to_string(root.join("docs/maintenance.md")).expect("maintenance guide");
    let coverage = fs::read_to_string(root.join("docs/api-coverage.md")).expect("coverage note");
    let gaps = fs::read_to_string(root.join("docs/intentional-gaps.md")).expect("gap note");

    for needle in [
        "cargo run --example responses_quickstart",
        "cargo run --example responses_streaming",
        "cargo run --example structured_outputs",
        "cargo run --example request_metadata",
        "cargo run --example embeddings",
        "cargo run --example upload_to_vector_store",
        "cargo run --example chat_completions_migration",
    ] {
        assert!(
            quickstart.contains(needle) || maintenance.contains(needle),
            "expected docs to reference `{needle}`"
        );
    }

    assert!(
        migration.contains("Responses is the primary surface"),
        "migration guide should position Responses as primary"
    );
    assert!(
        migration.contains("client.chat().completions()")
            && migration.contains("client.completions()"),
        "migration guide should mention compatibility namespaces"
    );

    for command in [
        "cargo fmt --all --check",
        "cargo check --all-targets --all-features",
        "cargo test --all-features -- --test-threads=5",
        "cargo clippy --all-targets --all-features -- -D warnings",
        "RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --all-features",
    ] {
        assert!(
            maintenance.contains(command),
            "maintenance guide should include `{command}`"
        );
    }

    for example in [
        "examples/responses_quickstart.rs",
        "examples/responses_streaming.rs",
        "examples/structured_outputs.rs",
        "examples/request_metadata.rs",
        "examples/upload_to_vector_store.rs",
        "examples/chat_completions_migration.rs",
    ] {
        assert!(
            coverage.contains(example),
            "coverage note should map to `{example}`"
        );
    }

    for gap in [
        "Azure compatibility",
        "Deprecated Assistants / Threads / Runs",
        "Realtime beta as the primary contract",
    ] {
        assert!(gaps.contains(gap), "gap note should mention `{gap}`");
    }
}
