---
id: T01
parent: S02
milestone: M059-y6osma
key_files:
  - scripts/m059_validate_pdf_batch.py
  - scripts/m059_replay_ingest.py
  - schemas/opendataloader-pdf.v1.json
key_decisions:
  - Replay is implemented as local artifact-level deterministic replay because M059 safety defaults prohibit external parser service calls.
duration: 
verification_result: passed
completed_at: 2026-06-12T10:49:35.297Z
blocker_discovered: false
---

# T01: Added manifest-driven batch validation and deterministic replay CLI tools for M059 S02.

**Added manifest-driven batch validation and deterministic replay CLI tools for M059 S02.**

## What Happened

Implemented `scripts/m059_validate_pdf_batch.py` for per-PDF parser output validation against manifest-declared JSON schemas, including manifest schema checks, explicit five-false safety default enforcement, output path template resolution, per-PDF pass/fail reporting, and aggregate stats. Implemented `scripts/m059_replay_ingest.py` for local deterministic artifact replay with SHA-256 byte identity checks, idempotent skip behavior, non-deterministic parser labeling, and no external network, graph write, production import, fact promotion, or LLM behavior.

## Verification

Verified syntax with `uv run python -m py_compile scripts/m059_validate_pdf_batch.py scripts/m059_replay_ingest.py scripts/m059_e2e_test.py`, validated M054 GROBID and OpenDataLoader outputs 5/5 each, and replayed M054 GROBID outputs with byte-identical SHA-256 results.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python -m py_compile scripts/m059_validate_pdf_batch.py scripts/m059_replay_ingest.py scripts/m059_e2e_test.py` | 0 | ✅ pass | 1000ms |
| 2 | `uv run python scripts/m059_validate_pdf_batch.py --manifest=artifacts/m054-pdf-acquisition/manifest.json --parser=grobid` | 0 | ✅ pass | 4200ms |
| 3 | `uv run python scripts/m059_validate_pdf_batch.py --manifest=artifacts/m054-pdf-acquisition/manifest.json --parser=opendataloader` | 0 | ✅ pass | 4200ms |
| 4 | `uv run python scripts/m059_replay_ingest.py --manifest=artifacts/m054-pdf-acquisition/manifest.json --parser=grobid --output-suffix=replay --output-dir=artifacts/m059-architecture/replay` | 0 | ✅ pass | 11600ms |

## Deviations

OpenDataLoader retroactive outputs exposed a schema compatibility gap from S01; `schemas/opendataloader-pdf.v1.json` was extended to accept the historical diagnostic shape while preserving the wrapper adapter shape.

## Known Issues

None.

## Files Created/Modified

- `scripts/m059_validate_pdf_batch.py`
- `scripts/m059_replay_ingest.py`
- `schemas/opendataloader-pdf.v1.json`
