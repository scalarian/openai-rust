---
name: sdk-core-worker
description: Implements shared Rust SDK runtime foundations such as config, transport, errors, retries, metadata, and blocking support.
---

# sdk-core-worker

NOTE: Startup and cleanup are handled by `worker-base`. This skill defines the work procedure.

## When to Use This Skill

Use this skill for features that change the shared runtime: client construction, auth/env loading, config, headers, retry/timeout policy, transport boundaries, response metadata, raw-response wrappers, error taxonomy, shared parser infrastructure, streaming core helpers, and feature-gated blocking support.

## Required Skills

- `brainstorming` — invoke before changing public client ergonomics, shared abstractions, or feature-flag behavior.
- `test-driven-development` — invoke before editing code; add failing tests first.
- `verification-before-completion` — invoke after implementation and before handoff so completion claims are evidence-backed.
- `systematic-debugging` — invoke if validators, live smokes, or transport behavior fail unexpectedly.

## Work Procedure

1. Read `mission.md`, `AGENTS.md`, `.factory/library/architecture.md`, `.factory/library/environment.md`, `.factory/library/user-testing.md`, and `.factory/services.yaml` before editing.
2. Invoke `brainstorming` if the feature changes shared public API shape, metadata wrappers, blocking behavior, or transport layering. In mission Exec Mode, the requirement is satisfied by invoking the skill, comparing at least two plausible design directions, and recording the chosen direction plus rejected alternative in your notes/handoff; do not wait for a user approval loop that is unavailable in non-interactive execution.
3. Invoke `test-driven-development` and add failing tests first. Prefer core-focused tests in the smallest seam that proves the contract:
   - config/env/auth tests
   - header/URL construction tests
   - retry/timeout/error tests
   - metadata/raw-response tests
   - feature-flag matrix tests for blocking
4. Implement the smallest shared-core change that makes the new tests pass. Keep `reqwest` private and preserve the existing family/core boundaries.
5. Reuse or extend shared helper seams instead of adding one-off endpoint-specific transport logic.
6. Run targeted validators first, then broader cargo checks for the changed seam.
7. Perform at least one manual verification step appropriate to the feature, such as:
   - inspecting captured mock requests/headers
   - running an env-gated live smoke if the feature fulfills a live assertion
   - verifying request-id exposure or raw-response behavior with a representative command
8. Invoke `verification-before-completion` and run the required validation commands before handoff.
9. In the handoff, be explicit about which shared invariants changed, which commands passed, and what evidence proves the behavior.

## Example Handoff

```json
{
  "salientSummary": "Implemented shared request metadata capture plus user-agent configuration in the core client. Added failing tests first for default headers, custom user-agent override, and request-id extraction; all targeted cargo tests and full lint/typecheck now pass.",
  "whatWasImplemented": "Extended the shared config/client core so every outbound request emits the documented SDK user-agent and preserves request IDs through the success/error metadata surface. Added feature-matrix coverage so blocking builds reuse the same metadata behavior instead of forking a second implementation.",
  "whatWasLeftUndone": "",
  "verification": {
    "commandsRun": [
      {
        "command": "cargo test --test core_headers user_agent_defaults_and_overrides",
        "exitCode": 0,
        "observation": "Verified default SDK user-agent plus custom override behavior against captured mock requests."
      },
      {
        "command": "cargo test --test core_response_meta request_id_is_exposed_on_success_and_error",
        "exitCode": 0,
        "observation": "Confirmed request IDs surface on both typed successes and API-status failures."
      },
      {
        "command": "cargo clippy --all-targets --all-features -- -D warnings",
        "exitCode": 0,
        "observation": "No lint regressions in shared core modules."
      }
    ],
    "interactiveChecks": [
      {
        "action": "Ran a loopback header-capture scenario with explicit org/project/user-agent overrides.",
        "observed": "Observed deterministic auth, org, project, and user-agent headers on the same captured request."
      }
    ]
  },
  "tests": {
    "added": [
      {
        "file": "tests/core_headers.rs",
        "cases": [
          {
            "name": "user_agent_defaults_and_overrides",
            "verifies": "Default SDK user-agent emission and documented custom override behavior."
          },
          {
            "name": "org_and_project_headers_are_conditional",
            "verifies": "Configured org/project headers are emitted only when present."
          }
        ]
      },
      {
        "file": "tests/core_response_meta.rs",
        "cases": [
          {
            "name": "request_id_is_exposed_on_success_and_error",
            "verifies": "Request IDs are preserved through success metadata and API-status errors."
          }
        ]
      }
    ]
  },
  "discoveredIssues": []
}
```

## When to Return to Orchestrator

- The feature requires a new architectural boundary not described in `.factory/library/architecture.md`.
- A shared-core change would force a breaking API redesign across already-planned features.
- The live assertion depends on an unavailable entitlement or credential path.
- Transport or parsing behavior appears to require a policy decision that is not obvious from the mission or validation contract.
