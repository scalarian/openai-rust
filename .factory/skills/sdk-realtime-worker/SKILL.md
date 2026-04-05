---
name: sdk-realtime-worker
description: Implements Realtime GA REST helpers, websocket session flows, event models, transcript tests, and low-risk live realtime verification.
---

# sdk-realtime-worker

NOTE: Startup and cleanup are handled by `worker-base`. This skill defines the work procedure.

## When to Use This Skill

Use this skill for Realtime GA features: client-secret helpers, Realtime call helpers, websocket session/bootstrap flows, event modeling, transcript accumulation, multimodal event families, and Realtime-specific examples/tests.

## Required Skills

- `brainstorming` — invoke before changing Realtime public session/event ergonomics.
- `test-driven-development` — invoke before editing code; add failing transcript or websocket tests first.
- `verification-before-completion` — invoke before handoff so claims about Realtime behavior are backed by command output.
- `systematic-debugging` — invoke immediately if websocket ordering, event decoding, or live Realtime behavior is unstable.

## Work Procedure

1. Read `mission.md`, `AGENTS.md`, `.factory/library/architecture.md`, `.factory/library/environment.md`, `.factory/library/user-testing.md`, and `.factory/services.yaml`.
2. Invoke `brainstorming` when the feature changes event naming, session ergonomics, or Realtime public types.
3. Invoke `test-driven-development` and add failing tests first. Prefer transcript-driven fixtures and local websocket harnesses over ad hoc manual poking.
4. Keep REST Realtime helpers and websocket machinery separate, while reusing shared error/metadata principles.
5. Model events first, then derived helpers. Do not make helper assumptions that bypass the canonical event transcript.
6. Run targeted transcript, websocket, and decode tests before broader validators.
7. Perform a minimal manual/live smoke only when the contract requires it, and keep it text-first unless the assertion explicitly requires richer live proof.
8. If the websocket or event flow fails unexpectedly, invoke `systematic-debugging` before attempting deeper fixes.
9. Invoke `verification-before-completion` and ensure the final handoff names the exact transcript/live evidence that proves the behavior.

## Example Handoff

```json
{
  "salientSummary": "Implemented Realtime client-secret helpers plus GA websocket text-event accumulation. Added failing transcript fixtures for bootstrap, output_text deltas, and terminal response reconciliation before wiring the public session API.",
  "whatWasImplemented": "Added typed Realtime client-secret and websocket session support with explicit unknown-event handling, bootstrap/session-state modeling, and text-response accumulation that reconciles against `response.done`. Reused the shared error taxonomy and kept websocket logic isolated from REST transport internals.",
  "whatWasLeftUndone": "",
  "verification": {
    "commandsRun": [
      {
        "command": "cargo test --test realtime_connection_contract bootstrap_and_clean_close",
        "exitCode": 0,
        "observation": "Verified websocket bootstrap events, stable session identifiers, and clean shutdown against a local harness."
      },
      {
        "command": "cargo test --test realtime_multimodal_output_contract output_items_reconcile_at_response_done",
        "exitCode": 0,
        "observation": "Verified structural output-item and content-part events reconcile into the final response state."
      },
      {
        "command": "set -a && . ./creds.txt && set +a && cargo test --test live_realtime_text_smoke -- --ignored --nocapture",
        "exitCode": 0,
        "observation": "Opened a tiny live Realtime text session and captured a successful terminal response with the expected event sequence."
      }
    ],
    "interactiveChecks": [
      {
        "action": "Reviewed a local websocket transcript for output_text delta ordering and terminal response reconciliation.",
        "observed": "Observed canonical GA event names, deterministic accumulation, and no stale beta aliases in the final typed transcript."
      }
    ]
  },
  "tests": {
    "added": [
      {
        "file": "tests/realtime_connection_contract.rs",
        "cases": [
          {
            "name": "bootstrap_and_clean_close",
            "verifies": "Session bootstrap, stable session ids, and clean socket shutdown."
          }
        ]
      },
      {
        "file": "tests/realtime_multimodal_output_contract.rs",
        "cases": [
          {
            "name": "output_items_reconcile_at_response_done",
            "verifies": "Structural Realtime events reconcile into one terminal response view."
          }
        ]
      }
    ]
  },
  "discoveredIssues": []
}
```

## When to Return to Orchestrator

- Public docs and observed GA behavior disagree in a way that changes the Realtime public contract.
- Live Realtime validation fails due to missing entitlement, handshake requirements, or account configuration outside repository control.
- The feature would require introducing a second, divergent Realtime protocol model instead of extending the canonical event model.
