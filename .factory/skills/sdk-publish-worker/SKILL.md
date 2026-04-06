---
name: sdk-publish-worker
description: Implements README/docs/examples/benches, validation harnesses, packaging metadata, and publish-readiness work.
---

# sdk-publish-worker

NOTE: Startup and cleanup are handled by `worker-base`. This skill defines the work procedure.

## When to Use This Skill

Use this skill for documentation, examples, README alignment, docs guides, benches, crate metadata, cargo packaging, architecture/coverage/gap notes, and other publish-readiness features.

## Required Skills

- `brainstorming` — invoke before reshaping public docs/example narratives or adding publish-facing materials that set developer expectations.
- `test-driven-development` — invoke before editing docs/examples; add failing validation harnesses or example checks first.
- `verification-before-completion` — invoke before handoff so docs/package claims are backed by mechanical validation.
- `systematic-debugging` — invoke if doc tests, example compiles, package checks, or README validation fail unexpectedly.

## Work Procedure

1. Read `mission.md`, `AGENTS.md`, `.factory/library/architecture.md`, `.factory/library/api-surface.md`, `.factory/library/user-testing.md`, and `.factory/services.yaml`.
2. Invoke `brainstorming` when changing the public story: README quickstart, migration guidance, guide structure, example inventory, or publish positioning.
3. Invoke `test-driven-development` and add failing validation harnesses first. Good first moves include:
   - extracting and validating the actual fenced README snippets/commands
   - example compile checks and representative example smoke runs
   - extracting and validating the actual docs guide snippets/commands
   - `cargo package` / manifest metadata assertions
4. Update docs/examples only after the validation harness expresses what "correct" means.
5. Keep docs aligned with the actual crate API. Do not add aspirational examples or unsupported feature claims.
6. Run the doc/example/package validators first, then the broader cargo validation commands required by the feature.
7. Perform representative smoke checks for examples and onboarding flows, not just compile-only checks.
8. Invoke `verification-before-completion` and confirm README, docs guides, examples, package assets, and publish notes are all mechanically validated from the actual published Markdown/snippets; do not rely on proxy string checks or stand-in fixtures when the feature claims mechanical validation.

## Example Handoff

```json
{
  "salientSummary": "Updated the README, docs guides, and examples to match the final Responses-first API and added validation harnesses for README snippets, representative examples, and package contents. All publish-facing checks now pass.",
  "whatWasImplemented": "Added the final publish-facing materials: a README quickstart aligned to the shipped client, migration guidance for compatibility surfaces, docs guides for key workflows, representative examples, and package/metadata validation so the repository can be published without stale claims or missing assets.",
  "whatWasLeftUndone": "",
  "verification": {
    "commandsRun": [
      {
        "command": "cargo test --test readme_contract",
        "exitCode": 0,
        "observation": "Validated every README snippet and documented shell command against the current crate API."
      },
      {
        "command": "cargo check --examples --all-features",
        "exitCode": 0,
        "observation": "All shipped examples compile under the declared feature set."
      },
      {
        "command": "cargo package --allow-dirty && cargo package --list",
        "exitCode": 0,
        "observation": "Publish artifact includes intended assets and excludes `creds.txt` plus local reference material."
      }
    ],
    "interactiveChecks": [
      {
        "action": "Ran the documented onboarding flow from README commands and one representative example smoke.",
        "observed": "The documented build/test/first-request flow worked without extra manual fixes, and the representative example matched the published API."
      }
    ]
  },
  "tests": {
    "added": [
      {
        "file": "tests/readme_contract.rs",
        "cases": [
          {
            "name": "all_readme_snippets_validate",
            "verifies": "README code blocks and documented commands remain mechanically valid."
          }
        ]
      },
      {
        "file": "tests/package_contract.rs",
        "cases": [
          {
            "name": "publish_artifact_contains_expected_assets",
            "verifies": "The packaged crate includes intended assets and excludes secrets/local-only material."
          }
        ]
      }
    ]
  },
  "discoveredIssues": []
}
```

## When to Return to Orchestrator

- The implementation surface is still unstable enough that docs/examples would become stale immediately.
- Required publish metadata or licensing decisions are missing from mission guidance.
- Example/runtime validation exposes a systemic product bug better handled by a non-publish feature first.
