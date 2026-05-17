# S04: Claim entity relation contracts

**Goal:** Define typed scientific extraction contracts for Claim, ScientificEntity, and ScientificRelation drafts before adding storage, DSPy, optimizers, or RLM workflows.
**Demo:** After this: fixture text can produce typed Claim, ScientificEntity, and ScientificRelation drafts with confidence, provenance, and validation errors.

## Must-Haves

- Claim, ScientificEntity, ScientificRelation, and extraction patch types exist.
- Drafts reference EvidencePath records or explicit validation errors.
- Validators reject missing evidence, invalid relation endpoints, paper mismatches, unsupported relation types, confidence out of range, and unstable IDs.
- No optimizer, DSPy, RLM, LadybugDB write, embedding, or live LLM call is required for baseline tests.

## Proof Level

- This slice proves: contract and validator tests

## Integration Closure

Consumes S03 EvidencePath records and produces storage-ready draft contracts plus validators for S05 LadybugDB schema expansion. Does not call LLMs, does not enable DSPy, does not write LadybugDB, and does not perform retrieval.

## Verification

- Extraction draft models expose schema version, extractor version, confidence, evidence path references, provenance, and validation diagnostics as code-readable fields.

## Tasks

- [ ] **T01: Add claim entity relation contract tests** `est:45m`
  Create red contract tests for `Claim`, `ScientificEntity`, `ScientificRelation`, and extraction patch/draft models over S03 EvidencePath fixtures. Tests must define stable IDs, confidence fields, provenance, evidence-path references, schema/extractor version fields, and validation diagnostics for missing evidence and invalid confidence. Done when tests fail for missing extraction-contract implementation while S03 evidence tests still pass.
  - Files: `tests/test_scientific_extraction_contracts.py`
  - Verify: uv run pytest tests/test_scientific_extraction_contracts.py -q

- [ ] **T02: Implement scientific extraction contract models** `est:1h 15m`
  Implement `src/arxiv_archive/scientific_extraction.py` with dataclasses for `Claim`, `ScientificEntity`, `ScientificRelation`, and `ExtractionPatch`. Add deterministic ID helpers and baseline validators for evidence presence, confidence range, schema version, and provenance. Done when initial extraction contract tests pass with no LLM/DSPy/storage calls.
  - Files: `src/arxiv_archive/scientific_extraction.py`, `tests/test_scientific_extraction_contracts.py`
  - Verify: uv run pytest tests/test_scientific_extraction_contracts.py tests/test_evidence_paths.py -q

- [ ] **T03: Implement relation and extraction patch validators** `est:1h`
  Add relation and patch validators that reject invalid relation endpoints, unsupported relation types, paper mismatches between entities/claims/evidence, and evidence paths that fail S03 validation. Add tests for valid fixture drafts and invalid endpoint/mismatch cases. Done when S05 can use the patch contract as a pre-storage validation boundary.
  - Files: `src/arxiv_archive/scientific_extraction.py`, `tests/test_scientific_extraction_contracts.py`
  - Verify: uv run pytest tests/test_scientific_extraction_contracts.py tests/test_evidence_paths.py tests/test_page_index.py -q

- [ ] **T04: Run S04 quality gates and regression smoke** `est:30m`
  Run final S04 regression gates: extraction contract tests, S03 evidence tests, PageIndex tests, S01 ingestion tests, analysis regression, CLI contract smoke, Ruff on touched files, and public module help smoke. Record limitations for S05/S07: contracts are deterministic drafts only, no extraction model, no embeddings, no LadybugDB persistence, no DSPy/RLM. Done when S04 is ready for closeout.
  - Files: `src/arxiv_archive/scientific_extraction.py`, `tests/test_scientific_extraction_contracts.py`
  - Verify: uv run pytest tests/test_scientific_extraction_contracts.py tests/test_evidence_paths.py tests/test_page_index.py tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q

## Files Likely Touched

- tests/test_scientific_extraction_contracts.py
- src/arxiv_archive/scientific_extraction.py
