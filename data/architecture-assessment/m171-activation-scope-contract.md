# M171 Activation Scope Contract

## Purpose

M171 addresses three requested next steps together:

1. production-style UniversalKBQueue activation readiness;
2. environment-specific longer queue soak;
3. richer write-path inventory categories.

This contract keeps the work local, auditable, and non-destructive.

## Allowed work

M171 may:

- create activation checklists, runbooks, readiness assessments, and local evidence artifacts;
- run bounded local queue soak profiles using temporary SQLite databases;
- add small local scripts for soak profile execution if useful;
- improve `scripts/inventory_write_paths.py` category precision with tests;
- regenerate M171 architecture assessment artifacts.

## Non-goals

M171 must not:

- start real production workers;
- connect queue workers to external production services;
- push commits or branches to a remote;
- mutate cloud, GitHub, database, or deployment state;
- weaken test architecture, onion, or write-path guardrails;
- use broad inventory categories that hide real shared-state risk;
- claim full production readiness beyond bounded local evidence.

## Pass conditions

### Activation readiness

- Activation checklist exists.
- Required local gates are explicit.
- Rollback and stop conditions are explicit.
- Readiness verdict says what is ready locally and what remains before real production.

### Environment-specific soak

- Profile parameters are explicit.
- Soak uses separate processes and queue connections.
- JSON and markdown results are written.
- All jobs complete exactly once.
- No worker errors or stuck workers occur.

### Inventory categories

- Richer categories are designed before editing.
- GitNexus impact analysis is run before scanner edits.
- Focused tests cover category behavior.
- Inventory remains `unknown=0`.
- Real shared-state risk remains visible.

## Required closeout checks

```text
uv run python scripts/verify_test_architecture.py --json
uv run python scripts/verify_onion_layering.py --json
uv run python scripts/inventory_write_paths.py --json <m171-final>.json --markdown <m171-final>.md
uv run pytest <focused M171 tests> -q
uv run ruff check <changed Python files>
uv run pyrefly check
uv run pre-commit run --all-files
gitnexus_detect_changes
```

## Claim boundary

M171 may claim:

```text
Queue activation readiness is locally assessed, environment soak evidence exists, and write-path inventory categories are more precise.
```

M171 must not claim:

```text
Production workers have been activated.
The system is fully production-ready.
All possible write-path concurrency risks are solved.
```
