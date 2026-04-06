#![allow(dead_code)]

use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, OnceLock},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FencedBlockLanguage {
    Rust,
    Shell,
    Other(String),
}

#[derive(Clone, Debug)]
pub struct MarkdownCodeBlock {
    pub relative_path: String,
    pub language: FencedBlockLanguage,
    pub info_string: String,
    pub code: String,
    pub start_line: usize,
    pub end_line: usize,
}

impl MarkdownCodeBlock {
    pub fn location(&self) -> String {
        format!(
            "{}:{}-{}",
            self.relative_path, self.start_line, self.end_line
        )
    }
}

#[derive(Clone, Debug)]
pub struct MarkdownCommandLine {
    pub relative_path: String,
    pub command: String,
    pub line: usize,
}

impl MarkdownCommandLine {
    pub fn location(&self) -> String {
        format!("{}:{}", self.relative_path, self.line)
    }
}

#[derive(Clone, Debug)]
pub struct MarkdownLink {
    pub source_path: String,
    pub target: String,
    pub line: usize,
}

impl MarkdownLink {
    pub fn location(&self) -> String {
        format!("{}:{}", self.source_path, self.line)
    }
}

#[derive(Clone, Debug)]
pub struct CargoPackageMetadata {
    pub package_name: String,
}

impl CargoPackageMetadata {
    pub fn read(path: impl AsRef<Path>) -> Self {
        let manifest = fs::read_to_string(path).expect("Cargo.toml should be readable");
        let package_name = manifest
            .lines()
            .skip_while(|line| line.trim() != "[package]")
            .skip(1)
            .find_map(|line| {
                let trimmed = line.trim();
                if trimmed.starts_with("name = ") {
                    trimmed
                        .split('"')
                        .nth(1)
                        .map(std::string::ToString::to_string)
                } else {
                    None
                }
            })
            .expect("package.name should exist in Cargo.toml");
        Self { package_name }
    }
}

#[derive(Clone, Debug)]
pub struct ExampleCommandValidation {
    pub repo_root: PathBuf,
    pub examples: BTreeSet<String>,
}

impl ExampleCommandValidation {
    pub fn from_repo_root(repo_root: impl AsRef<Path>) -> Self {
        let repo_root = repo_root.as_ref().to_path_buf();
        let examples = fs::read_dir(repo_root.join("examples"))
            .expect("examples directory should exist")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension() == Some(OsStr::new("rs")))
            .filter_map(|path| {
                path.file_stem()
                    .and_then(OsStr::to_str)
                    .map(std::string::ToString::to_string)
            })
            .collect();
        Self {
            repo_root,
            examples,
        }
    }
}

pub fn extract_fenced_blocks(path: impl AsRef<Path>) -> Result<Vec<MarkdownCodeBlock>, String> {
    let path = path.as_ref();
    let relative_path = relative_markdown_path(path);
    let content = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let mut blocks = Vec::new();
    let mut in_block = false;
    let mut info_string = String::new();
    let mut block_lines = Vec::new();
    let mut block_start_line = 0usize;

    for (index, line) in content.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim_start();
        if !in_block {
            if let Some(info) = trimmed.strip_prefix("```") {
                in_block = true;
                info_string = info.trim().to_string();
                block_lines.clear();
                block_start_line = line_number + 1;
            }
            continue;
        }

        if trimmed.starts_with("```") {
            let end_line = block_start_line
                .saturating_add(block_lines.len())
                .saturating_sub(1);
            blocks.push(MarkdownCodeBlock {
                relative_path: relative_path.clone(),
                language: parse_block_language(&info_string),
                info_string: info_string.clone(),
                code: block_lines.join("\n"),
                start_line: block_start_line,
                end_line,
            });
            in_block = false;
            info_string.clear();
            block_lines.clear();
            continue;
        }

        block_lines.push(line.to_string());
    }

    if in_block {
        return Err(format!(
            "{}:{}: unterminated fenced block",
            relative_path, block_start_line
        ));
    }

    Ok(blocks)
}

pub fn extract_command_lines(block: &MarkdownCodeBlock) -> Vec<MarkdownCommandLine> {
    block
        .code
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return None;
            }

            Some(MarkdownCommandLine {
                relative_path: block.relative_path.clone(),
                command: trimmed.to_string(),
                line: block.start_line + index,
            })
        })
        .collect()
}

pub fn parse_markdown_links(source_path: &str, content: &str) -> Vec<MarkdownLink> {
    let mut links = Vec::new();
    for (index, line) in content.lines().enumerate() {
        let mut search = line;
        let mut offset = 0usize;
        while let Some(open_bracket) = search.find('[') {
            if open_bracket > 0 && search.as_bytes()[open_bracket - 1] == b'!' {
                search = &search[open_bracket + 1..];
                offset += open_bracket + 1;
                continue;
            }
            let Some(close_bracket) = search[open_bracket..].find("](") else {
                break;
            };
            let link_start = open_bracket + close_bracket + 2;
            let Some(close_paren) = search[link_start..].find(')') else {
                break;
            };
            let target = &search[link_start..link_start + close_paren];
            links.push(MarkdownLink {
                source_path: source_path.to_string(),
                target: target.to_string(),
                line: index + 1,
            });
            let next_index = link_start + close_paren + 1;
            search = &search[next_index..];
            offset += next_index;
            let _ = offset;
        }
    }
    links
}

