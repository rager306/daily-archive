# S04: Claim entity relation contracts — UAT

**Milestone:** M003-km5fty
**Written:** 2026-05-17T17:56:48.172Z

# S04 UAT: Claim entity relation contracts

## Scenario
A fixture paper section has already passed through S01 full-text ingestion, S02 PageIndex construction, and S03 SemanticChunk/EvidencePath creation. S04 should let downstream code construct typed claim/entity/relation drafts that remain traceable to that evidence path.

## Steps
1. Build the fixture EvidencePath from `tests/fixtures/full_text/structured_paper.md` using the existing S01-S03 pipeline.
2. Construct a `Claim`, `ScientificEntity`, and `ScientificRelation` with confidence, schema version, extractor version, provenance, and the EvidencePath.
3. Bundle them into an `ExtractionPatch`.
4. Run `validate_claim()` and `validate_extraction_patch()`.
5. Mutate the drafts to include missing evidence, invalid confidence, unsupported relation type, duplicate IDs, bad relation endpoints, paper mismatch, and EvidencePath warnings.

## Expected Results
- Valid drafts return no validation diagnostics.
- Invalid drafts return explicit diagnostics for the exact broken contract.
- No LLM, DSPy, optimizer, embedding, retrieval, LadybugDB write, or RLM workflow is required.

## Evidence
- `uv run pytest tests/test_scientific_extraction_contracts.py tests/test_evidence_paths.py tests/test_page_index.py tests/test_full_text_ingestion.py tests/test_analysis.py tests/test_cli_contract.py -q` => `50 passed`.
- `uv run ruff check ...` => `All checks passed!`.
- `uv run python -m arxiv_archive --help` => exit 0 and help rendered.
- `uv run pyrefly check src/arxiv_archive/scientific_extraction.py tests/test_scientific_extraction_contracts.py` => `0 errors`.
- `uv run ty check src/arxiv_archive/scientific_extraction.py tests/test_scientific_extraction_contracts.py` => `All checks passed!`.

