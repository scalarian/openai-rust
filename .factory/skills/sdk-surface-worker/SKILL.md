---
name: sdk-surface-worker
description: Implements endpoint families, typed request/response models, pagination, multipart flows, and family-scoped helpers.
---

# sdk-surface-worker

NOTE: Startup and cleanup are handled by `worker-base`. This skill defines the work procedure.

## When to Use This Skill

Use this skill for resource-family features: Responses, Conversations, Chat/Completions compatibility, Embeddings, Models, Moderations, Images, Audio, Files, Uploads, Vector Stores, Batches, Webhooks, Fine-tuning, Evals, Containers, Skills, and Videos.

## Required Skills

- `brainstorming` — invoke before introducing or reshaping public resource-family APIs, builders, or helpers.
- `test-driven-development` — invoke before editing code; add failing family tests first.
- `verification-before-completion` — invoke before handoff so the family contract is verified with evidence.
- `systematic-debugging` — invoke if family tests, multipart handling, pagination, or live-smoke behavior fails unexpectedly.

## Work Procedure

1. Read `mission.md`, `AGENTS.md`, `.factory/library/architecture.md`, `.factory/library/api-surface.md`, `.factory/library/environment.md`, `.factory/library/user-testing.md`, and `.factory/services.yaml`.
2. Invoke `brainstorming` when the feature affects public family ergonomics, helper naming, builder flows, or compatibility labeling. In mission Exec Mode, satisfy this by comparing plausible API/design directions in-session and recording the chosen direction plus a rejected alternative in your notes/handoff; do not wait for an unavailable user approval loop.
3. Invoke `test-driven-development` and add failing tests before implementation. Choose the right validation seam:
   - request-shaping/multipart tests
   - mocked family integration tests
   - pagination or polling helper tests
   - transcript tests for family-specific streaming helpers
   - env-gated live smokes only for assertions that require real API proof
4. Implement the family surface using the shared transport/parser core. Do not add a family-specific HTTP stack or duplicate shared metadata/error logic.
5. Keep family models local unless a type is truly shared across multiple families.
6. Add or update representative examples only when the feature description, validation contract, or existing failing validators explicitly require example coverage. Do not broaden scope with unrelated example/docs churn when the feature is otherwise code-only.
7. Run targeted tests for the family first, then broader cargo validators required by the feature.
8. Perform at least one manual or smoke verification step appropriate to the feature:
   - inspect multipart bodies or captured queries
   - run a representative example against mocks
   - run a low-cost live smoke if the feature fulfills a live assertion
9. Invoke `verification-before-completion` and confirm the changed surface matches the validation contract, docs/examples, and family boundaries.

## Example Handoff

```json
{
  "salientSummary": "Implemented the Files and Uploads surfaces with typed multipart flows, chunked upload helper, and batch lifecycle parsing. Added failing multipart and lifecycle tests first, then verified a tiny live upload smoke with staged credentials.",
  "whatWasImplemented": "Added typed Files/Uploads APIs, multipart helpers, and lifecycle parsing for create/part/complete/cancel flows. Reused the shared transport and metadata core, added helper coverage for chunked uploads, and wired the family into the public client surface with matching examples.",
  "whatWasLeftUndone": "",
  "verification": {
    "commandsRun": [
      {
        "command": "cargo test --test files_contract_mock retrieve_and_delete",
        "exitCode": 0,
        "observation": "Verified typed file retrieval and delete parsing with local path-guard coverage."
      },
      {
        "command": "cargo test --test uploads_contract_mock lifecycle_and_chunking",
        "exitCode": 0,
        "observation": "Verified create/part/complete/cancel plus chunked-upload helper behavior."
      },
      {
        "command": "set -a && . ./creds.txt && set +a && cargo test --test live_files_smoke -- --ignored --nocapture",
        "exitCode": 0,
        "observation": "Uploaded a tiny text fixture and captured a real request ID from the returned file object."
      }
    ],
    "interactiveChecks": [
      {
        "action": "Inspected captured multipart requests for file upload and upload-part creation.",
        "observed": "Confirmed documented part names, filenames, byte fidelity, and purpose fields without extra transport-specific mutations."
      }
    ]
  },
  "tests": {
    "added": [
      {
        "file": "tests/files_contract_mock.rs",
        "cases": [
          {
            "name": "retrieve_and_delete",
            "verifies": "Files retrieve/delete routes, typed parsing, and local id guards."
          }
        ]
      },
      {
        "file": "tests/uploads_contract_mock.rs",
        "cases": [
          {
            "name": "lifecycle_and_chunking",
            "verifies": "Upload create/part/complete/cancel semantics plus helper chunk ordering."
          }
        ]
      }
    ]
  },
  "discoveredIssues": []
}
```

## When to Return to Orchestrator

- The feature appears to require a new shared-core abstraction rather than a family-local change.
- Public docs and current API behavior conflict in a way that materially changes what the SDK should expose.
- The live assertion is blocked by missing entitlement, unavailable model/capability, or a budget decision that is not already documented.
- A compatibility surface would force the primary Responses-first architecture to bend around legacy behavior.
