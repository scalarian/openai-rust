# OpenAI Public API Research (2026)

Raw planning notes for the current public OpenAI API surface used by this mission.

## Primary/Current Public Surfaces
- Responses
- Conversations
- Realtime GA
- Embeddings
- Images
- Audio (transcription, translation, speech)
- Files
- Uploads
- Vector Stores
- Fine-tuning
- Models
- Moderations
- Batches
- Evals
- Webhooks
- Containers
- Videos
- Other current OpenAI-only public families confirmed during artifacting

## Compatibility/Public But Secondary
- Chat Completions
- Legacy Completions

## Explicitly Deprecated or Out of Scope
- Assistants / Threads / Runs
- Realtime beta as the primary contract
- Azure compatibility

## Important Behavioral Notes
- Responses is the preferred modern surface for new work.
- Unknown additive fields/events must be tolerated; the platform evolves quickly.
- Structured outputs and tool-calling paths need first-class typed treatment.
- Streaming is event-rich, not just token deltas.
- Realtime needs its own websocket event model but should follow the same error/typing principles as REST streaming.

## Live Validation Implications
- Use tiny prompts and fixtures.
- Capture request IDs.
- Prefer tolerant live assertions over brittle generated-text expectations.
- Treat entitlement-gated failures as blockers to escalate, not as reasons to silently skip implementation.
