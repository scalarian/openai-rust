# Rust SDK Design Research

Raw planning notes for the Rust-side architecture choices.

## Chosen Direction
- Single publishable crate
- Async-first public API
- Shared private transport/parsing core
- Feature-gated blocking facade over the same operation descriptions
- `reqwest` + `serde`-native implementation
- Stable Rust only

## Why
- Keeps endpoint logic in one canonical path.
- Avoids leaking `reqwest` internals into the public API.
- Makes pagination, multipart, retries, metadata capture, and streaming reusable across resource families.
- Keeps long-term maintenance manageable as OpenAI adds new endpoint families.

## Testing Stack
- Unit tests for config, URL/header shaping, parser behavior, retry logic, and helper logic
- Mocked integration tests for endpoint request/response contracts
- Transcript-driven SSE/WebSocket tests
- Env-gated live smoke tests for representative real API flows
- Packaging/docs validation as part of release quality

## Main Risks
- Duplicating transport logic between async and blocking
- Letting family modules bypass shared metadata/error behavior
- Overusing shared model modules when family-local types would be clearer
- Treating live-test budget as unlimited
