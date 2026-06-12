---
id: M059-y6osma
title: "M060 Manifest Driven PDF Ingest Architecture"
status: complete
completed_at: 2026-06-12T10:51:18.267Z
key_decisions:
  - ADR-013 binds future PDF ingest to manifest-driven validation and replay contracts.
  - M061 should scale 2-hop BFS through manifest-gated validation/replay, not direct graph import.
  - S02 replay remains local artifact-level replay because all safety defaults remain false.
key_files:
  - schemas/daily-archive.pdf-batch-manifest.v1.json
  - schemas/daily-archive.parser-op.v1.json
  - schemas/grobid-tei.v1.json
  - schemas/opendataloader-pdf.v1.json
  - scripts/m059_build_manifest.py
  - scripts/m059_jsonschema_validate.py
  - scripts/m059_validate_pdf_batch.py
  - scripts/m059_replay_ingest.py
  - scripts/m059_e2e_test.py
  - tests/test_m059_s01.py
  - tests/test_m059_s02.py
  - artifacts/m059-architecture/decision.md
  - artifacts/m059-architecture/m054-validation-report.json
  - artifacts/m059-architecture/m054-grobid-replay-report.json
  - artifacts/m059-architecture/m059-s02-e2e-report.json
lessons_learned:
  - Retroactive schemas must explicitly include historical diagnostic output shapes when manifests point at historical artifacts.
  - Replay tooling should report non-deterministic parser outputs without asserting byte identity.
---

# M059-y6osma: M060 Manifest Driven PDF Ingest Architecture

**M059 established manifest-driven PDF ingest contracts plus operational validation and deterministic replay proof for M061 scale-up.**

## What Happened

M059 delivered the manifest-driven ingest architecture in two slices. S01 created the schema layer, retroactive manifests, validator baseline, and ADR-013. S02 added operational validation and replay tooling, exercised it end-to-end on the M054 five-PDF batch across GROBID and OpenDataLoader, produced byte-identical GROBID replay evidence, added tests, ran guardrails, and emitted the M061 decision document. The milestone closes with manifest-gated validation and replay ready to constrain future 2-hop BFS scale-up work without authorizing graph writes, production import, fact promotion, external network calls, or LLM calls by default.

## Success Criteria Results

- PASS: Six schemas and retroactive manifests were delivered by S01.
- PASS: `scripts/m059_validate_pdf_batch.py` validates M054 GROBID and OpenDataLoader outputs 5/5 each.
- PASS: `scripts/m059_replay_ingest.py` verifies deterministic GROBID replay with SHA-256 byte identity.
- PASS: `scripts/m059_e2e_test.py` writes validation/replay/e2e reports and returns `passed=true`.
- PASS: M061 decision doc emitted at `artifacts/m059-architecture/decision.md`.

## Definition of Done Results

- PASS: S01 and S02 are complete in GSD.
- PASS: `uv run pytest tests/test_m059_s02.py -q` passed 7/7.
- PASS: `uv run pytest tests/test_m059_s01.py -q` passed 8/8 after the OpenDataLoader schema compatibility fix.
- PASS: M045 trajectory reported `verdict=on_track`.
- PASS: M044 guardrail reported ok.
- PASS: Milestone validation recorded verdict `pass`.

## Requirement Outcomes

No explicit requirement IDs were advanced in the active GSD requirement contract. The milestone vision was satisfied by the S01/S02 artifacts and validation evidence.

## Deviations

S02 extended `schemas/opendataloader-pdf.v1.json` to add the retroactive M055 diagnostic branch required for the M054 OpenDataLoader validation proof.

## Follow-ups

Use `artifacts/m059-architecture/decision.md` to scope M061 manifest-gated 2-hop BFS scale-up. Keep safety defaults false until a later milestone explicitly authorizes a narrower mutation path.
