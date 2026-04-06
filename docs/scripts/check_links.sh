#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

python3 - <<'PY'
import re
import sys
from pathlib import Path

root = Path.cwd()
files = [
    root / "README.md",
    root / "CODE_OF_CONDUCT.md",
    root / "SECURITY.md",
    root / "SUPPORT.md",
    root / "CHANGELOG.md",
]
files.extend(sorted((root / "docs").glob("*.md")))
files = [path for path in files if path.exists()]

link_re = re.compile(r'(?<!\!)\[[^\]]+\]\(([^)]+)\)')
heading_re = re.compile(r'^\s{0,3}#{1,6}\s+(.*)$')
slug_cleanup_re = re.compile(r'[^\w\- ]')
multi_hyphen_re = re.compile(r'-+')
heading_cache = {}


def slugify(text: str) -> str:
    text = text.strip().lower().replace("`", "")
    text = slug_cleanup_re.sub("", text)
    text = text.replace(" ", "-")
    return multi_hyphen_re.sub("-", text).strip("-")


def anchors_for(path: Path) -> set[str]:
    if path not in heading_cache:
        anchors = set()
        if path.suffix.lower() == ".md":
            for line in path.read_text(encoding="utf-8").splitlines():
                match = heading_re.match(line)
                if match:
                    anchors.add(slugify(match.group(1)))
        heading_cache[path] = anchors
    return heading_cache[path]


errors = []
for source in files:
    for line_number, line in enumerate(
        source.read_text(encoding="utf-8").splitlines(),
        start=1,
    ):
        for target in link_re.findall(line):
            target = target.strip()
            if not target or target.startswith(("http://", "https://", "mailto:")):
                continue

            if target.startswith("#"):
                anchor = target[1:]
                if anchor and anchor not in anchors_for(source):
                    errors.append(
                        f"{source.relative_to(root)}:{line_number}: missing anchor #{anchor}"
                    )
                continue

            path_part, _, anchor = target.partition("#")
            resolved = (source.parent / path_part).resolve()

            try:
                resolved.relative_to(root)
            except ValueError:
                errors.append(
                    f"{source.relative_to(root)}:{line_number}: link escapes repo: {target}"
                )
                continue

            if not resolved.exists():
                errors.append(
                    f"{source.relative_to(root)}:{line_number}: missing path: {target}"
                )
                continue

            if anchor:
                if resolved.suffix.lower() != ".md":
                    errors.append(
                        f"{source.relative_to(root)}:{line_number}: anchor target is not markdown: {target}"
                    )
                    continue
                if anchor not in anchors_for(resolved):
                    errors.append(
                        f"{source.relative_to(root)}:{line_number}: missing anchor in {resolved.relative_to(root)}: #{anchor}"
                    )

if errors:
    print("\n".join(errors))
    sys.exit(1)
PY
