use std::{collections::BTreeSet, fs, path::PathBuf};

mod support;

use support::markdown::{
    CargoPackageMetadata, ExampleCommandValidation, FencedBlockLanguage,
    assert_markdown_rust_block_compiles, extract_command_lines, extract_fenced_blocks,
    parse_markdown_links, validate_command_line,
};

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
fn docs_reference_current_commands_and_examples_from_the_published_markdown() {
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

    for (relative, content) in [
        ("docs/quickstart.md", &quickstart),
        ("docs/migration-guide.md", &migration),
        ("docs/maintenance.md", &maintenance),
        ("docs/api-coverage.md", &coverage),
        ("docs/intentional-gaps.md", &gaps),
    ] {
        for link in parse_markdown_links(relative, content) {
            if link.target.starts_with("http://") || link.target.starts_with("https://") {
                continue;
            }

            assert!(
                root.join(&link.target).exists(),
                "{} should point at an existing local artifact `{}`",
                link.location(),
                link.target
            );
        }
    }
}

#[test]
fn docs_fenced_snippets_and_commands_validate_against_real_markdown_locations() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let docs = [
        "docs/quickstart.md",
        "docs/responses-guide.md",
        "docs/migration-guide.md",
        "docs/files-and-vector-stores.md",
        "docs/maintenance.md",
    ];
    let package = CargoPackageMetadata::read(root.join("Cargo.toml"));
    let examples = ExampleCommandValidation::from_repo_root(&root);
    let mut seen_locations = BTreeSet::new();

    for relative in docs {
        let path = root.join(relative);
        let blocks = extract_fenced_blocks(&path).expect("doc blocks should parse");
        assert!(
            !blocks.is_empty(),
            "{relative} should contain fenced blocks for publish validation"
        );

        for block in blocks {
            seen_locations.insert(block.location());
            match block.language {
                FencedBlockLanguage::Rust => assert_markdown_rust_block_compiles(&block),
                FencedBlockLanguage::Shell => {
                    for command in extract_command_lines(&block) {
                        seen_locations.insert(command.location());
                        validate_command_line(&command, &package, &examples);
                    }
                }
                FencedBlockLanguage::Other(_) => {}
            }
        }
    }

    assert!(
        seen_locations
            .iter()
            .any(|location| location.starts_with("docs/quickstart.md:")),
        "expected publish validation to record exact quickstart source locations"
    );
    assert!(
        seen_locations
            .iter()
            .any(|location| location.starts_with("docs/files-and-vector-stores.md:")),
        "expected publish validation to record exact files/vector-stores source locations"
    );
}
