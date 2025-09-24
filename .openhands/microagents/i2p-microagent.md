---
name: Issue-to-Prompt (i2p) Microagent
type: service
version: 1.0.0
agent: CodeActAgent
triggers: ["i2p", "issue-to-prompt", "prompt augmentation", "linear", "webhook", "ticket"]
---

# Issue-to-Prompt (i2p) Microagent

This microagent converts new issues (e.g., from Linear) into high-quality, structured implementation prompts grounded in the repository's code. It uses dynamic codebase discovery, optional AST search, and domain-aware templates.

## Responsibilities
- Listen for new issue events (Linear webhook)
- Run i2p over the repository to generate an augmented implementation spec
- Optionally post the result back to the issue as a comment
- Provide deterministic structure and guardrails for output

## Inputs
- Title and description of the issue (from Linear or manual trigger)
- Optional metadata (labels, project, team) to improve domain routing

## Outputs
- Structured Markdown prompt with:
  - Executive Summary
  - Project Context
  - Implementation Scope (files/areas)
  - Expert Analysis
  - Atomic Implementation Units
  - Testing & Validation
  - Deployment Procedures
  - Success Metrics

## Service Interface
- HTTP
  - GET /health
  - POST /linear/webhook → body: Linear Issue payload (id, title, description)

## Environment
- OPENROUTER_API_KEY (required)
- LINEAR_API_KEY (optional, enables comment post-back)
- LINEAR_API_URL (default https://api.linear.app/graphql)
- LINEAR_POST_BACK (true|false, default true)
- I2P_SERVICE_PORT (default 8081)

## Container
- Dockerfile: .evals/Dockerfile.i2p
- Image name (GHCR): ghcr.io/<org>/<repo>-i2p
- Healthcheck: GET /health

## CI/CD
- Workflow: .github/workflows/i2p-docker.yml
- Actions: hadolint → buildx → metadata → build-push
- Tags: ref, PR, semver, sha, latest on default branch

## Execution Flow
1. Webhook received from Linear (Issue Created/Updated)
2. Extract title + description → compose issue_text
3. Run .evals/i2p.py issue_text --llm
4. Read .evals/i2p_dynamic_prompt.md
5. If LINEAR_POST_BACK=true and LINEAR_API_KEY present → add comment to issue

## Quality Guardrails
- Schema/section validation for output completeness
- Reference checker: validate file paths in output exist
- Token budgeting: adapt file selection depth
- Optional webhook signature verification

## Failure Modes & Handling
- Missing LLM key → return 500 with actionable message
- File discovery returns empty context → degrade gracefully to structure-only spec with diagnostics
- Linear API failure → return 202 with result stored but not posted; retry policy external

## Security
- Do not log secrets
- Optional HMAC verification for webhooks
- Truncate outbound comments to safe limits

## Observability
- Request logs, duration, and basic counters
- Optional OpenTelemetry integration (future)

## Local Runbook
- python3 -m venv .venv && source .venv/bin/activate
- pip install -r .evals/requirements-service.txt
- export OPENROUTER_API_KEY=... [LINEAR_API_KEY=...]
- uvicorn .evals.i2p_service:app --host 0.0.0.0 --port 8081
- curl -X POST http://localhost:8081/linear/webhook -H 'Content-Type: application/json' \
  -d '{"title":"Improve JWT auth","description":"add rate limiting + Argon2 tuning"}'

## Ownership
- Maintainer: DevOps and Infrastructure Specialist
- Reviewers: API Security Specialist, Database Performance Specialist

## Enhancements Roadmap
- Add embeddings-assisted file selection
- Strict JSON schema with autovalidation
- AST-grep integration and fallback to ripgrep
- Linear label-aware domain routing
