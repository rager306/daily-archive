---
id: M065-vq0do4
title: "M062 fd production hardening"
status: complete
provides:
  - production-ready daily-archive fd embedder wrapper
  - binding ADR-019 fd embedding service contract
  - 52-case fd contract evidence suite
  - M062 closeout REPORT, SUMMARY, and VALIDATION artifacts
key_decisions:
  - ADR-019 binds daily-archive fd integration to the fd v2 embedding service contract
  - fd service configuration is env-driven, not hardcoded in source code
  - fd v1 contract gaps are recorded as measured evidence, not hidden behind wrapper success
  - five safety defaults remain false by default
patterns_established:
  - env-driven FD_* configuration for endpoint, model, dimensions, retry, timeout, and circuit settings
  - wrapper resilience plus executable contract evidence before downstream graph decisions
observability_surfaces:
  - Embedder metrics snapshot
  - fd contract report markdown
  - fd actual-vs-required gap analysis
  - machine-readable contract results JSON
requirement_outcomes:
  - id: M062-fd-hardening
    from_status: active
    to_status: validated
    proof: S01/S02/S03/S04 tests and closeout artifacts
duration: M062 S01-S04
verification_result: passed
completed_at: 2026-06-14T00:00:00.000Z
key_files:
  - src/arxiv_archive/embedder.py
  - scripts/test_fd_contract.py
  - tests/test_m062_s01.py
  - tests/test_m062_s02.py
  - tests/test_m062_s03.py
  - tests/test_m062_s04.py
  - doc/adr/ADR-019-fd-embedding-service-contract.md
  - artifacts/m062-fd-contract/REPORT.md
  - artifacts/m062-fd-contract/fd-contract-results.json
  - artifacts/m062-fd-contract/fd-contract-report.md
  - artifacts/m062-fd-contract/fd-actual-vs-required.md
lessons_learned:
  - wrapper hardening needs a machine-readable service contract to avoid guessing retry semantics
  - contract failures are useful closeout evidence when the service under test is still fd v1
  - endpoint, model, dimensions, retry, timeout, and circuit settings must be environment-driven
  - codebase-memory governance mirror must be regenerated after ADR amendments
---

# M065-vq0do4: M062 fd production hardening

**M062 hardened daily-archive fd embedding integration with a production wrapper, binding ADR-019 contract, 52 contract checks, env-driven configuration, and closeout artifacts.**

## What Happened

M062 delivered four connected slices. S01 unified the embedder wrapper in `src/arxiv_archive/embedder.py` and added retry/backoff, circuit breaker, graceful degradation, metrics, and safety-default checks. S02 created ADR-019 as the binding fd embedding service contract for daily-archive. S03 added `scripts/test_fd_contract.py` and generated contract evidence under `artifacts/m062-fd-contract/`. S04 synthesized the milestone into `REPORT.md`, amended ADR-019 with env-driven configuration, regenerated codebase-memory governance, and emitted closeout validation.

The milestone separates daily-archive wrapper readiness from fd service implementation gaps. fd v1 can still miss production endpoints and headers, but those gaps are now explicit contract evidence rather than implicit caller risk.

## Cross-Slice Verification

- S01/S01v2: 14 wrapper and env override tests.
- S02: 8 ADR/service-contract tests.
- S03: 52 contract checks, with 40 passed, 5 failed, 7 skipped against observed fd v1 behavior.
- S04: closeout tests verify REPORT sections, ADR amendment, Amendment Log, closeout artifacts, codebase-memory sync, and M050/M062 regression.
- Safety defaults remain false: graph writes, production import, fact promotion, external network, and LLM calls.
- Default fd endpoint uses `http://127.0.0.1:8000/v1/embeddings`.

## Requirement Changes

- M062-fd-hardening: active → validated — wrapper hardening, ADR-019, contract tests, and S04 closeout evidence complete the planned scope.
- M062-fd-v2-verification: queued — future validation should rerun the same 52 checks after fd v2 deployment.

## Decision Re-evaluation

| Decision | Status | Evidence |
|---|---|---|
| ADR-019 binding fd service contract | Keep | S02 tests and S03 contract results validate ADR usefulness. |
| Env-driven FD configuration | Keep | S01v2 tests and S04 amendment document the user feedback and implementation pattern. |
| M063 proceeds after M062 | Keep | fd hardening is closed; GraphDB selection should not wait for fd v2 deployment. |

## Delivered Evidence

| Evidence | Path | Result |
|---|---|---|
| Final report | `artifacts/m062-fd-contract/REPORT.md` | 8 Russian sections |
| Contract report | `artifacts/m062-fd-contract/fd-contract-report.md` | 52 total, 40 pass |
| Gap analysis | `artifacts/m062-fd-contract/fd-actual-vs-required.md` | P0/P1/P2 actual vs required |
| Machine-readable results | `artifacts/m062-fd-contract/fd-contract-results.json` | Contract evidence JSON |
| Binding ADR | `doc/adr/ADR-019-fd-embedding-service-contract.md` | Section 4.5 + Amendment Log |
| Governance mirror | `.codebase-memory/adr.md` | ADR-019 present after sync |
| Closeout validation | `.gsd/milestones/M065-vq0do4/M065-vq0do4-VALIDATION.md` | pass |

## Known Limitations

- fd v1 still lacks some fd v2 production endpoints and headers; these are recorded as contract failures/skips, not fixed in daily-archive.
- The S03 suite measures actual fd service behavior but does not deploy fd v2.
- Future verification should rerun the same 52 contract checks after fd v2 deployment.

## Follow-ups

- Queue and run `M062-fd-v2-verification` after fd v2 is deployed.
- Use ADR-019 as the binding source for any fd service contract changes.
- Keep fd configuration environment-driven across CI, staging, and production.
