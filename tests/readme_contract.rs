use std::{fs, path::Path};

mod support;

use support::markdown::{
    CargoPackageMetadata, ExampleCommandValidation, FencedBlockLanguage,
    assert_markdown_rust_block_compiles, extract_command_lines, extract_fenced_blocks,
    parse_markdown_links, validate_command_line,
};

#[test]
fn readme_sections_links_and_claims_resolve_from_the_published_markdown() {
    let readme = fs::read_to_string(repo_root().join("README.md")).expect("README.md should exist");
    for section in [
        "# openai-rust",
        "## Quickstart",
        "## Capability Overview",
        "## Start Here",
        "## Crate Map",
        "## Contributing and Releases",
        "## License",
        "> [!WARNING]",
    ] {
        assert!(
            readme.contains(section),
            "README.md should contain section `{section}`"
        );
    }

    let links = parse_markdown_links("README.md", &readme);
    for link in links {
        if link.target.starts_with("http://") || link.target.starts_with("https://") {
            continue;
        }

        assert!(
            repo_root().join(&link.target).exists(),
            "{} should point at an existing local artifact `{}`",
            link.location(),
            link.target
        );
    }

    assert!(
        readme.contains("not an official OpenAI product"),
        "README.md should include the non-affiliation warning"
    );
    assert!(
        readme.contains("actions/workflows/ci.yml/badge.svg"),
        "README.md should include the CI badge"
    );

    for exported_surface in [
        ("`openai_rust::OpenAI`", "src/lib.rs"),
        ("`openai_rust::resources`", "src/resources/mod.rs"),
        ("`openai_rust::realtime`", "src/realtime"),
        ("`openai_rust::helpers`", "src/helpers"),
        ("`openai_rust::blocking`", "src/blocking"),
    ] {
        assert!(
            readme.contains(exported_surface.0),
            "README.md should mention {}",
            exported_surface.0
        );
        assert!(
            repo_root().join(exported_surface.1).exists(),
            "README claim {} should resolve to {}",
            exported_surface.0,
            exported_surface.1
        );
    }
}

#[test]
fn readme_fenced_snippets_and_commands_validate_against_real_markdown_locations() {
    let readme_path = repo_root().join("README.md");
    let fenced_blocks = extract_fenced_blocks(&readme_path).expect("README blocks should parse");
    assert_eq!(fenced_blocks.len(), 3, "README.md fence count drifted");

    for block in fenced_blocks {
        match block.language {
            FencedBlockLanguage::Rust => assert_markdown_rust_block_compiles(&block),
            FencedBlockLanguage::Shell => {
                for command in extract_command_lines(&block) {
                    validate_command_line(
                        &command,
                        &CargoPackageMetadata::read(repo_root().join("Cargo.toml")),
                        &ExampleCommandValidation::from_repo_root(repo_root()),
                    );
                }
            }
            FencedBlockLanguage::Other(_) => {}
        }
    }
}

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}
