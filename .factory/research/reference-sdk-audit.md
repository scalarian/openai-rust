# Reference SDK Audit

Raw planning notes captured from the local `references/openai-python` and `references/openai-node` repositories.

## Purpose
Use this file for clean-room surface inventory, ergonomics comparison, and compatibility expectations. Do not copy code or mirror implementation layout directly from the references.

## Main Findings
- Both official SDKs expose broad platform-family groupings rather than a flat endpoint list.
- Responses is the clearly primary generation surface; Chat Completions and legacy Completions remain compatibility surfaces.
- Common user-facing ergonomics include env-backed client construction, raw-response access, pagination helpers, upload/poll helpers, `output_text` helpers, and webhook verification helpers.
- Python has explicit sync + async clients; Node is async-only. For this Rust mission, async-first with optional blocking parity was chosen.
- Beta/source drift exists in the reference repos, so public docs plus current API behavior must outrank local generated inventories when they disagree.

## Reference Paths Inspected
- `references/openai-python/README.md`
- `references/openai-python/api.md`
- `references/openai-python/src/openai/_client.py`
- `references/openai-python/src/openai/resources/**`
- `references/openai-node/README.md`
- `references/openai-node/api.md`
- `references/openai-node/src/client.ts`
- `references/openai-node/src/resources/**`
