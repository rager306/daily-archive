# M005/S01 Import-Ready Chunk Model Contract Review

Verdict: PASS

## Scope Reviewed

Reviewed against `.gsd/milestones/M005-dlko4z/slices/S01/import-model-review-rubric.md`:

- `.gsd/milestones/M005-dlko4z/slices/S01/import-ready-chunk-contract.md`
- `.gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-manifest.json`
- `.gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-rationale.md`
- `src/arxiv_archive/chunk_import_contract.py`
- `tests/test_chunk_import_contract.py`
- Prior BLOCK review previously stored at `.gsd/milestones/M005-dlko4z/slices/S01/run-evidence/contract-review-summary.md`

Verification run:

- `PYTHONPATH=src:. pytest -q tests/test_chunk_import_contract.py` → 19 passed, 1 pytest config warning (`asyncio_mode` unknown).
- Independent probe script exercised the exact prior blockers and requested final re-review checks: route enum validation, non-import route exclusion from import counts, route/chunk-type compatibility, evidence span containment, retrieval-only validity versus import readiness, nested required fields, redaction leakage, diagnostics count mismatches, and redacted serialization.

## Blockers

None.

## Flags

None.

## Notes

### Route enum validation is enforced

Status: pass.

The validator now defines explicit route enums in `VALID_ROUTES` and rejects unknown graph-ready routes with `invalid_route` (`src/arxiv_archive/chunk_import_contract.py:27`, `src/arxiv_archive/chunk_import_contract.py:381`). Import eligibility also requires `route in VALID_ROUTES`, so invalid routes are not counted import-eligible (`src/arxiv_archive/chunk_import_contract.py:558`). Regression coverage exists in `test_invalid_route_is_rejected_and_not_counted_import_eligible` (`tests/test_chunk_import_contract.py:344`). Independent probing confirmed `route=not_a_route` returns `valid_package=False`, `import_eligible_chunk_count=0`, and `invalid_route`.

### Non-import routes cannot be counted import-eligible

Status: pass.

The validator now defines `NON_IMPORT_ROUTES = {"retrieval_only", "exclude_from_extraction"}` and emits route-specific refusal diagnostics for graph-ready chunks using those routes (`src/arxiv_archive/chunk_import_contract.py:40`, `src/arxiv_archive/chunk_import_contract.py:378`). `_is_import_eligible_chunk` also excludes those routes before counting import eligibility (`src/arxiv_archive/chunk_import_contract.py:563`). Regression coverage exists for graph-ready retrieval-only chunks (`tests/test_chunk_import_contract.py:331`), and independent probing additionally verified `exclude_from_extraction` is refused and counted as zero import-eligible.

A structurally valid retrieval-only package with zero import-eligible chunks remains allowed and not import-ready, preserving the package-validity/import-eligibility distinction (`tests/test_chunk_import_contract.py:227`).

### Route/chunk-type compatibility matrix is enforced

Status: pass.

The executable compatibility matrix in `ROUTE_COMPATIBLE_CHUNK_TYPES` mirrors the written route compatibility boundary and is checked both during graph-ready validation and import-eligible counting (`src/arxiv_archive/chunk_import_contract.py:41`, `src/arxiv_archive/chunk_import_contract.py:383`, `src/arxiv_archive/chunk_import_contract.py:565`). Regression coverage exists in `test_incompatible_route_and_chunk_type_is_rejected` (`tests/test_chunk_import_contract.py:356`). Independent probing confirmed `route=table_extraction` with `chunk_type=claim_candidate` returns `route_chunk_type_mismatch` and `import_eligible_chunk_count=0`.

### Evidence path source span must contain chunk source span

Status: pass.

The validator now resolves the graph-ready chunk evidence path and calls `_evidence_span_contains_chunk_span`; the check requires both spans to be valid, share the same coordinate space, and have evidence `char_start <= chunk.char_start` plus evidence `char_end >= chunk.char_end` (`src/arxiv_archive/chunk_import_contract.py:367`, `src/arxiv_archive/chunk_import_contract.py:571`). Regression coverage exists in `test_evidence_path_span_must_contain_chunk_span` (`tests/test_chunk_import_contract.py:369`). Independent probing confirmed both disjoint spans and mismatched coordinate spaces produce `invalid_source_span` and zero import-eligible chunks.

### Prior nested-field blocker remains fixed

Status: pass.

Nested required-field validation remains present for paper, conversion, element, chunk, annotation, evidence path, quality warning, and diagnostics objects. Regression coverage exists in `test_nested_required_fields_are_validated` (`tests/test_chunk_import_contract.py:259`). Independent probing confirmed missing nested conversion fields still invalidate the package with a specific missing-field reason.

### Prior redaction blocker remains fixed

Status: pass.

The validator continues to reject forbidden raw-text, embedding, vector, secret, and optimizer-trace fields, plus unsafe redaction flags. It also checks conversion-level and diagnostics-level leakage flags and serializes validation output with safe redaction/write flags (`src/arxiv_archive/chunk_import_contract.py:192`, `src/arxiv_archive/chunk_import_contract.py:506`). Regression coverage exists in `test_raw_text_embeddings_and_vectors_are_rejected`, `test_conversion_and_diagnostics_redaction_flags_are_validated`, and `test_validation_serialization_is_redacted` (`tests/test_chunk_import_contract.py:186`, `tests/test_chunk_import_contract.py:288`, `tests/test_chunk_import_contract.py:216`). Independent probing confirmed raw chunk text, embeddings, and vectors are rejected, and validation serialization reports `raw_text_included=false`, `embeddings_included=false`, `ladybugdb_written=false`, and `production_import_attempted=false`.

### Prior diagnostic-count blocker remains fixed

Status: pass.

The validator still compares diagnostics-reported import/refusal counts with computed validator counts (`src/arxiv_archive/chunk_import_contract.py:486`). Regression coverage exists in `test_diagnostics_counts_must_match_computed_counts` (`tests/test_chunk_import_contract.py:309`). Independent probing confirmed stale recorded counts produce `diagnostics_import_count_mismatch` and `diagnostics_refusal_count_mismatch`.

## Overclaim Assessment

Pass. The written contract and corpus artifacts continue to avoid overclaiming import readiness. They explicitly separate package validity from import eligibility, forbid production KG writes in M005, and avoid claims about semantic/vector retrieval, DSPy extraction, LLM chunking, or broad corpus readiness. The executable validator now aligns with the contract’s route, route/type, and evidence-span gates closely enough for S02 baseline measurement to use its import-eligible counts without the prior false-confidence blockers.

## Raw-Text and Embedding Leakage Assessment

Pass. The contract forbids raw paper text, raw chunk text, embeddings, vectors, secrets, credentials, and optimizer traces in machine artifacts. The validator enforces forbidden field names, redaction flags, conversion leakage flags, diagnostics leakage flags, and no-write flags. Tests and independent probes confirm the prior leakage blockers remain fixed.

## Corpus Coverage Assessment

Pass. The manifest reuses the existing deterministic M004 ten-paper corpus rather than silently expanding scope. The inner review minimum covers repaired conversion failures, prior zero-chunk/blocker evidence, S07 trusted candidates, math/theory, multimodal/table/figure risk, method/result boundary risk, and administrative/metadata pollution risk. The rationale correctly treats missing artifacts as S02 findings rather than silently skipping them.

## Recommendation for S02

S02 may proceed to baseline measurement using this contract and validator boundary. S02 must still avoid claiming final import readiness; it may report baseline readiness, blockers, route exclusions, and measurement gaps only. Production KG import remains out of scope for M005 until later dry-run and independent review gates pass.
