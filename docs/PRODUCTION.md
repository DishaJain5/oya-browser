# agentchrome — production checklist

Derived by Keel from this repository on adoption; edit it to fit, and keep it true. The reviewers under `.claude/agents/` enforce it.

| | |
|---|---|
| Languages | typescript |
| Stack | anthropic, express, next, postgres, sentry, supabase, typescript, vitest |
| Runs on | Supabase + AWS + Kubernetes + containers |
| Gate | `npm run typecheck && npm test` |
| Closest template | AI agent backend (like Open WebUI) |

## The gate

`npm run typecheck && npm test` is the one command that says whether the project is good. Keel runs it after every turn; CI runs it before every build; nobody says "done" without it. For TypeScript that is the typecheck (`tsc --noEmit`) and the tests; lint is a bonus, types are the point.

## Where it stands

Readiness 16/100 when this file was written. In order:

1. **Make it safe** — Nothing else matters while someone can run code on your machine or read a secret from the repository.
   - Request bodies are not validated with a schema — Add zod; parse every body and query with `.strict()` schemas shared with the frontend; return 400 with the issues.
   - 19 actions pinned to a tag, not a commit — in 4 workflows — Pin each `uses:` to a commit SHA with the version in a comment (`actions/checkout@<sha> # v4`), and let Dependabot's `github-actions` ecosystem keep them current.
2. **Make it checkable** — One command that says whether it is good. Every later step is verified by this one.
   - Agent team incomplete: missing `pm`, `designer`, `principal`, `qa`, `reviewer`, `security`, `reliability`, the lifecycle in CLAUDE.md and the frontend skill — Add the missing agents under `.claude/agents/`, the production checklist they enforce (`docs/PRODUCTION.md`), the lifecycle and engineering rules in CLAUDE.md, and the `frontend` skill when there is a frontend. Keel never overwrites a file that exists.
   - The agent instructions are thin — Rewrite CLAUDE.md from the code: the gate, a file map, the request path, the invariants (with the tests), how to extend, how to run. The review does this when asked to fix the docs.
   - Pushes queue instead of cancelling — in 2 workflows — Add `concurrency: { group: ${{ github.workflow }}-${{ github.ref }}, cancel-in-progress: true }` (without cancel-in-progress on the deploy workflow, so a rollout is never cut in half).
   - Jobs can hang for six hours — in 4 workflows — Set `timeout-minutes:` on every job (15 for checks, 30 for a build and deploy).
3. **Make it deployable the same way twice** — An image, a compose file for the laptop, manifests for the cluster, dev and prod apart, a pipeline that does it — so production is not a person.
   - No .env.example — Add a .env.example listing every variable the code reads, with a comment each and no values — and read config only from the environment.
   - The container runs as root — Add a non-root user (`addgroup --system nodejs && adduser --system app`) and `USER app` before the final `CMD`; make the working directory owned by it.
   - The container runs as root — Add a non-root user (`addgroup --system nodejs && adduser --system app`) and `USER app` before the final `CMD`; make the working directory owned by it.
   - The container runs as root — Add a non-root user (`addgroup --system nodejs && adduser --system app`) and `USER app` before the final `CMD`; make the working directory owned by it.
   - Deploys without waiting for the checks — in 2 workflows — Run the gate in a `check` job in the same workflow and give the build and deploy jobs `needs: check`.
4. **Make it survive a bad day** — Rollouts that drop nothing, a schema the repository can recreate, limits that keep one client from being an outage.
   - No health endpoint — Add `/api/health` (alive, cheap) and `/api/health/ready` (checks the database) and point the probes at them.
5. **Own where it runs** — Build around the cloud you already pay for; know the ceiling of the platform you started on; keep the data somewhere the repository can recreate.
   - Data and auth live in Supabase — Migration path: keep Postgres — Supabase is Postgres — but own it: every schema change as a migration in the repository, auth through the `auth` template's sessions when the Supabase auth model stops fitting, and a compose Postgres for dev so nobody develops against production data.
   - Already on AWS — build around it — Set the registry and hosts in `k8s/overlays/{dev,prod}` to the AWS equivalents; the workflows only need the login step changed.

## Change discipline

- Small commits, one thing each, conventional messages. A change appends to `CHANGELOG.md`: past tense, what and why.
- Prefer a dependency the project already has over a new one.
- Config comes from the environment and every variable is in `.env.example` with a comment. Secrets are never in the repository — they live in the cloud's secret manager and are rendered at deploy.
- Schema changes are migration files in the repository, applied before the new version serves — expand/contract, N−1 compatible, never a rename in place.
- Logs are one JSON object per line with `request_id`; no bare `console.log` in the backend.

## Production rules

Checked by the `reliability` and `security` reviewers; argue with the rule here, not in a PR.

- Handlers are stateless; session and rate-limit state live in Redis or a signed cookie, never in process memory once there are replicas.
- Every outbound call has a timeout shorter than its caller's, and the deadline travels in a header. Retries: bounded, exponential backoff with jitter, idempotent operations only.
- A mutating route a client may retry takes `Idempotency-Key`, stored with the request hash and the response. Exactly-once is at-least-once plus an idempotent consumer.
- "Write the row and publish the event" is one transaction through an outbox; every consumer has a dead-letter path and a replay tool.
- Pagination is keyset, capped at 100. IDs are UUIDs (v7 where the database sorts on them).
- Inputs are validated once at the boundary with a schema; unknown fields are rejected; bodies are bounded. Between services: short-lived signed tokens with an audience.
- Rate limits per principal (`429` + `RateLimit-*`); auth endpoints also lock out.
- Three probes with three meanings: startup, readiness (checks dependencies), liveness (never does). Drain on SIGTERM; a `preStop` sleep so endpoints are removed first.
- Circuit breakers and per-dependency concurrency limits on every external call; degrade with a typed "unavailable", never a 500.
- Images run as a non-root user, pinned base, `.dockerignore`, tagged by commit SHA — `latest` is an alias, never what a cluster pulls.
- CI: `permissions: contents: read` by default, actions pinned to SHAs, a concurrency group, a timeout on every job, and no deploy that does not `needs:` the gate. Production deploys from a tag or a dispatch into a protected environment.
- Alerts fire on SLO burn rate and link to a runbook. Backups are only real once restored.

## Deploying

Data and auth in Supabase: make sure the repository can recreate the schema (migrations here, not in the dashboard), and run dev against a local database, never production data.
