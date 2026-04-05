#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if command -v rustup >/dev/null 2>&1; then
  rustup component add clippy rustfmt >/dev/null 2>&1 || true
fi

if [[ -f "$ROOT/Cargo.toml" ]]; then
  cargo fetch --locked >/dev/null 2>&1 || cargo fetch >/dev/null 2>&1 || true
fi
