# S05: Source asset preservation and multimodal manifest — UAT

**Milestone:** M005-dlko4z
**Written:** 2026-05-19T10:31:19.095Z

# S05: Source asset preservation and multimodal manifest — UAT

**Milestone:** M005-dlko4z

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: S05 ships source-preservation files, manifests, validation tests, and review artifacts. There is no live UI or service.

## Preconditions

- S04 annotation sidecar diagnostics exist.
- S03 structure-aware package diagnostics exist.
- S01 gold-corpus manifest exists.
- S05 run-evidence directory exists.

## Smoke Test

Run the slice verification command and confirm it prints `53 passed`, `All checks passed!`, and an artifact guard with `source_file_count=12`, `asset_count=283`, `hash_coverage_rate=1.0`, `missing_original_pdf=8`, `review=PASS`, and `safety_flags_false=true`.

## Test Cases

### 1. Source asset contract and tests pass

1. Run `uv run pytest tests/test_source_asset_manifest.py tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q`.
2. **Expected:** 53 tests pass.

### 2. Source files are preserved with hashes

1. Read `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-summary.json`.
2. **Expected:** `paper_count=10`, `valid_manifest_count=10`, `source_file_count=12`, `hash_coverage_rate=1.0`, and `media_type_counts` includes 10 Markdown files and 2 PDFs.

### 3. Missing sources are explicit

1. Read `missing_counts` in the source asset summary.
2. **Expected:** `missing_original_pdf=8` is present.

### 4. Asset records are linked but not extracted facts

1. Read per-paper manifests under `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/manifests/`.
2. **Expected:** There are 283 assets total, all `linked_not_extracted`, all `promoted_to_fact=false`, all exclude `trusted_kg_import`, `production_ladybugdb_write`, and `embedding_generation`.

### 5. Safety flags remain closed

1. Inspect summary and diagnostics safety flags.
2. **Expected:** `raw_text_included=false`, `chunk_text_included=false`, `raw_binary_included=false`, `base64_included=false`, `embeddings_included=false`, `vectors_included=false`, `secrets_included=false`, `ladybugdb_written=false`, and `production_import_attempted=false`.

### 6. Independent review passed

1. Read `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-review-summary.md`.
2. **Expected:** Verdict is PASS and S06 is authorized only for diagnostic/source-review consumption.

## Edge Cases

### Missing PDF

1. Inspect a per-paper manifest whose warning counts include `missing_original_pdf`.
2. **Expected:** The manifest remains valid, the missing PDF is visible as a warning/diagnostic, and no raw content is logged.

### Asset tries to become a KG fact

1. Mutate an asset in tests to set `promoted_to_fact=true` or add `trusted_kg_import` to allowed uses.
2. **Expected:** The validator rejects the manifest.

## Failure Signals

- Tests fail.
- Missing or empty S05 summary/diagnostic/report files.
- `hash_coverage_rate < 1.0` over preserved files.
- Missing PDFs are absent from diagnostics despite known unavailable PDFs.
- Any safety flag is true.
- Any asset has `promoted_to_fact=true` or allows trusted import/embedding generation.

## Requirements Proved By This UAT

- R030 — Source artifacts and asset manifests are preserved with paths, hashes, provenance, linkage, redaction, and safety flags.
- R029 — Import-ready chunk package evidence is advanced with source context and linked asset candidates for future review/benchmark gates.

## Not Proven By This UAT

- Extraction of actual figures/tables/equations/references as standalone files.
- OCR/table structure recovery.
- Multimodal retrieval quality.
- Semantic/vector retrieval.
- Entity/relation/claim extraction quality.
- Trusted KG import or production LadybugDB writes.
