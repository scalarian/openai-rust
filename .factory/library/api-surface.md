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
