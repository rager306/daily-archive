---
id: S02
parent: M059-y6osma
milestone: M059-y6osma
provides:
  - Operational validation and replay tooling for M061 manifest-scaled ingest.
  - Decision document for M061 scope and constraints.
requires:
  - slice: S01
    provides: Schemas, retroactive manifests, and ADR-013 manifest-driven ingest decision.
affects:
  []
key_files:
  - scripts/m059_validate_pdf_batch.py
  - scripts/m059_replay_ingest.py
  - scripts/m059_e2e_test.py
  - tests/test_m059_s02.py
  - schemas/opendataloader-pdf.v1.json
  - artifacts/m059-architecture/decision.md
  - artifacts/m059-architecture/m054-validation-report.json
  - artifacts/m059-architecture/m054-grobid-replay-report.json
  - artifacts/m059-architecture/m059-s02-e2e-report.json
key_decisions:
  - Replay remains local and artifact-level for M059 because all safety defaults remain false.
  - M061 is authorized to scale via manifest-gated 2-hop BFS validation/replay, not direct graph import.
patterns_established:
  - Manifest parser expectations resolve per-PDF output templates before schema validation.
  - Deterministic parser replay is proven by SHA-256 byte identity and idempotent skip behavior.
observability_surfaces:
  - Per-PDF CLI pass/fail output with aggregate stats.
  - JSON validation, replay, and e2e reports under artifacts/m059-architecture/.
drill_down_paths:
  - .gsd/milestones/M059-y6osma/slices/S02/tasks/T01-SUMMARY.md
  - .gsd/milestones/M059-y6osma/slices/S02/tasks/T02-SUMMARY.md
  - .gsd/milestones/M059-y6osma/slices/S02/tasks/T03-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-06-12T10:50:31.781Z
blocker_discovered: false
---

# S02: Validation + replay tooling + decision

**S02 delivered validation, replay, e2e proof, and the M061 manifest-gated scale-up decision.**

## What Happened

Built three S02 scripts: `m059_validate_pdf_batch.py` validates parser outputs across a manifest, `m059_replay_ingest.py` verifies deterministic replay by SHA-256 byte identity, and `m059_e2e_test.py` runs the M054 proof and writes reports. Added seven pytest checks for safety defaults, loopback defaults, both parser validations, replay idempotency, CLI output, and e2e report/decision presence. Extended the OpenDataLoader schema to accept the retroactive M055 diagnostic output shape referenced by the M054 manifest. Wrote the M061 decision document under `artifacts/m059-architecture/decision.md`.

## Verification

Passed: `uv run pytest tests/test_m059_s02.py -q` (7/7), `uv run pytest tests/test_m059_s01.py -q` (8/8), M054 GROBID validation 5/5, M054 OpenDataLoader validation 5/5, GROBID replay byte-identical 5/5, `uv run python scripts/m059_e2e_test.py`, M045 trajectory on_track, and M044 guardrail ok.

## Requirements Advanced

None.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

OpenDataLoader schema compatibility was expanded during S02 because the S01 schema referenced by the M054 manifest did not accept the actual M055 retroactive diagnostic outputs. The change preserves the adapter schema while adding a diagnostic oneOf branch.

## Known Limitations

Replay is local artifact-level replay, not live parser invocation, because S02 safety defaults explicitly prohibit external network calls and production mutation.

## Follow-ups

M061 should use the decision doc to scale to 2-hop BFS behind manifest-gated validation and replay checks.

## Files Created/Modified

- `scripts/m059_validate_pdf_batch.py` — New CLI for per-PDF parser output validation across a batch manifest.
- `scripts/m059_replay_ingest.py` — New CLI for deterministic local replay and SHA-256 byte identity reporting.
- `scripts/m059_e2e_test.py` — New M054 e2e runner writing validation, replay, and aggregate reports.
- `tests/test_m059_s02.py` — New pytest coverage for S02 tooling and decision artifact.
- `schemas/opendataloader-pdf.v1.json` — Extended to accept retroactive M055 OpenDataLoader diagnostic outputs via oneOf.
- `artifacts/m059-architecture/decision.md` — Decision doc for M061 manifest-gated 2-hop BFS scale-up.
