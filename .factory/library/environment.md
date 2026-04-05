# Environment

Environment variables, external dependencies, and setup notes.

**What belongs here:** required env vars, external API access, local secret-handling rules, and setup notes that workers must know before running live validation.
**What does NOT belong here:** service ports/commands (use `.factory/services.yaml`).

---

## Required Environment Variables
- `OPENAI_API_KEY` — required for live API validation.
- `OPENAI_BASE_URL` — optional override for alternate OpenAI-compatible endpoints; default target is `https://api.openai.com/v1`.
- `OPENAI_ORG_ID` — optional org routing header.
- `OPENAI_PROJECT_ID` — optional project routing header.
- `OPENAI_WEBHOOK_SECRET` — optional for webhook-verification examples/tests when using real signed fixtures.

## Local Secret Handling
- Live-test credentials are staged in `./creds.txt` and ignored by git.
- Never commit `creds.txt`, echo secret values into logs, or paste secret material into source files/tests.
- When a command needs live credentials in a fresh shell, use: `set -a && . ./creds.txt && set +a && <command>`.
- When a validation assertion specifically needs the SDK's default OpenAI host, unset `OPENAI_BASE_URL` in that same shell before running the live command so the check cannot silently hit an override host.
- If live validation fails because the staged project lacks entitlement to a public surface, return to the orchestrator with the exact endpoint and error instead of mocking around it.

## External Dependencies
- No database, queue, or web frontend is required for this mission.
- Mocked integration tests should use in-process localhost HTTP/WebSocket harnesses.
- Live validation depends only on outbound HTTPS/WebSocket access to the OpenAI API.

## Budget and Runtime Constraints
- The user explicitly approved spending up to roughly `$5` total on live validation, but workers must keep prompts, fixtures, and concurrency small.
- Prefer tiny fixtures, minimal prompts, `n=1`, short audio clips, and single-turn flows.
- Treat media, Realtime, and long-running job surfaces as the highest-risk spend categories.
