# S05 Source Asset Preservation Report

## Verdict

S05 source asset preservation is ready for downstream benchmark review, with explicit limitations.

The slice preserves available source artifacts, writes redacted per-paper manifests, links S04 annotation sidecars to asset records, and keeps all assets as non-fact diagnostic metadata. It does not claim multimodal extraction, embeddings, KG import readiness, or production persistence.

## What is preserved

From the 10-paper gold corpus:

- Papers covered: 10
- Valid per-paper manifests: 10
- Preserved source files: 12
- Hash coverage over preserved source files: 1.0
- Media types preserved:
  - `text/markdown`: 10
  - `application/pdf`: 2

Each preserved source file records:

- stable source file id
- original path
- S05 workspace path
- SHA-256
- byte size
- media type
- provenance
- redaction and no-write flags

## Missing sources

The dry run found 8 missing original PDFs in the current local source paths:

```json
{
  "missing_original_pdf": 8
}
```

This is an observed limitation, not a silent failure. The manifests preserve all available normalized Markdown and the 2 PDFs currently available in the required/fallback paths. Future acquisition or repair work can target the missing PDFs directly from the manifest diagnostics.

## Asset linkage

The dry run links S04 annotation sidecars and S03 source spans into 283 asset records:

```json
{
  "equation": 146,
  "figure": 86,
  "metadata": 2,
  "reference": 11,
  "table": 38
}
```

All asset records are currently:

```text
extraction_state = linked_not_extracted
promoted_to_fact = false
```

Allowed uses:

```text
source_review
benchmark_diagnostics
```

Excluded uses:

```text
trusted_kg_import
production_ladybugdb_write
embedding_generation
```

## Redaction and safety boundary

The run-level safety flags are all false:

```json
{
  "raw_text_included": false,
  "chunk_text_included": false,
  "raw_binary_included": false,
  "base64_included": false,
  "embeddings_included": false,
  "vectors_included": false,
  "secrets_included": false,
  "ladybugdb_written": false,
  "production_import_attempted": false
}
```

Machine JSON/JSONL artifacts contain paths, hashes, spans, provenance, linkage, warning counts, and safety flags. They do not contain raw paper text, raw chunk text, raw binary/base64 payloads, embeddings, vectors, secrets, optimizer traces, or KG facts.

## Independent review

Independent artifact review returned PASS.

Key review conclusions:

- Source/hash coverage is meaningful and inspectable.
- Missing PDFs are explicit at run and per-paper levels.
- Per-paper manifests validate.
- Asset records carry source pointers and spans, not raw content.
- All assets remain non-facts and blocked from import/write/embedding generation.
- S06 can safely consume these manifests as diagnostic/source-review input.

Review artifact:

```text
.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-review-summary.md
```

## Authoritative diagnostics

Future agents should inspect these first:

- `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-summary.json` — run-level source/asset counts, missing-source diagnostics, safety flags.
- `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/source-asset-package-diagnostics.jsonl` — per-paper source file hash/size/media metadata and warning counts.
- `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/manifests/*.json` — per-paper source files and linked asset records.
- `.gsd/milestones/M005-dlko4z/slices/S05/run-evidence/papers/*/source/` — preserved source files.

## What S06 should do next

S06 can benchmark chunking methods using these manifests as source-review context. In particular, S06 should:

- join asset records to `source_files` by `source_file_id` for immutable source hash context;
- compare chunking candidates on table/figure/equation/reference linkage quality;
- treat `linked_not_extracted` as candidate evidence only;
- keep KG import, embeddings, and production writes blocked unless a later review explicitly authorizes them.

## What remains unproven

S05 does not prove:

- extraction of figures, tables, equations, or references as standalone files;
- OCR or table structure recovery;
- multimodal retrieval quality;
- semantic/vector retrieval;
- claim/entity/relation extraction quality;
- KG import readiness;
- production LadybugDB writes;
- broad corpus scaling.