pub fn assert_markdown_rust_block_compiles(block: &MarkdownCodeBlock) {
    let temp_dir = temp_dir_for("markdown-snippet");
    let source_path = temp_dir.join("snippet.rs");
    let binary_path = temp_dir.join("snippet-bin");
    let wrapper = rust_wrapper_for(block);
    fs::write(&source_path, wrapper).expect("temporary snippet source should be writable");

    let deps_dir = target_deps_dir();
    let openai_rust_rlib = find_rlib(&deps_dir, "openai_rust");
    let serde_json_rlib = find_rlib(&deps_dir, "serde_json");
    let output = Command::new(rustc_path())
        .arg("--edition")
        .arg("2024")
        .arg(&source_path)
        .arg("-o")
        .arg(&binary_path)
        .arg("-L")
        .arg(format!("dependency={}", deps_dir.display()))
        .arg("--extern")
        .arg(format!("openai_rust={}", openai_rust_rlib.display()))
        .arg("--extern")
        .arg(format!("serde_json={}", serde_json_rlib.display()))
        .output()
        .expect("rustc should run for markdown snippet validation");

    assert!(
        output.status.success(),
        "{} failed to compile as published `{}` snippet.\nstdout:\n{}\nstderr:\n{}",
        block.location(),
        block.info_string,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn validate_command_line(
    command: &MarkdownCommandLine,
    package: &CargoPackageMetadata,
    examples: &ExampleCommandValidation,
) {
    if let Some(example_name) = command.command.strip_prefix("cargo run --example ") {
        let example_name = example_name.trim();
        assert!(
            examples.examples.contains(example_name),
            "{} references missing example `{example_name}`",
            command.location()
        );
        run_example_command(command, &examples.repo_root);
        return;
    }

    if let Some(package_name) = command.command.strip_prefix("cargo add ") {
        assert_eq!(
            package_name.trim(),
            package.package_name,
            "{} should document `cargo add {}` for the published crate",
            command.location(),
            package.package_name
        );
        return;
    }

    if command.command == "cargo test --workspace" {
        assert!(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("Cargo.toml")
                .exists(),
            "{} should be rooted in a Cargo project",
            command.location()
        );
        return;
    }

    if command.command.starts_with("cargo test --test ")
        || command.command == "cargo test --doc"
        || matches!(
            command.command.as_str(),
            "cargo check --examples --all-features"
                | "cargo fmt --all --check"
                | "cargo check --all-targets --all-features"
                | "cargo test --all-features -- --test-threads=5"
                | "cargo clippy --all-targets --all-features -- -D warnings"
                | "RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --all-features"
        )
    {
        return;
    }

    panic!(
        "{} documents an unsupported or unvalidated command `{}`",
        command.location(),
        command.command
    );
}

fn parse_block_language(info_string: &str) -> FencedBlockLanguage {
    let tag = info_string
        .split([',', ' ', '\t'])
        .find(|part| !part.is_empty())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match tag.as_str() {
        "rust" => FencedBlockLanguage::Rust,
        "sh" | "shell" | "bash" | "zsh" => FencedBlockLanguage::Shell,
        other => FencedBlockLanguage::Other(other.to_string()),
    }
}

fn rust_wrapper_for(block: &MarkdownCodeBlock) -> String {
    if block.code.contains("fn main") {
        format!(
            "#![allow(unused_imports, unused_variables, dead_code)]\n{}",
            block.code
        )
    } else {
        format!(
            "#![allow(unused_imports, unused_variables, dead_code)]\nfn main() -> Result<(), Box<dyn std::error::Error>> {{\n{}\n    Ok(())\n}}\n",
            indent_block(&block.code)
        )
    }
}

fn indent_block(input: &str) -> String {
    input
        .lines()
        .map(|line| format!("    {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn relative_markdown_path(path: &Path) -> String {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.strip_prefix(&manifest_dir)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn target_deps_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidate = manifest_dir.join("target/debug/deps");
    assert!(
        candidate.exists(),
        "expected compiled dependency directory at {}",
        candidate.display()
    );
    candidate
}

fn find_rlib(dir: &Path, crate_name: &str) -> PathBuf {
    fs::read_dir(dir)
        .expect("target dependency directory should be readable")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(OsStr::to_str)
                .map(|name| {
                    name.starts_with(&format!("lib{crate_name}-")) && name.ends_with(".rlib")
                })
                .unwrap_or(false)
        })
        .unwrap_or_else(|| panic!("missing compiled `{crate_name}` rlib in {}", dir.display()))
}

fn run_example_command(command: &MarkdownCommandLine, repo_root: &Path) {
    static CARGO_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _guard = CARGO_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("cargo example lock should be available");
    let output = Command::new(cargo_path())
        .arg("run")
        .arg("--quiet")
        .arg("--example")
        .arg(
            command
                .command
                .trim_start_matches("cargo run --example ")
                .trim(),
        )
        .current_dir(repo_root)
        .output()
        .expect("cargo run --example should launch");

    assert!(
        output.status.success(),
        "{} failed to execute `{}`.\nstdout:\n{}\nstderr:\n{}",
        command.location(),
        command.command,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn temp_dir_for(prefix: &str) -> PathBuf {
    let unique = format!(
        "{}-{}-{}",
        prefix,
        std::process::id(),
        std::thread::current().name().unwrap_or("markdown")
    )
    .replace(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-', "-");
    let dir = std::env::temp_dir().join(unique);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temporary directory should be creatable");
    dir
}

fn cargo_path() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

fn rustc_path() -> String {
    std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string())
}
