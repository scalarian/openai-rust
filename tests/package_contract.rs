use std::{fs, path::Path};

#[test]
fn cargo_metadata_is_publish_ready_and_apache_licensed() {
    let manifest =
        fs::read_to_string(repo_root().join("Cargo.toml")).expect("Cargo.toml should exist");

    for expected in [
        "name = \"openai-rust\"",
        "version = \"0.1.0\"",
        "license = \"Apache-2.0\"",
        "repository = \"https://github.com/scalarian/openai-rust\"",
        "documentation = \"https://docs.rs/openai-rust\"",
        "homepage = \"https://github.com/scalarian/openai-rust\"",
        "keywords = [\"openai\", \"api\", \"sdk\", \"responses\", \"rust\"]",
        "categories = [\"api-bindings\", \"asynchronous\"]",
    ] {
        assert!(
            manifest.contains(expected),
            "Cargo.toml should contain `{expected}` for publish-ready metadata"
        );
    }

    let license = fs::read_to_string(repo_root().join("LICENSE"))
        .expect("Apache-2.0 license file should exist at the repository root");
    assert!(
        license.contains("Apache License") && license.contains("Version 2.0, January 2004"),
        "LICENSE should contain the Apache-2.0 text"
    );
}

#[test]
fn publish_filters_exclude_local_only_material() {
    let manifest =
        fs::read_to_string(repo_root().join("Cargo.toml")).expect("Cargo.toml should exist");

    for expected in [
        "\"src/**\"",
        "\"tests/**\"",
        "\"examples/**\"",
        "\"docs/**\"",
    ] {
        assert!(
            !manifest.contains(expected),
            "Cargo.toml should not rely on an include-only package list for `{expected}`"
        );
    }

    for forbidden in [
        "exclude = [",
        "\".factory/**\"",
        "\"references/**\"",
        "\"creds.txt\"",
    ] {
        assert!(
            manifest.contains(forbidden),
            "Cargo.toml should exclude `{forbidden}`"
        );
    }
}

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}
