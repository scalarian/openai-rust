use std::{fs, path::PathBuf};

#[test]
fn repo_health_files_and_ci_scaffolding_exist() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for relative in [
        "CODE_OF_CONDUCT.md",
        "SECURITY.md",
        "SUPPORT.md",
        "CHANGELOG.md",
        ".github/CODEOWNERS",
        ".github/pull_request_template.md",
        ".github/ISSUE_TEMPLATE/bug_report.yml",
        ".github/ISSUE_TEMPLATE/feature_request.yml",
        ".github/ISSUE_TEMPLATE/docs_issue.yml",
        ".github/ISSUE_TEMPLATE/config.yml",
        ".github/workflows/ci.yml",
        "docs/scripts/check_links.sh",
        "docs/scripts/generate_llms_exports.sh",
    ] {
        assert!(
            root.join(relative).exists(),
            "expected `{relative}` to exist"
        );
    }

    let ci = fs::read_to_string(root.join(".github/workflows/ci.yml")).expect("ci workflow");
    for command in [
        "cargo fmt --all --check",
        "cargo test --workspace",
        "docs/scripts/check_links.sh",
        "docs/scripts/generate_llms_exports.sh",
        "cargo package --workspace --allow-dirty --no-verify",
    ] {
        assert!(ci.contains(command), "workflow should run `{command}`");
    }

    let codeowners =
        fs::read_to_string(root.join(".github/CODEOWNERS")).expect("CODEOWNERS should exist");
    assert!(
        codeowners.contains("@staticpayload"),
        "CODEOWNERS should reference the maintainer handle"
    );

    let changelog =
        fs::read_to_string(root.join("CHANGELOG.md")).expect("CHANGELOG.md should exist");
    assert!(
        changelog.contains("## Unreleased"),
        "CHANGELOG.md should keep an Unreleased section"
    );
}
