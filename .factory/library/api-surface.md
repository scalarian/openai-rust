# API Surface

High-level scope map for the crate. This file is for workers choosing where new code belongs.

## Primary Modern Surface
- Responses
- Conversations
- Input-items and input-token helpers
- Structured outputs
- Tool calling
- Streaming helpers

### Responses streaming invariant
- Background resume uses the server event `sequence_number` contract for `starting_after`; resume helpers and client-side stream filters must key off each event's `sequence_number`, not a local ordinal counter.

## Compatibility Surfaces
- Chat Completions
- Stored chat completions and stored message listing
- Legacy Completions

### Compatibility invariants
- Chat Completions streams require at least one terminal chunk with a real `finish_reason`; `[DONE]` alone is not sufficient proof of a valid terminal chat completion.
- Stored chat-completion retrieval must tolerate `choices[].message.tool_calls: null`; live stored records may return `null` instead of an array or omitted field for that compatibility-only message shape.
- Legacy Completions streamed and non-streamed payloads are the same `text_completion` shape; `[DONE]` alone is not sufficient proof of a valid terminal completion payload.
- Legacy Completions rejects `best_of` when `stream=true`; treat that combination as an invalid compatibility request.

### Shared query serialization note
- `responses` percent-encodes query keys and values, but `chat` and `conversations` currently use local `append_query` helpers that concatenate raw strings. When adding or fixing list/filter helpers, prefer percent-encoded query serialization and avoid copying the raw concatenation pattern.

## Core Retrieval Surfaces
- Embeddings
- Models
- Moderations

## Media Surfaces
- Images generation/edit/variation
- Audio transcription
- Audio translation
- Audio speech generation
- Shared multimodal input helpers used by Responses/Chat

## File and Retrieval Workflow Surfaces
- Files
- Uploads
- Vector Stores
- Vector Store Files
- Vector Store File Batches
- Batches
- Webhooks

## Advanced Platform Surfaces
- Fine-tuning
- Evals
- Containers
- Skills (if the public surface is available to the staged project)
- Videos

## Realtime Surface
- Realtime client secrets
- Realtime call helpers
- GA websocket session/event model

## Explicit Exclusions
- Azure compatibility
- Deprecated Assistants/Threads/Runs flows
- Realtime beta-only semantics as the primary contract
