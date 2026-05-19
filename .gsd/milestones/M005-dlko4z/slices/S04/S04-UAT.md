# S04: Chunk annotation sidecars — UAT

**Milestone:** M005-dlko4z
**Written:** 2026-05-19T08:47:22.644Z

# S04: Chunk annotation sidecars — UAT

**Milestone:** M005-dlko4z

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: S04 ships deterministic package/test/artifact behavior, not a live UI or service. The acceptance contract is proven by tests, redacted JSON/JSONL evidence, and independent artifact review.

## Preconditions

- S03 structure-aware chunking is complete.
- `.gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-manifest.json` exists.
- S04 run-evidence files exist and are non-empty.

## Smoke Test

Run the slice verification command and confirm it prints `38 passed`, `All checks passed!`, and an artifact guard with `coverage_rate: 1.0`, `review: PASS`, and `safety_flags_false: true`.

## Test Cases

### 1. Contract tests pass

1. Run `uv run pytest tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q`.
2. **Expected:** 38 tests pass, including unresolved annotation chunk, promoted fact, nested raw-text leakage, and annotation-values-do-not-authorize-import tests.

### 2. Annotation artifacts are present and reviewable

1. Check `.gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-summary.json` is non-empty.
2. Check `.gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-package-diagnostics.jsonl` is non-empty.
3. **Expected:** Summary reports `paper_count=10`, `chunk_count=1831`, `annotated_chunk_count=1831`, `annotation_count=7448`, `chunk_annotation_coverage_rate=1.0`, and `min_annotations_per_chunk>=4`.

### 3. Import/no-write boundary remains closed

1. Inspect the summary and diagnostics safety flags.
2. **Expected:** `promoted_to_fact_count=0`, `import_ready_count=0`, `import_eligible_chunk_count=0`, `raw_text_included=false`, `chunk_text_included=false`, `embeddings_included=false`, `vectors_included=false`, `secrets_included=false`, `ladybugdb_written=false`, and `production_import_attempted=false`.

### 4. Independent review passed

1. Read `.gsd/milestones/M005-dlko4z/slices/S04/run-evidence/annotation-review-summary.md`.
2. **Expected:** Verdict is PASS after remediation of aggregate-only diagnostics.

## Edge Cases

### Annotation tries to become a fact

1. Mutate an annotation to set `promoted_to_fact=true` in tests.
2. **Expected:** Validator rejects the package with `annotation_promoted_to_fact`.

### Annotation hides raw text in nested values

1. Mutate an annotation value to include a nested `raw_text` field in tests.
2. **Expected:** Validator rejects the package with `raw_text_leakage`, and diagnostics do not include the leaked raw sentence.

## Failure Signals

- Any test failure in `tests/test_structure_aware_chunking.py` or `tests/test_chunk_import_contract.py`.
- Missing or empty S04 run-evidence files.
- `chunk_annotation_coverage_rate < 1.0` or `min_annotations_per_chunk < 4`.
- Any safety flag set to true.
- Any `promoted_to_fact_count > 0` or `import_eligible_chunk_count > 0`.

## Requirements Proved By This UAT

- R029 — Advances the typed chunk package with deterministic sidecars, redaction, route metadata, and review blockers while keeping import blocked.
- R030 — Advances future source asset preservation by producing table/figure asset-link hints without treating assets or annotations as KG facts.

## Not Proven By This UAT

- Source PDF/Markdown/asset preservation is not implemented yet; that is S05.
- Real chunking-method benchmarking is not implemented yet; that is S06.
- Isolated KG import rehearsal is not implemented yet; that is S07.
- Semantic/vector retrieval, entity extraction, relation extraction, and production LadybugDB persistence remain unvalidated.
