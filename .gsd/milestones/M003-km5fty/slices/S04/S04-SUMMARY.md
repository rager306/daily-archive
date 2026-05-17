---
id: S04
parent: M003-km5fty
milestone: M003-km5fty
provides:
  - Storage-ready ExtractionPatch contract for S05 LadybugDB SCI KG schema expansion.
  - Validation diagnostics that S07 evaluation fixtures can count and assert.
  - Typed draft boundary that S08 DSPy wrappers can target later without changing graph schema.
requires:
  - slice: S03
    provides: EvidencePath and SemanticChunk traceability substrate consumed by S04 tests and model fields.
affects:
  - S05
  - S06
  - S07
  - S08
  - S09
  - S10
key_files:
  - src/arxiv_archive/scientific_extraction.py
  - tests/test_scientific_extraction_contracts.py
key_decisions:
  - Kept S04 deterministic and local-only: no DSPy, LLM, embeddings, retrieval, RLM, or LadybugDB writes.
  - Used data-returned validation diagnostics instead of exceptions for draft contract validation so S05/S07 can inspect failures.
  - Represented claim/entity/relation provenance, schema version, extractor version, confidence, and evidence path as explicit fields.
patterns_established:
  - Scientific extraction drafts must be deterministic dataclasses before graph persistence.
  - Draft validation returns inspectable diagnostic strings rather than hiding invalid data behind exceptions.
  - Every claim/entity/relation draft must carry evidence_path, confidence, schema_version, extractor_version, and provenance.
observability_surfaces:
  - Validation diagnostics for Claim, ScientificEntity, ScientificRelation, and ExtractionPatch.
  - Explicit schema_version, extractor_version, confidence, provenance, and EvidencePath fields on all draft objects.
  - Duplicate ID, endpoint, paper mismatch, unsupported relation type, and evidence warning messages suitable for S05/S07 inspection.
drill_down_paths:
  - .gsd/milestones/M003-km5fty/slices/S04/tasks/T01-SUMMARY.md
  - .gsd/milestones/M003-km5fty/slices/S04/tasks/T02-SUMMARY.md
  - .gsd/milestones/M003-km5fty/slices/S04/tasks/T03-SUMMARY.md
  - .gsd/milestones/M003-km5fty/slices/S04/tasks/T04-SUMMARY.md
duration: ""
verification_result: passed
completed_at: 2026-05-17T17:56:48.172Z
blocker_discovered: false
---

# S04: Claim entity relation contracts

**S04 defined deterministic Claim, ScientificEntity, ScientificRelation, and ExtractionPatch contracts with evidence-path-backed validators.**

## What Happened

S04 added the typed scientific extraction contract layer on top of S03 EvidencePath. It introduced frozen dataclasses for Claim, ScientificEntity, ScientificRelation, and ExtractionPatch, plus deterministic ID helper functions and validation functions for individual drafts and complete patches. Validators now report missing evidence, evidence-path warnings, invalid confidence, missing schema/extractor versions, missing provenance, unstable ID prefixes, unsupported relation types, invalid relation endpoints, duplicate draft IDs, paper mismatches, and evidence paper mismatches. Tests build EvidencePath records from the existing full-text/PageIndex/SemanticChunk fixture pipeline, proving that S04 attaches to the real S03 substrate rather than inventing isolated mocks. Final verification passed across S04, S03, S02, S01, analysis, CLI, lint, diagnostics, and type checks.

## Verification

Fresh verification after the final code change passed: `uv run pytest tests/test_scientific_extraction_contracts.py tests/test_evidence_paths.py tests/test_page_index.py tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q` reported 50 passed; Ruff reported all checks passed; CLI help smoke exited 0; LSP diagnostics reported no diagnostics; Pyrefly reported 0 errors; Ty reported all checks passed.

## Requirements Advanced

- R017 — Implemented and verified Claim, ScientificEntity, ScientificRelation, and ExtractionPatch contracts backed by EvidencePath and explicit validation diagnostics.

## Requirements Validated

- R017 — S04 verification passed: 50 pytest tests across extraction/evidence/PageIndex/ingestion/analysis/CLI, Ruff clean, CLI help smoke exit 0, LSP diagnostics clean, Pyrefly 0 errors, Ty passed.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

T02 implemented a baseline validate_extraction_patch path earlier than planned so T03 could focus on deeper validator edge cases. Scope remained inside S04 typed contracts and validators.

## Known Limitations

S04 does not extract claims/entities/relations from text automatically; it defines draft contracts and validators only. It does not persist to LadybugDB, create embeddings, perform retrieval, invoke DSPy, run optimizers, or implement RLM workflows.

## Follow-ups

S05 should consume ExtractionPatch as the pre-storage validation boundary for LadybugDB writes. S07 should include these validation diagnostics in benchmark metrics. S08 must remain gated until S07 metrics and fixtures are verified.

## Files Created/Modified

- `src/arxiv_archive/scientific_extraction.py` — New deterministic Claim, ScientificEntity, ScientificRelation, and ExtractionPatch dataclasses with ID helpers and validators.
- `tests/test_scientific_extraction_contracts.py` — New S04 contract and validator tests over S03 EvidencePath fixtures.
